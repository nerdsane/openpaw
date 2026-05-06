//! Daily Brief Lifecycle - spawn the agent-driven DailyBrief Session.
//!
//! Triggered by `DailyBrief.Start`. This integration collects recent proof
//! packets, completed work cycles, and open risks, then creates a Session that
//! synthesizes the human-readable daily brief and dispatches `DailyBrief.Render`
//! with a visual_daily_brief_svg data URI. It keeps the daily rollup
//! Temper-visible without hiding the judgment work inside deterministic glue.

use temper_wasm_sdk::prelude::*;

const PROOF_PACKETS_PATH: &str = "/tdata/ProofPackets";
const QUALITY_FINDINGS_PATH: &str = "/tdata/QualityFindings";
const SECURITY_FINDINGS_PATH: &str = "/tdata/SecurityFindings";
const SESSIONS_PATH: &str = "/tdata/Sessions";
const WORK_CYCLES_PATH: &str = "/tdata/WorkCycles";

const SESSION_CONFIGURE: &str = "TemperPaw.Configure";
const PATROL_ATTACH_SESSION: &str = "TemperPaw.Patrol.AttachSession";
const PATROL_RENDER: &str = "TemperPaw.Patrol.Render";

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let base_url = resolve_api_url(&ctx);
        let headers = odata_headers(&ctx);
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        match ctx.trigger_action.as_str() {
            "Start" => handle_start(&ctx, &base_url, &headers, &fields),
            other => Err(format!(
                "daily_brief_lifecycle: unsupported trigger {other}"
            )),
        }?;

        set_success_result("", &json!({ "status": "daily_brief_lifecycle_complete" }));
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

fn handle_start(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let brief_id = entity_id(ctx);
    let brief_date = string_param(ctx, fields, "brief_date", "BriefDate");
    let proofs = query_collection(
        ctx,
        base_url,
        headers,
        PROOF_PACKETS_PATH,
        "Status eq 'Ready'",
        20,
    )?;
    let work_cycles = query_collection(
        ctx,
        base_url,
        headers,
        WORK_CYCLES_PATH,
        "Status eq 'Complete'",
        20,
    )?;
    let quality_risks = query_collection(
        ctx,
        base_url,
        headers,
        QUALITY_FINDINGS_PATH,
        "Status eq 'Open'",
        20,
    )?;
    let security_risks = query_collection(
        ctx,
        base_url,
        headers,
        SECURITY_FINDINGS_PATH,
        "Status eq 'Open'",
        20,
    )?;

    let proof_packet_ids = ids_json(&proofs);
    let done_items = done_items_json(&work_cycles, &proofs);
    let open_risks = open_risks_json(&quality_risks, &security_risks);
    let fallback_visual_summary_url = visual_daily_brief_svg(
        &brief_date,
        proofs.len(),
        work_cycles.len(),
        quality_risks.len() + security_risks.len(),
    );
    let session_id = create_entity(ctx, base_url, headers, SESSIONS_PATH)?;
    let session_model = configured_session_value(ctx, "daily_brief_model", "mock");
    let session_provider = configured_session_value(ctx, "daily_brief_provider", "mock");
    let prompt_input = DailyBriefPrompt {
        brief_id: &brief_id,
        brief_date: &brief_date,
        proof_packet_ids: &proof_packet_ids,
        done_items: &done_items,
        open_risks: &open_risks,
        proof_count: proofs.len(),
        completed_work_count: work_cycles.len(),
        quality_risk_count: quality_risks.len(),
        security_risk_count: security_risks.len(),
        fallback_visual_summary_url: &fallback_visual_summary_url,
    };
    let session_prompt = if session_provider == "mock" {
        mock_daily_brief_plan(&prompt_input)
    } else {
        daily_brief_session_prompt(&prompt_input)
    };

    post_action(
        ctx,
        base_url,
        headers,
        "Sessions",
        &session_id,
        SESSION_CONFIGURE,
        &json!({
            "system_prompt": "You are a Patrol daily brief agent. Produce factual, visual, human-readable summaries from Temper state, and close the loop by dispatching Temper actions.",
            "user_message": session_prompt,
            "model": session_model,
            "provider": session_provider,
            "temperature": "0.4",
            "max_turns": "8",
            "tools_enabled": "temper_get,temper_list,temper_action,temper_read",
            "temper_api_url": base_url,
            "soul_id": "SRE",
            "agent_id": "paw-patrol-daily-brief",
            "session_mode": "patrol_daily_brief"
        }),
    )?;
    post_action(
        ctx,
        base_url,
        headers,
        "DailyBriefs",
        &brief_id,
        PATROL_ATTACH_SESSION,
        &json!({
            "session_id": &session_id,
            "session_status": "running"
        }),
    )?;

    ctx.log(
        "info",
        &format!(
            "daily_brief_lifecycle: attached agent-driven DailyBrief Session {session_id} to {brief_id}"
        ),
    );
    Ok(())
}

struct DailyBriefPrompt<'a> {
    brief_id: &'a str,
    brief_date: &'a str,
    proof_packet_ids: &'a str,
    done_items: &'a str,
    open_risks: &'a str,
    proof_count: usize,
    completed_work_count: usize,
    quality_risk_count: usize,
    security_risk_count: usize,
    fallback_visual_summary_url: &'a str,
}

fn daily_brief_session_prompt(input: &DailyBriefPrompt<'_>) -> String {
    format!(
        "You are creating the agent-driven DailyBrief Session for Patrol.\n\nDailyBrief entity: {}\nDate: {}\n\nSource facts collected by daily_brief_lifecycle:\n- Completed WorkCycles: {}\n- Ready ProofPackets: {}\n- Open quality risks: {}\n- Open security risks: {}\n\nproof_packet_ids JSON:\n{}\n\ndone_items JSON:\n{}\n\nopen_risks JSON:\n{}\n\nRequired output action:\nUse `temper.action(\"DailyBriefs\", \"{}\", \"Render\", params)` to dispatch the Patrol render action. The OData action is `{}`.\n\nRender params:\n- summary_markdown: concise daily brief with done items, open risks, escalations, and next actions.\n- visual_summary_url: factual visual daily summary. You may reuse this fallback visual_daily_brief_svg if no better factual diagram is available: {}\n- proof_packet_ids: JSON array string from the source facts, unless you find newer Ready ProofPackets.\n- open_risks: JSON array string, refined only if you can cite Temper evidence.\n- done_items: JSON array string, refined only if you can cite Temper evidence.\n\nRules:\n- Keep the summary super readable for humans and agents.\n- Include Mermaid diagrams in summary_markdown when they clarify state transitions or risk flow.\n- Do not invent work. Query Temper if you need more detail.\n- If you cannot render safely, dispatch `TemperPaw.Patrol.Fail` on this DailyBrief with an error_message explaining the blocker.",
        input.brief_id,
        empty_fallback(input.brief_date, "unspecified"),
        input.completed_work_count,
        input.proof_count,
        input.quality_risk_count,
        input.security_risk_count,
        input.proof_packet_ids,
        input.done_items,
        input.open_risks,
        input.brief_id,
        PATROL_RENDER,
        input.fallback_visual_summary_url
    )
}

fn mock_daily_brief_plan(input: &DailyBriefPrompt<'_>) -> String {
    let summary_markdown = format!(
        "# Patrol Daily Brief\n\nDate: {}\n\n```mermaid\nflowchart LR\n  Work[\"Completed WorkCycles: {}\"] --> Proofs[\"Ready ProofPackets: {}\"]\n  Proofs --> Risks[\"Open risks: {}\"]\n  Risks --> Brief[\"DailyBrief.Render\"]\n```\n\n## Done items\n\n- Completed WorkCycles: {}\n- Ready ProofPackets: {}\n\n## Open risks\n\n- Quality findings: {}\n- Security findings: {}\n\nThis deterministic mock brief proves the agent-driven DailyBrief Session can dispatch `DailyBrief.Render`. Real providers replace this with richer judgment and visual prioritization.",
        empty_fallback(input.brief_date, "unspecified"),
        input.completed_work_count,
        input.proof_count,
        input.quality_risk_count + input.security_risk_count,
        input.completed_work_count,
        input.proof_count,
        input.quality_risk_count,
        input.security_risk_count
    );
    let params = json!({
        "summary_markdown": summary_markdown,
        "visual_summary_url": input.fallback_visual_summary_url,
        "proof_packet_ids": input.proof_packet_ids,
        "open_risks": input.open_risks,
        "done_items": input.done_items
    });
    mock_temper_action_plan(
        "mock-daily-brief-render",
        "DailyBriefs",
        input.brief_id,
        "Render",
        params,
        "DailyBrief rendered by deterministic mock_plan.",
    )
}

fn query_collection(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    path: &str,
    filter: &str,
    top: usize,
) -> Result<Vec<Value>, String> {
    let url = format!(
        "{base_url}{path}?$filter={}&$top={top}",
        encode_query(filter)
    );
    let resp = ctx.http_call("GET", &url, headers, "")?;
    let body = parse_json_response(resp, path)?;
    Ok(body
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

fn ids_json(items: &[Value]) -> String {
    json!(
        items
            .iter()
            .filter_map(entity_id_from_response)
            .collect::<Vec<_>>()
    )
    .to_string()
}

fn done_items_json(work_cycles: &[Value], proofs: &[Value]) -> String {
    let mut items = Vec::new();
    for work_cycle in work_cycles {
        let id = entity_id_from_response(work_cycle).unwrap_or_default();
        let task = string_from_entity(work_cycle, "task_summary", "TaskSummary");
        items.push(json!({
            "type": "WorkCycle",
            "id": id,
            "summary": empty_fallback(&task, "completed Patrol work")
        }));
    }
    for proof in proofs {
        let id = entity_id_from_response(proof).unwrap_or_default();
        items.push(json!({
            "type": "ProofPacket",
            "id": id,
            "summary": "proof ready"
        }));
    }
    json!(items).to_string()
}

fn open_risks_json(quality: &[Value], security: &[Value]) -> String {
    let mut risks = Vec::new();
    for item in quality {
        risks.push(json!({
            "type": "QualityFinding",
            "id": entity_id_from_response(item).unwrap_or_default(),
            "title": empty_fallback(&string_from_entity(item, "title", "Title"), "quality risk"),
            "severity": empty_fallback(&string_from_entity(item, "severity", "Severity"), "medium")
        }));
    }
    for item in security {
        risks.push(json!({
            "type": "SecurityFinding",
            "id": entity_id_from_response(item).unwrap_or_default(),
            "title": empty_fallback(&string_from_entity(item, "title", "Title"), "security risk"),
            "severity": empty_fallback(&string_from_entity(item, "severity", "Severity"), "high"),
            "risk_lane": empty_fallback(&string_from_entity(item, "risk_lane", "RiskLane"), "L2")
        }));
    }
    json!(risks).to_string()
}

fn visual_daily_brief_svg(
    brief_date: &str,
    proof_count: usize,
    completed_work_count: usize,
    open_risk_count: usize,
) -> String {
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='1200' height='720' viewBox='0 0 1200 720'><rect width='1200' height='720' fill='#eef2ff'/><rect x='56' y='56' width='1088' height='608' rx='18' fill='#ffffff' stroke='#1f2937' stroke-width='3'/><text x='96' y='136' font-family='Inter, Arial, sans-serif' font-size='46' font-weight='700' fill='#111827'>Patrol Daily Brief</text><text x='96' y='194' font-family='Inter, Arial, sans-serif' font-size='24' fill='#4b5563'>Date: {}</text><g font-family='Inter, Arial, sans-serif'><rect x='96' y='278' width='270' height='170' rx='14' fill='#dcfce7' stroke='#16a34a'/><text x='126' y='338' font-size='26' font-weight='700' fill='#166534'>Done items</text><text x='126' y='400' font-size='52' font-weight='800' fill='#14532d'>{}</text><rect x='466' y='278' width='270' height='170' rx='14' fill='#dbeafe' stroke='#2563eb'/><text x='496' y='338' font-size='26' font-weight='700' fill='#1e3a8a'>Proofs ready</text><text x='496' y='400' font-size='52' font-weight='800' fill='#1d4ed8'>{}</text><rect x='836' y='278' width='270' height='170' rx='14' fill='#fee2e2' stroke='#dc2626'/><text x='866' y='338' font-size='26' font-weight='700' fill='#991b1b'>Open risks</text><text x='866' y='400' font-size='52' font-weight='800' fill='#b91c1c'>{}</text></g><text x='96' y='548' font-family='Inter, Arial, sans-serif' font-size='22' fill='#374151'>Review only escalations or risk lanes that require human approval. Everything else should already have worker, reviewer, evaluator, and proof evidence.</text></svg>",
        escape_xml(empty_fallback(brief_date, "unspecified")),
        completed_work_count,
        proof_count,
        open_risk_count
    );
    format!("data:image/svg+xml,{}", percent_encode_svg(&svg))
}

fn entity_id(ctx: &Context) -> String {
    ctx.entity_state
        .get("entity_id")
        .and_then(Value::as_str)
        .unwrap_or(&ctx.entity_id)
        .to_string()
}

fn string_param(ctx: &Context, fields: &Value, snake: &str, pascal: &str) -> String {
    ctx.trigger_params
        .get(snake)
        .and_then(Value::as_str)
        .or_else(|| ctx.trigger_params.get(pascal).and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| string_from_fields(fields, snake, pascal))
}

fn string_from_entity(entity: &Value, snake: &str, pascal: &str) -> String {
    entity
        .get(snake)
        .and_then(Value::as_str)
        .or_else(|| entity.get(pascal).and_then(Value::as_str))
        .or_else(|| {
            entity
                .get("fields")
                .and_then(|fields| fields.get(snake).or_else(|| fields.get(pascal)))
                .and_then(Value::as_str)
        })
        .unwrap_or("")
        .to_string()
}

fn string_from_fields(fields: &Value, snake: &str, pascal: &str) -> String {
    fields
        .get(snake)
        .and_then(Value::as_str)
        .or_else(|| fields.get(pascal).and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn empty_fallback<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn resolve_api_url(ctx: &Context) -> String {
    ctx.config
        .get("temper_api_url")
        .filter(|value| !value.is_empty() && !value.contains("{secret:"))
        .cloned()
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
}

fn odata_headers(ctx: &Context) -> Vec<(String, String)> {
    vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), ctx.tenant.clone()),
        ("x-temper-principal-kind".to_string(), "agent".to_string()),
        ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
        ("x-temper-agent-type".to_string(), "system".to_string()),
    ]
}

fn configured_session_value(ctx: &Context, key: &str, fallback: &str) -> String {
    ctx.config
        .get(key)
        .filter(|value| !value.trim().is_empty() && !value.contains("{secret:"))
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

fn mock_temper_action_plan(
    tool_call_id: &str,
    entity_set: &str,
    entity_id: &str,
    action_name: &str,
    params: Value,
    final_text: &str,
) -> String {
    let params_json = params.to_string();
    let code = format!(
        "params = json.loads({})\ntemper.action({}, {}, {}, params)",
        json_string_literal(&params_json),
        json_string_literal(entity_set),
        json_string_literal(entity_id),
        json_string_literal(action_name)
    );
    json!({
        "mock_plan": {
            "steps": [
                {
                    "tool_calls": [
                        {
                            "id": tool_call_id,
                            "name": "temper.action",
                            "input": { "code": code }
                        }
                    ]
                },
                {
                    "final_text": final_text
                }
            ]
        }
    })
    .to_string()
}

fn json_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn create_entity(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    path: &str,
) -> Result<String, String> {
    let url = format!("{base_url}{path}");
    let entity_set = path.rsplit('/').next().unwrap_or(path);
    let resp = ctx.http_call("POST", &url, headers, "{}")?;
    let body = parse_json_response(resp, &format!("create {entity_set}"))?;
    entity_id_from_response(&body).ok_or_else(|| format!("create {entity_set}: missing entity_id"))
}

fn post_action(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    entity_set: &str,
    entity_id: &str,
    action_path: &str,
    body: &Value,
) -> Result<Value, String> {
    let url = format!("{base_url}/tdata/{entity_set}('{entity_id}')/{action_path}");
    let resp = ctx.http_call("POST", &url, headers, &body.to_string())?;
    parse_json_response(
        resp,
        &format!("{action_path} on {entity_set}('{entity_id}')"),
    )
}

fn parse_json_response(resp: HttpResponse, label: &str) -> Result<Value, String> {
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "{label} failed with HTTP {}: {}",
            resp.status,
            truncate(&resp.body, 500)
        ));
    }
    if resp.body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&resp.body).map_err(|err| format!("{label}: parse response: {err}"))
}

fn entity_id_from_response(value: &Value) -> Option<String> {
    value
        .get("entity_id")
        .or_else(|| value.get("id"))
        .or_else(|| value.get("Id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn encode_query(input: &str) -> String {
    input
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

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn percent_encode_svg(value: &str) -> String {
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

fn truncate(input: &str, max: usize) -> String {
    if input.len() <= max {
        input.to_string()
    } else {
        format!("{}[truncated]", input.chars().take(max).collect::<String>())
    }
}
