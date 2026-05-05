//! Daily Brief Lifecycle - render the human-readable daily brief.
//!
//! Triggered by `DailyBrief.Start`. This integration collects recent proof
//! packets, completed work cycles, and open risks, then dispatches
//! `DailyBrief.Render` with a visual_daily_brief_svg data URI. It keeps the
//! daily rollup Temper-visible while giving humans and agents one readable page
//! of done items and open risks.

use temper_wasm_sdk::prelude::*;

const PROOF_PACKETS_PATH: &str = "/tdata/ProofPackets";
const QUALITY_FINDINGS_PATH: &str = "/tdata/QualityFindings";
const SECURITY_FINDINGS_PATH: &str = "/tdata/SecurityFindings";
const WORK_CYCLES_PATH: &str = "/tdata/WorkCycles";

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
            other => Err(format!("daily_brief_lifecycle: unsupported trigger {other}")),
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
    let proofs = query_collection(ctx, base_url, headers, PROOF_PACKETS_PATH, "Status eq 'Ready'", 20)?;
    let work_cycles = query_collection(ctx, base_url, headers, WORK_CYCLES_PATH, "Status eq 'Complete'", 20)?;
    let quality_risks = query_collection(ctx, base_url, headers, QUALITY_FINDINGS_PATH, "Status eq 'Open'", 20)?;
    let security_risks =
        query_collection(ctx, base_url, headers, SECURITY_FINDINGS_PATH, "Status eq 'Open'", 20)?;

    let proof_packet_ids = ids_json(&proofs);
    let done_items = done_items_json(&work_cycles, &proofs);
    let open_risks = open_risks_json(&quality_risks, &security_risks);
    let summary_markdown = summary_markdown(
        &brief_date,
        proofs.len(),
        work_cycles.len(),
        quality_risks.len(),
        security_risks.len(),
    );
    let visual_summary_url = visual_daily_brief_svg(
        &brief_date,
        proofs.len(),
        work_cycles.len(),
        quality_risks.len() + security_risks.len(),
    );

    post_action(
        ctx,
        base_url,
        headers,
        "DailyBriefs",
        &brief_id,
        PATROL_RENDER,
        &json!({
            "summary_markdown": summary_markdown,
            "visual_summary_url": visual_summary_url,
            "proof_packet_ids": proof_packet_ids,
            "open_risks": open_risks,
            "done_items": done_items
        }),
    )?;

    ctx.log(
        "info",
        &format!("daily_brief_lifecycle: rendered human-readable daily brief {brief_id}"),
    );
    Ok(())
}

fn query_collection(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    path: &str,
    filter: &str,
    top: usize,
) -> Result<Vec<Value>, String> {
    let url = format!("{base_url}{path}?$filter={}&$top={top}", encode_query(filter));
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

fn summary_markdown(
    brief_date: &str,
    proof_count: usize,
    completed_work_count: usize,
    quality_risk_count: usize,
    security_risk_count: usize,
) -> String {
    format!(
        "# Patrol Daily Brief\n\nDate: {}\n\n## Done items\n\n- Completed WorkCycles: {}\n- Ready ProofPackets: {}\n\n## Open risks\n\n- Quality findings: {}\n- Security findings: {}\n\nThis is the human-readable daily brief for Patrol work. Use the visual summary first, then drill into OData links and ProofPackets when something needs review.",
        empty_fallback(brief_date, "unspecified"),
        completed_work_count,
        proof_count,
        quality_risk_count,
        security_risk_count
    )
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
    parse_json_response(resp, &format!("{action_path} on {entity_set}('{entity_id}')"))
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
