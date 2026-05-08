const DAILY_BRIEF_RESULT_BEGIN: &str = "DAILY_BRIEF_RESULT_JSON_BEGIN";
const DAILY_BRIEF_RESULT_END: &str = "DAILY_BRIEF_RESULT_JSON_END";

async fn run_daily_brief(
    client: &reqwest::Client,
    config: &Config,
    worker_run: &WorkerRunState,
    daily_brief_id: &str,
) -> Result<String> {
    let workdir = ensure_worktree(config, worker_run).await?;
    let prompt = daily_brief_agent_prompt(daily_brief_id, worker_run);

    info!(
        worker_run_id = %worker_run.id,
        daily_brief_id,
        workdir = %workdir.display(),
        "starting local Codex DailyBrief agent"
    );

    let output = run_codex_exec_command(config, &workdir, prompt, "run local Codex DailyBrief")
        .await?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if stderr.trim().is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}\n\n[stderr]\n{stderr}")
    };

    if !output.status.success() {
        bail!(
            "DailyBrief Codex agent failed with status {:?}: {}",
            output.status.code(),
            truncate_middle(&combined, 4_000)
        );
    }

    let brief = parse_daily_brief_agent_output(&combined)?;
    post_entity_action(
        client,
        config,
        "DailyBriefs",
        daily_brief_id,
        "Render",
        json!({
            "summary_markdown": brief.summary_markdown,
            "visual_summary_url": brief.visual_summary_url,
            "proof_packet_ids": brief.proof_packet_ids,
            "open_risks": brief.open_risks,
            "done_items": brief.done_items,
        }),
    )
    .await?;

    Ok(format!(
        "Agent-led DailyBrief {daily_brief_id} rendered by local Codex WorkerRun {}.\n\nResidual risks: {}\n\nSummary:\n{}",
        worker_run.id,
        empty_fallback(&brief.residual_risks, "None recorded."),
        truncate_middle(&brief.summary_markdown, 3_000)
    ))
}

fn extract_daily_brief_id(task: &str) -> Option<String> {
    task.lines().find_map(|line| {
        line.trim()
            .strip_prefix("DailyBrief:")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn daily_brief_agent_prompt(daily_brief_id: &str, worker_run: &WorkerRunState) -> String {
    format!(
        r#"You are the local Codex DailyBrief agent for TemperPaw Paw Patrol.

DailyBrief: {daily_brief_id}
WorkerRun: {worker_run_id}
Branch: {branch}

Use the source facts in the WorkerRun task below as your factual substrate. You may inspect the local repo if needed for names or links, but do not edit files and do not invent completed work.
Your job is judgment and presentation: synthesize what matters, make it visually readable, call out risks, and keep it useful to both humans and future agents.

WorkerRun task / source facts:
{task}

Return exactly one JSON object between these markers, with no markdown outside the markers:
{begin}
{{
  "summary_markdown": "Concise human-readable markdown. Include Mermaid diagrams when they clarify state or risk flow.",
  "visual_summary_url": "data:image/svg+xml,... factual visual summary; use the supplied fallback visual from the task if you cannot improve it",
  "proof_packet_ids": ["pp-..."],
  "open_risks": [{{"type":"QualityFinding|SecurityFinding|ObservabilityFinding","id":"...","title":"...","severity":"..."}}],
  "done_items": [{{"type":"WorkCycle|ProofPacket|PatrolRun","id":"...","summary":"..."}}],
  "residual_risks": ["Any caveats about incomplete data or follow-up needed."]
}}
{end}

The JSON must be valid. Keep the brief readable and visual."#,
        daily_brief_id = daily_brief_id,
        worker_run_id = worker_run.id,
        branch = worker_run_branch_label(worker_run),
        task = empty_fallback(&worker_run.task, "(no source facts recorded)"),
        begin = DAILY_BRIEF_RESULT_BEGIN,
        end = DAILY_BRIEF_RESULT_END,
    )
}

fn parse_daily_brief_agent_output(output: &str) -> Result<DailyBriefAgentOutput> {
    let json_text = extract_daily_brief_result_json(output)?;
    let raw: DailyBriefRawOutput =
        serde_json::from_str(json_text).context("parse DailyBrief Codex JSON")?;
    raw.into_agent_output()
}

fn extract_daily_brief_result_json(output: &str) -> Result<&str> {
    let (_, after_begin) = output
        .split_once(DAILY_BRIEF_RESULT_BEGIN)
        .context("DailyBrief Codex output was missing result begin marker")?;
    let (json_text, _) = after_begin
        .split_once(DAILY_BRIEF_RESULT_END)
        .context("DailyBrief Codex output was missing result end marker")?;
    let json_text = json_text.trim();
    if json_text.is_empty() {
        bail!("DailyBrief Codex result JSON was empty");
    }
    Ok(json_text)
}

#[derive(Debug)]
struct DailyBriefAgentOutput {
    summary_markdown: String,
    visual_summary_url: String,
    proof_packet_ids: String,
    open_risks: String,
    done_items: String,
    residual_risks: String,
}

#[derive(Debug, serde::Deserialize)]
struct DailyBriefRawOutput {
    #[serde(default)]
    summary_markdown: String,
    #[serde(default)]
    visual_summary_url: String,
    #[serde(default)]
    proof_packet_ids: Value,
    #[serde(default)]
    open_risks: Value,
    #[serde(default)]
    done_items: Value,
    #[serde(default)]
    residual_risks: Value,
}

impl DailyBriefRawOutput {
    fn into_agent_output(self) -> Result<DailyBriefAgentOutput> {
        let summary_markdown = self.summary_markdown.trim().to_string();
        if summary_markdown.is_empty() {
            bail!("DailyBrief Codex result summary_markdown was empty");
        }

        Ok(DailyBriefAgentOutput {
            summary_markdown,
            visual_summary_url: normalize_visual_summary_url(&self.visual_summary_url),
            proof_packet_ids: normalize_json_array_string(self.proof_packet_ids, "proof_packet_ids")?,
            open_risks: normalize_json_array_string(self.open_risks, "open_risks")?,
            done_items: normalize_json_array_string(self.done_items, "done_items")?,
            residual_risks: normalize_residual_risks(self.residual_risks)?,
        })
    }
}

fn normalize_visual_summary_url(value: &str) -> String {
    let value = value.trim();
    if value.starts_with("data:image/svg+xml") || value.starts_with("http://") || value.starts_with("https://") {
        value.to_string()
    } else {
        let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='1200' height='720' viewBox='0 0 1200 720'><rect width='1200' height='720' fill='#f8fafc'/><text x='80' y='140' font-family='Inter, Arial, sans-serif' font-size='54' font-weight='700' fill='#0f172a'>Patrol Daily Brief</text><text x='80' y='220' font-family='Inter, Arial, sans-serif' font-size='28' fill='#334155'>Rendered by local Codex from Temper state.</text></svg>";
        format!("data:image/svg+xml,{}", percent_encode_text(svg))
    }
}

fn normalize_json_array_string(value: Value, label: &str) -> Result<String> {
    match value {
        Value::Array(_) => Ok(value.to_string()),
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Ok("[]".to_string());
            }
            let parsed: Value =
                serde_json::from_str(trimmed).with_context(|| format!("parse {label} JSON string"))?;
            if !parsed.is_array() {
                bail!("{label} JSON string must decode to an array");
            }
            Ok(parsed.to_string())
        }
        Value::Null => Ok("[]".to_string()),
        _ => bail!("{label} must be an array or JSON array string"),
    }
}

fn normalize_residual_risks(value: Value) -> Result<String> {
    match value {
        Value::Array(items) => {
            let risks = items
                .into_iter()
                .filter_map(|item| item.as_str().map(str::trim).map(str::to_string))
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>();
            if risks.is_empty() {
                Ok("None recorded.".to_string())
            } else {
                Ok(risks.join("; "))
            }
        }
        Value::String(text) => Ok(empty_fallback(text.trim(), "None recorded.").to_string()),
        Value::Null => Ok("None recorded.".to_string()),
        _ => bail!("residual_risks must be an array or string"),
    }
}

fn empty_fallback<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn percent_encode_text(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            b' ' => "%20".to_string(),
            other => format!("%{other:02X}"),
        })
        .collect::<Vec<_>>()
        .join("")
}
