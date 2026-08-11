//! Pure functions that assemble OTS trajectory JSON from Session state.
//!
//! Kept separate from `lib.rs` so the mapping logic can be unit-tested without
//! a live Temper context. The resulting JSON shape matches the `OTSTrajectory`
//! serde schema in `temper/crates/temper-ots/src/models/trajectory.rs` and is
//! versioned at "0.1.0".
//!
//! Turn reconstruction (ARN-109): turns come from the SessionEntry tree, not
//! from a synthetic single turn. Every assistant entry closes one LLM cycle;
//! the user / tool-result / steering / compaction entries that precede it are
//! that turn's prompt side. Decisions come from the assistant's `tool_use`
//! blocks, are answered by the `tool_result` blocks of the following turn, and
//! are enriched with wall-clock duration from the per-session tool-span JSONL.
//!
//! Payload discipline: message bodies that already live in TemperFS are emitted
//! as file references, never inlined, and inline text is bounded per message and
//! per trajectory. Inlining full bodies once cost ~300MB of a 491MB database
//! (.proofs/061); the budget below is the guard against a repeat.

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Value of `metadata.harness` — identifies the runtime that produced the run.
pub const HARNESS: &str = "temperpaw";
/// OTS schema version emitted by this module.
pub const OTS_VERSION: &str = "0.1.0";
/// Largest inline text body attached to a single OTS message.
pub const MAX_MESSAGE_INLINE_CHARS: usize = 4_000;
/// Largest total inline text across the whole trajectory document.
pub const MAX_TRAJECTORY_INLINE_CHARS: usize = 64_000;
/// Largest `consequence.result_summary`.
pub const MAX_RESULT_SUMMARY_CHARS: usize = 500;
/// Largest serialized `choice.arguments` payload.
pub const MAX_ARGUMENTS_CHARS: usize = 4_000;
/// Largest inlined system prompt.
pub const MAX_SYSTEM_PROMPT_CHARS: usize = 2_000;
/// Largest task description taken from the user message.
pub const MAX_TASK_DESCRIPTION_CHARS: usize = 500;

const EPOCH: &str = "1970-01-01T00:00:00Z";

/// Everything the emitter knows about a finished session.
pub struct TrajectoryInputs<'a> {
    /// Stable trajectory id (`trj-<session_id>`) — the idempotency key.
    pub trajectory_id: &'a str,
    /// Session entity id.
    pub session_id: &'a str,
    /// Owning agent entity id.
    pub agent_id: &'a str,
    /// Terminal session status (`Completed` / `Failed` / `Cancelled`).
    pub status: &'a str,
    /// Session entity `fields` object.
    pub fields: &'a Value,
    /// Session tree as JSONL (one entry per line), oldest first.
    pub session_jsonl: &'a str,
    /// Per-session tool-span JSONL written by `monty_repl`.
    pub tool_spans_jsonl: &'a str,
    /// Full entity state (used for the event log).
    pub entity_state: &'a Value,
    /// Identity of the governing actor spec (`<app>@<version>`).
    pub spec_version: &'a str,
}

/// Map the Session's terminal state + has_result flag to an OTS `OutcomeType`.
///
/// The OTS enum serializes as snake_case; returned strings match the on-wire
/// representation exactly.
pub fn derive_outcome(status: &str, has_result: bool) -> &'static str {
    match (status, has_result) {
        ("Completed", true) => "success",
        ("Completed", false) => "partial_success",
        ("Failed", _) => "failure",
        ("Cancelled", _) => "partial_success",
        _ => "failure",
    }
}

/// Classify a tool-call error message into an OTS `consequence.error_type`.
///
/// Categories match the conventions described in ADR-0035 decision section 4.
pub fn classify_error(error_text: &str) -> &'static str {
    let lowered = error_text.to_ascii_lowercase();
    if lowered.is_empty() {
        "unknown_error"
    } else if lowered.contains("timeout") || lowered.contains("timed out") {
        "tool_timeout"
    } else if lowered.contains("cedar") || lowered.contains("denied") {
        "cedar_denied"
    } else {
        "tool_error"
    }
}

/// Truncate a string to at most `max_chars` characters (not bytes). Keeps UTF-8 safe.
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

/// Format milliseconds since the Unix epoch as an RFC-3339 UTC timestamp.
///
/// The WASM guests have no chrono dependency, so this implements the civil-date
/// conversion directly. Negative inputs clamp to the epoch — a trajectory with a
/// pre-1970 timestamp is a corrupted clock, not signal worth preserving.
pub fn rfc3339_from_millis(millis: i64) -> String {
    let millis = millis.max(0);
    let total_secs = millis / 1_000;
    let ms = (millis % 1_000) as u32;
    let days = total_secs / 86_400;
    let secs_of_day = total_secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let second = secs_of_day % 60;
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{ms:03}Z"
    )
}

/// Howard Hinnant's `civil_from_days` — days since 1970-01-01 to (y, m, d).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// A parsed session-tree entry (JSONL line or materialized SessionEntry row).
#[derive(Debug, Clone)]
pub struct TreeEntry {
    /// Entry id (`u-*`, `a-*`, `t-*`, `c-*`, `s-*`, `h-*`).
    pub id: String,
    /// Parent entry id, absent for the header.
    pub parent_id: Option<String>,
    /// `header` | `message` | `compaction` | `steering`.
    pub entry_type: String,
    /// `user` | `assistant` | empty.
    pub role: String,
    /// Inline content when the entry was not externalized.
    pub content: Option<Value>,
    /// TemperFS file id when the body was externalized.
    pub content_file_id: Option<String>,
    /// Immutable file version for stable historical reads.
    pub content_file_version_id: Option<String>,
    /// Token estimate recorded with the entry.
    pub tokens: u64,
    /// The whole line, so extras (`ts_ms`, token signals, usage) stay reachable.
    pub raw: Value,
}

impl TreeEntry {
    fn from_value(value: Value) -> Option<Self> {
        let id = value.get("id").and_then(Value::as_str)?.to_string();
        if id.is_empty() {
            return None;
        }
        Some(TreeEntry {
            parent_id: value
                .get("parentId")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            entry_type: value
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("message")
                .to_string(),
            role: value
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            content: value.get("content").cloned(),
            content_file_id: value
                .get("content_file_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            content_file_version_id: value
                .get("content_file_version_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            tokens: value.get("tokens").and_then(Value::as_u64).unwrap_or(0),
            id,
            raw: value,
        })
    }

    fn is_assistant(&self) -> bool {
        self.role == "assistant"
    }

    fn is_header(&self) -> bool {
        self.entry_type == "header"
    }

    /// Wall-clock time the entry was recorded, when the writer stamped one.
    fn recorded_at(&self) -> Option<String> {
        if let Some(ms) = self.raw.get("ts_ms").and_then(json_i64) {
            return Some(rfc3339_from_millis(ms));
        }
        self.raw
            .get("timestamp")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    /// Content blocks when the entry carries an Anthropic-style block array.
    fn blocks(&self) -> Option<&Vec<Value>> {
        self.content.as_ref().and_then(Value::as_array)
    }
}

fn json_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_f64().map(|f| f as i64))
        .or_else(|| value.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
}

fn json_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_f64().filter(|f| *f >= 0.0).map(|f| f as u64))
        .or_else(|| value.as_str().and_then(|s| s.trim().parse::<u64>().ok()))
}

/// Parse session-tree JSONL into ordered entries. Invalid lines are skipped —
/// a corrupted line must not cost the whole trajectory.
pub fn parse_session_entries(session_jsonl: &str) -> Vec<TreeEntry> {
    session_jsonl
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(TreeEntry::from_value)
        .collect()
}

/// Resolve the root→leaf chain the session actually executed.
///
/// Prefers the recorded `session_leaf_id`. When that leaf is missing or its
/// parent chain is broken (continuation/recovery races can leave the Session
/// field ahead of durable rows), falls back to the newest walkable entry and
/// finally to raw file order, so a damaged tree still yields real turns.
pub fn resolve_chain(entries: &[TreeEntry], leaf_id: &str) -> Vec<usize> {
    let mut by_id: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        by_id.insert(entry.id.as_str(), index);
    }

    let walk = |leaf: &str| -> Option<Vec<usize>> {
        let mut chain = Vec::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut cursor = Some(leaf.to_string());
        while let Some(id) = cursor {
            let index = *by_id.get(id.as_str())?;
            if !seen.insert(entries[index].id.as_str()) {
                break; // cycle guard — malformed parent pointer
            }
            chain.push(index);
            cursor = entries[index].parent_id.clone();
        }
        chain.reverse();
        Some(chain)
    };

    let has_message = |chain: &[usize]| chain.iter().any(|i| !entries[*i].is_header());

    if !leaf_id.is_empty()
        && let Some(chain) = walk(leaf_id)
        && has_message(&chain)
    {
        return chain;
    }

    for index in (0..entries.len()).rev() {
        if let Some(chain) = walk(&entries[index].id)
            && has_message(&chain)
        {
            return chain;
        }
    }

    (0..entries.len()).collect()
}

/// One reconstructed LLM cycle: the prompt-side entries plus the assistant
/// entry that closed it. The final turn may have no assistant entry when the
/// session was cancelled or failed mid-cycle.
#[derive(Debug, Clone)]
pub struct TurnDraft {
    /// Indices of prompt-side entries, in order.
    pub prompt: Vec<usize>,
    /// Index of the assistant entry that closed the turn.
    pub assistant: Option<usize>,
}

/// Group a root→leaf chain into turns at assistant-message boundaries.
pub fn group_turns(entries: &[TreeEntry], chain: &[usize]) -> Vec<TurnDraft> {
    let mut turns: Vec<TurnDraft> = Vec::new();
    let mut prompt: Vec<usize> = Vec::new();

    for index in chain {
        let entry = &entries[*index];
        if entry.is_header() {
            continue;
        }
        if entry.is_assistant() {
            turns.push(TurnDraft {
                prompt: std::mem::take(&mut prompt),
                assistant: Some(*index),
            });
        } else {
            prompt.push(*index);
        }
    }

    if !prompt.is_empty() {
        turns.push(TurnDraft {
            prompt,
            assistant: None,
        });
    }

    turns
}

/// Bounded inline-text accounting shared across the whole trajectory.
struct InlineBudget {
    remaining: usize,
}

impl InlineBudget {
    fn new(total: usize) -> Self {
        InlineBudget { remaining: total }
    }

    /// Take up to `per_message_max` characters, respecting the global budget.
    /// Returns the text plus the number of characters dropped.
    fn take(&mut self, text: &str, per_message_max: usize) -> (String, usize) {
        let cap = per_message_max.min(self.remaining);
        let total = text.chars().count();
        if cap >= total {
            self.remaining = self.remaining.saturating_sub(total);
            return (text.to_string(), 0);
        }
        let taken: String = text.chars().take(cap).collect();
        self.remaining = self.remaining.saturating_sub(cap);
        (taken, total - cap)
    }
}

/// A tool call the assistant chose, as recovered from a `tool_use` block.
#[derive(Debug, Clone)]
struct ToolCall {
    id: String,
    name: String,
    arguments: Option<Value>,
}

/// The observation a tool call produced, as recovered from a `tool_result` block.
#[derive(Debug, Clone, Default)]
struct Observation {
    is_error: bool,
    text: String,
}

fn tool_calls_from_entry(entry: &TreeEntry) -> Vec<ToolCall> {
    let Some(blocks) = entry.blocks() else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter_map(|block| {
            let id = block.get("id").and_then(Value::as_str)?.to_string();
            Some(ToolCall {
                name: block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                arguments: block.get("input").cloned(),
                id,
            })
        })
        .collect()
}

fn observations_from_entry(entry: &TreeEntry) -> Vec<(String, Observation)> {
    let Some(blocks) = entry.blocks() else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_result"))
        .filter_map(|block| {
            let id = block.get("tool_use_id").and_then(Value::as_str)?.to_string();
            let text = match block.get("content") {
                Some(Value::String(s)) => s.clone(),
                Some(other) => serde_json::to_string(other).unwrap_or_default(),
                None => String::new(),
            };
            Some((
                id,
                Observation {
                    is_error: block
                        .get("is_error")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    text,
                },
            ))
        })
        .collect()
}

fn bound_arguments(arguments: Option<Value>) -> Option<Value> {
    let arguments = arguments?;
    if arguments.is_null() {
        return None;
    }
    let serialized = serde_json::to_string(&arguments).unwrap_or_default();
    if serialized.chars().count() <= MAX_ARGUMENTS_CHARS {
        return Some(arguments);
    }
    Some(json!({
        "_truncated": true,
        "_original_chars": serialized.chars().count(),
        "_preview": truncate_chars(&serialized, MAX_ARGUMENTS_CHARS),
    }))
}

fn parse_arguments_field(value: Option<&Value>) -> Option<Value> {
    match value {
        Some(Value::String(s)) => Some(
            serde_json::from_str::<Value>(s).unwrap_or_else(|_| Value::String(s.clone())),
        ),
        Some(Value::Null) | None => None,
        Some(other) => Some(other.clone()),
    }
}

/// Build a decision from a tool call plus whatever evidence exists for it.
fn build_decision(
    call: &ToolCall,
    observation: Option<&Observation>,
    span: Option<&Value>,
) -> Value {
    let name = if call.name.is_empty() || call.name == "unknown" {
        span.and_then(|s| s.get("tool_name"))
            .and_then(Value::as_str)
            .unwrap_or(&call.name)
            .to_string()
    } else {
        call.name.clone()
    };

    let arguments = call
        .arguments
        .clone()
        .filter(|value| !value.is_null())
        .or_else(|| parse_arguments_field(span.and_then(|s| s.get("arguments"))));

    let mut choice = json!({ "action": if name.is_empty() { "unknown".to_string() } else { name } });
    if let Some(arguments) = bound_arguments(arguments) {
        choice["arguments"] = arguments;
    }

    let (is_error, result_text) = match observation {
        Some(observation) => (observation.is_error, observation.text.clone()),
        None => (
            span.and_then(|s| s.get("is_error"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            span.and_then(|s| s.get("result"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ),
    };

    let mut consequence = json!({
        "success": !is_error,
        "result_summary": truncate_chars(&result_text, MAX_RESULT_SUMMARY_CHARS),
    });
    if is_error {
        consequence["error_type"] = json!(classify_error(&result_text));
    }

    let mut decision = json!({
        "decision_id": call.id,
        "decision_type": "tool_selection",
        // cause_id links the decision to the observation it caused: the
        // tool_result block carrying the same tool_call_id, which lands in the
        // next turn's prompt side.
        "cause_id": call.id,
        "choice": choice,
        "consequence": consequence,
    });
    if let Some(duration) = span.and_then(|s| s.get("duration_ms")).and_then(json_u64) {
        decision["_duration_ms"] = json!(duration);
    }
    decision
}

/// Convert a single tool-span JSON object (as emitted by
/// `monty_repl::emit_tool_call_telemetry`) into an `OTSDecision` JSON value.
///
/// Used for spans that no assistant entry claims — the session tree body was
/// externalized to TemperFS, so the span is the only surviving evidence of the
/// call. `_duration_ms` is preserved as a non-standard field for the evaluation
/// agents; the OTS schema has no home for tool wall-clock time.
pub fn span_to_decision(span: &Value) -> Value {
    let call = ToolCall {
        id: span
            .get("tool_call_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        name: span
            .get("tool_name")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        arguments: None,
    };
    build_decision(&call, None, Some(span))
}

/// Parse a tool-span JSONL document into span values keyed by tool_call_id,
/// preserving execution order.
///
/// Invalid lines are skipped silently — tool-span persistence is best-effort and
/// the emitter must not fail on a corrupted span.
pub fn parse_tool_spans(tool_spans_jsonl: &str) -> Vec<Value> {
    tool_spans_jsonl
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect()
}

/// Extract first and last event timestamps from the entity event log.
///
/// Returns `(first, last)` as ISO-8601 strings. Falls back to `"1970-01-01T00:00:00Z"`
/// when no events exist — keeps the schema populated with a legal value rather
/// than failing deserialization.
pub fn extract_event_bookends(entity_state: &Value) -> (String, String) {
    let events = entity_state.get("events").and_then(|v| v.as_array());
    let Some(events) = events else {
        return (EPOCH.to_string(), EPOCH.to_string());
    };
    let first = events
        .first()
        .and_then(|e| e.get("timestamp"))
        .and_then(|v| v.as_str())
        .unwrap_or(EPOCH)
        .to_string();
    let last = events
        .last()
        .and_then(|e| e.get("timestamp"))
        .and_then(|v| v.as_str())
        .unwrap_or(EPOCH)
        .to_string();
    (first, last)
}

/// Timestamps of the events that close an LLM cycle, oldest first.
///
/// The entity event log is a hot tail (older events are dropped at snapshot
/// boundaries), so this is a fallback for entries written before per-entry
/// timestamps were recorded — never the primary source.
fn turn_boundary_event_timestamps(entity_state: &Value) -> Vec<String> {
    const CYCLE_CLOSING_ACTIONS: &[&str] = &[
        "ProcessToolCalls",
        "CheckSteering",
        "RecordResult",
        "RecordResultNoReply",
        "RecordResultInlineReply",
    ];
    entity_state
        .get("events")
        .and_then(Value::as_array)
        .map(|events| {
            events
                .iter()
                .filter(|event| {
                    event
                        .get("action")
                        .and_then(Value::as_str)
                        .is_some_and(|action| CYCLE_CLOSING_ACTIONS.contains(&action))
                })
                .filter_map(|event| event.get("timestamp").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn field_str<'a>(fields: &'a Value, key: &str) -> &'a str {
    fields.get(key).and_then(Value::as_str).unwrap_or("")
}

fn message_role(entry: &TreeEntry) -> &'static str {
    if entry.is_assistant() {
        return "assistant";
    }
    if entry.entry_type == "compaction" {
        return "system";
    }
    let has_tool_results = entry
        .blocks()
        .is_some_and(|blocks| {
            blocks
                .iter()
                .any(|block| block.get("type").and_then(Value::as_str) == Some("tool_result"))
        });
    if has_tool_results || entry.id.starts_with("t-") {
        return "tool";
    }
    "user"
}

/// Build the OTS message for one session entry, honoring the inline budget.
fn build_message(entry: &TreeEntry, timestamp: &str, budget: &mut InlineBudget) -> Value {
    let role = message_role(entry);
    let mut message = json!({
        "message_id": entry.id,
        "role": role,
        "timestamp": entry.recorded_at().unwrap_or_else(|| timestamp.to_string()),
    });

    // Externalized bodies are referenced, never fetched and never inlined.
    if let Some(file_id) = &entry.content_file_id {
        let mut data = json!({
            "content_file_id": file_id,
            "externalized": true,
            "tokens": entry.tokens,
        });
        if let Some(version_id) = &entry.content_file_version_id {
            data["content_file_version_id"] = json!(version_id);
        }
        message["content"] = json!({
            "type": content_type_for_role(role),
            "data": data,
        });
        return message;
    }

    let mut content = json!({ "type": "text" });
    let mut data = Map::new();

    match entry.content.clone() {
        Some(Value::String(text)) => {
            let (text, dropped) = budget.take(&text, MAX_MESSAGE_INLINE_CHARS);
            content["text"] = json!(text);
            if dropped > 0 {
                data.insert("truncated_chars".to_string(), json!(dropped));
            }
        }
        Some(Value::Array(blocks)) => {
            let mut text_parts: Vec<String> = Vec::new();
            let mut reasoning_parts: Vec<String> = Vec::new();
            let mut tool_calls: Vec<Value> = Vec::new();
            let mut tool_results: Vec<Value> = Vec::new();

            for block in &blocks {
                match block.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(Value::as_str) {
                            text_parts.push(text.to_string());
                        }
                    }
                    "thinking" | "redacted_thinking" => {
                        if let Some(text) = block
                            .get("thinking")
                            .or_else(|| block.get("text"))
                            .and_then(Value::as_str)
                        {
                            reasoning_parts.push(text.to_string());
                        }
                    }
                    "tool_use" => {
                        let mut call = json!({
                            "id": block.get("id").and_then(Value::as_str).unwrap_or(""),
                            "name": block.get("name").and_then(Value::as_str).unwrap_or(""),
                        });
                        if let Some(arguments) = bound_arguments(block.get("input").cloned()) {
                            call["arguments"] = arguments;
                        }
                        tool_calls.push(call);
                    }
                    "tool_result" => {
                        let text = match block.get("content") {
                            Some(Value::String(s)) => s.clone(),
                            Some(other) => serde_json::to_string(other).unwrap_or_default(),
                            None => String::new(),
                        };
                        let (text, dropped) = budget.take(&text, MAX_MESSAGE_INLINE_CHARS);
                        let mut result = json!({
                            "tool_call_id": block
                                .get("tool_use_id")
                                .and_then(Value::as_str)
                                .unwrap_or(""),
                            "is_error": block
                                .get("is_error")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                            "content": text,
                        });
                        if dropped > 0 {
                            result["truncated_chars"] = json!(dropped);
                        }
                        tool_results.push(result);
                    }
                    _ => {}
                }
            }

            if !text_parts.is_empty() {
                let (text, dropped) = budget.take(&text_parts.join("\n"), MAX_MESSAGE_INLINE_CHARS);
                content["text"] = json!(text);
                if dropped > 0 {
                    data.insert("truncated_chars".to_string(), json!(dropped));
                }
            }
            if !reasoning_parts.is_empty() {
                let (reasoning, _) =
                    budget.take(&reasoning_parts.join("\n"), MAX_MESSAGE_INLINE_CHARS);
                message["reasoning"] = json!(reasoning);
            }
            if !tool_calls.is_empty() {
                content["type"] = json!("tool_call");
                data.insert("tool_calls".to_string(), json!(tool_calls));
            }
            if !tool_results.is_empty() {
                content["type"] = json!("tool_response");
                data.insert("tool_results".to_string(), json!(tool_results));
            }
        }
        Some(other) if !other.is_null() => {
            let serialized = serde_json::to_string(&other).unwrap_or_default();
            let (text, dropped) = budget.take(&serialized, MAX_MESSAGE_INLINE_CHARS);
            content["text"] = json!(text);
            if dropped > 0 {
                data.insert("truncated_chars".to_string(), json!(dropped));
            }
        }
        _ => {}
    }

    // Compaction entries keep their summary in an extra field, not `content`.
    if entry.entry_type == "compaction"
        && let Some(summary) = entry.raw.get("summary").and_then(Value::as_str)
    {
        let (text, dropped) = budget.take(summary, MAX_MESSAGE_INLINE_CHARS);
        content["text"] = json!(text);
        data.insert("compaction".to_string(), json!(true));
        if dropped > 0 {
            data.insert("truncated_chars".to_string(), json!(dropped));
        }
    }

    if !data.is_empty() {
        content["data"] = Value::Object(data);
    }
    message["content"] = content;
    message
}

fn content_type_for_role(role: &str) -> &'static str {
    match role {
        "tool" => "tool_response",
        _ => "text",
    }
}

/// Copy token-id / mask / logprob signals onto the turn when the serving stack
/// recorded them. Absent otherwise — the emitter never fabricates them and never
/// makes a provider round-trip to fetch them.
fn attach_token_signals(turn: &mut Value, source: &Value) {
    for (field, validator) in [
        ("prompt_token_ids", is_u32_array as fn(&Value) -> bool),
        ("completion_token_ids", is_u32_array),
        ("response_mask", is_u8_array),
        ("logprobs", is_f64_array),
    ] {
        if let Some(value) = source.get(field).filter(|value| validator(value)) {
            turn[field] = value.clone();
        }
    }
}

fn is_u32_array(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.iter().all(|item| item.as_u64().is_some_and(|n| n <= u32::MAX as u64)))
}

fn is_u8_array(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.iter().all(|item| item.as_u64().is_some_and(|n| n <= u8::MAX as u64)))
}

fn is_f64_array(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.iter().all(|item| item.as_f64().is_some()))
}

fn file_resource(resources: &mut Vec<Value>, kind: &str, file_id: &str) {
    if file_id.is_empty() {
        return;
    }
    resources.push(json!({
        "type": kind,
        "uri": format!("temperfs://Files('{file_id}')"),
    }));
}

/// Assemble a complete `OTSTrajectory` JSON document.
pub fn build_trajectory(inputs: &TrajectoryInputs<'_>) -> Value {
    let TrajectoryInputs {
        trajectory_id,
        session_id,
        agent_id,
        status,
        fields,
        session_jsonl,
        tool_spans_jsonl,
        entity_state,
        spec_version,
    } = *inputs;

    let (event_start, timestamp_end) = extract_event_bookends(entity_state);
    let entries = parse_session_entries(session_jsonl);
    let chain = resolve_chain(&entries, field_str(fields, "session_leaf_id"));
    let turn_drafts = group_turns(&entries, &chain);

    let timestamp_start = chain
        .iter()
        .find_map(|index| entries[*index].recorded_at())
        .unwrap_or(event_start);

    let has_result = fields
        .get("has_result")
        .and_then(|v| v.as_bool())
        .or_else(|| {
            fields
                .get("has_result")
                .and_then(|v| v.as_str())
                .map(|s| s == "true")
        })
        .unwrap_or(false);
    let outcome = derive_outcome(status, has_result);

    let task_description = truncate_chars(
        field_str(fields, "user_message"),
        MAX_TASK_DESCRIPTION_CHARS,
    );

    let mut tags: Vec<String> = Vec::new();
    for key in ["model", "provider", "session_mode"] {
        let value = field_str(fields, key);
        if !value.is_empty() {
            tags.push(value.to_string());
        }
    }

    // Index every observation on the chain so a decision made in turn N can be
    // answered by the tool_result that lands in turn N+1.
    let mut observations: BTreeMap<String, Observation> = BTreeMap::new();
    let mut turn_of_tool_call: BTreeMap<String, usize> = BTreeMap::new();
    // Observation order preserves execution order for calls the assistant entry
    // could not name (externalized body); BTreeMap iteration would not.
    let mut observed_order: Vec<String> = Vec::new();
    for (turn_index, draft) in turn_drafts.iter().enumerate() {
        if let Some(assistant) = draft.assistant {
            for call in tool_calls_from_entry(&entries[assistant]) {
                turn_of_tool_call.insert(call.id, turn_index);
            }
        }
        for prompt_index in &draft.prompt {
            for (id, observation) in observations_from_entry(&entries[*prompt_index]) {
                // A tool_result on turn N's prompt answers a call made in N-1.
                if turn_index > 0 {
                    turn_of_tool_call.entry(id.clone()).or_insert(turn_index - 1);
                }
                if !observations.contains_key(&id) {
                    observed_order.push(id.clone());
                }
                observations.insert(id, observation);
            }
        }
    }

    let spans = parse_tool_spans(tool_spans_jsonl);
    let mut span_by_id: BTreeMap<String, &Value> = BTreeMap::new();
    for span in &spans {
        if let Some(id) = span.get("tool_call_id").and_then(Value::as_str) {
            span_by_id.insert(id.to_string(), span);
        }
    }

    let boundary_timestamps = turn_boundary_event_timestamps(entity_state);
    let mut budget = InlineBudget::new(MAX_TRAJECTORY_INLINE_CHARS);
    let mut turns: Vec<Value> = Vec::new();
    let mut claimed: BTreeSet<String> = BTreeSet::new();

    for (turn_index, draft) in turn_drafts.iter().enumerate() {
        let assistant = draft.assistant.map(|index| &entries[index]);
        let timestamp = assistant
            .and_then(|entry| entry.recorded_at())
            .or_else(|| boundary_timestamps.get(turn_index).cloned())
            .unwrap_or_else(|| timestamp_start.clone());

        let mut messages: Vec<Value> = Vec::new();
        for prompt_index in &draft.prompt {
            messages.push(build_message(&entries[*prompt_index], &timestamp, &mut budget));
        }
        if let Some(entry) = assistant {
            messages.push(build_message(entry, &timestamp, &mut budget));
        }

        // Decisions the assistant made in this cycle, in call order, plus any
        // call attributed here through its tool_result.
        let mut ordered_ids: Vec<String> = Vec::new();
        let mut calls: BTreeMap<String, ToolCall> = BTreeMap::new();
        if let Some(entry) = assistant {
            for call in tool_calls_from_entry(entry) {
                ordered_ids.push(call.id.clone());
                calls.insert(call.id.clone(), call);
            }
        }
        for span in &spans {
            let Some(id) = span.get("tool_call_id").and_then(Value::as_str) else {
                continue;
            };
            if turn_of_tool_call.get(id) == Some(&turn_index) && !calls.contains_key(id) {
                ordered_ids.push(id.to_string());
                calls.insert(
                    id.to_string(),
                    ToolCall {
                        id: id.to_string(),
                        name: span
                            .get("tool_name")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown")
                            .to_string(),
                        arguments: None,
                    },
                );
            }
        }
        // Calls whose only evidence is the observation (assistant body was
        // externalized and no span exists) still deserve a decision.
        for id in &observed_order {
            if turn_of_tool_call.get(id) == Some(&turn_index) && !calls.contains_key(id) {
                ordered_ids.push(id.clone());
                calls.insert(
                    id.clone(),
                    ToolCall {
                        id: id.clone(),
                        name: "unknown".to_string(),
                        arguments: None,
                    },
                );
            }
        }

        let mut decisions: Vec<Value> = Vec::new();
        let mut turn_error = false;
        let mut turn_duration_ms: u64 = 0;
        for id in &ordered_ids {
            let Some(call) = calls.get(id) else { continue };
            let decision = build_decision(
                call,
                observations.get(id),
                span_by_id.get(id).copied(),
            );
            if decision["consequence"]["success"] == json!(false) {
                turn_error = true;
            }
            if let Some(duration) = decision.get("_duration_ms").and_then(json_u64) {
                turn_duration_ms += duration;
            }
            decisions.push(decision);
            claimed.insert(id.clone());
        }

        let span_id = assistant
            .map(|entry| format!("{session_id}:{}", entry.id))
            .unwrap_or_else(|| format!("{session_id}:turn-{}", turn_index + 1));

        let mut turn = json!({
            "turn_id": (turn_index + 1) as i64,
            "span_id": span_id,
            "timestamp": timestamp,
            "error": turn_error,
            "messages": messages,
            "decisions": decisions,
        });
        if turn_duration_ms > 0 {
            turn["duration_ms"] = json!(turn_duration_ms as f64);
        }

        let prompt_tokens: u64 = assistant
            .and_then(|entry| entry.raw.get("input_tokens").and_then(json_u64))
            .unwrap_or_else(|| {
                draft
                    .prompt
                    .iter()
                    .map(|index| entries[*index].tokens)
                    .sum()
            });
        let completion_tokens: u64 = assistant
            .and_then(|entry| {
                entry
                    .raw
                    .get("output_tokens")
                    .and_then(json_u64)
                    .or(Some(entry.tokens))
            })
            .unwrap_or(0);
        turn["_prompt_tokens"] = json!(prompt_tokens);
        turn["_completion_tokens"] = json!(completion_tokens);

        if let Some(entry) = assistant {
            attach_token_signals(&mut turn, &entry.raw);
        }

        turns.push(turn);
    }

    // Spans no turn claimed (whole tree unavailable, or a call the tree never
    // recorded) still carry real decisions — attach them to the last turn so no
    // evidence is silently dropped.
    let orphan_decisions: Vec<Value> = spans
        .iter()
        .filter(|span| {
            span.get("tool_call_id")
                .and_then(Value::as_str)
                .is_none_or(|id| !claimed.contains(id))
        })
        .map(span_to_decision)
        .collect();

    if !orphan_decisions.is_empty() {
        if turns.is_empty() {
            turns.push(json!({
                "turn_id": 1_i64,
                "span_id": format!("{session_id}:turn-1"),
                "timestamp": timestamp_start,
                "error": orphan_decisions
                    .iter()
                    .any(|d| d["consequence"]["success"] == json!(false)),
                "messages": Vec::<Value>::new(),
                "decisions": orphan_decisions,
            }));
        } else {
            let last = turns.len() - 1;
            if orphan_decisions
                .iter()
                .any(|d| d["consequence"]["success"] == json!(false))
            {
                turns[last]["error"] = json!(true);
            }
            if let Some(existing) = turns[last]["decisions"].as_array_mut() {
                existing.extend(orphan_decisions);
            }
        }
    }

    // Every session produces at least one turn, so an empty tree plus zero
    // spans still yields a schema-valid document rather than an empty array.
    if turns.is_empty() {
        turns.push(json!({
            "turn_id": 1_i64,
            "span_id": format!("{session_id}:turn-1"),
            "timestamp": timestamp_start,
            "error": status == "Failed",
            "messages": Vec::<Value>::new(),
            "decisions": Vec::<Value>::new(),
        }));
    }

    // trajectory_id is duplicated inside `metadata` because Temper's server-side
    // POST handler at temper-server/src/observe/evolution/trajectories.rs reads
    // it from metadata.trajectory_id (not the OTS top-level field). Emitting in
    // both places keeps OTS schema compliance AND lets the Turso row use the same
    // id my module stored on the Session entity — which is what makes
    // INSERT OR REPLACE-based retry idempotency actually work.
    let mut metadata = json!({
        "trajectory_id": trajectory_id,
        "task_description": task_description,
        "domain": "temperpaw-agent",
        "timestamp_start": timestamp_start,
        "timestamp_end": timestamp_end,
        "agent_id": agent_id,
        "framework": "temperpaw",
        "harness": HARNESS,
        "environment": "production",
        "outcome": outcome,
        "tags": tags,
    });
    if !spec_version.is_empty() {
        metadata["spec_version"] = json!(spec_version);
    }

    let mut resources: Vec<Value> = Vec::new();
    file_resource(&mut resources, "session_tree", field_str(fields, "session_file_id"));
    file_resource(&mut resources, "tool_spans", field_str(fields, "tool_spans_file_id"));
    file_resource(
        &mut resources,
        "prepared_context",
        field_str(fields, "prepared_context_file_id"),
    );
    file_resource(
        &mut resources,
        "provider_response",
        field_str(fields, "provider_response_file_id"),
    );
    file_resource(
        &mut resources,
        "system_prompt",
        field_str(fields, "system_prompt_file_id"),
    );

    let mut trajectory = json!({
        "trajectory_id": trajectory_id,
        "version": OTS_VERSION,
        "metadata": metadata,
        "turns": turns,
        "_token_usage": {
            "input_tokens": fields.get("input_tokens").and_then(json_u64).unwrap_or(0),
            "output_tokens": fields.get("output_tokens").and_then(json_u64).unwrap_or(0),
            "context_tokens": fields.get("context_tokens").and_then(json_u64).unwrap_or(0),
        },
        "_session_turn_count": fields.get("turn_count").and_then(json_u64).unwrap_or(0),
    });

    if !resources.is_empty() {
        trajectory["context"] = json!({ "resources": resources });
    }

    let system_prompt = field_str(fields, "system_prompt");
    if !system_prompt.is_empty() {
        // OTSSystemMessage only has { content, timestamp } — no `role` field.
        trajectory["system_message"] = json!({
            "content": truncate_chars(system_prompt, MAX_SYSTEM_PROMPT_CHARS),
            "timestamp": trajectory["metadata"]["timestamp_start"].clone(),
        });
    }

    trajectory
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs<'a>(
        fields: &'a Value,
        session_jsonl: &'a str,
        tool_spans_jsonl: &'a str,
        entity_state: &'a Value,
        status: &'a str,
    ) -> TrajectoryInputs<'a> {
        TrajectoryInputs {
            trajectory_id: "trj-ss-1",
            session_id: "ss-1",
            agent_id: "aj-1",
            status,
            fields,
            session_jsonl,
            tool_spans_jsonl,
            entity_state,
            spec_version: "paw-agent@0.1.0",
        }
    }

    fn entity_state_with_events() -> Value {
        json!({
            "events": [
                { "action": "Created", "timestamp": "2026-01-01T00:00:00Z" },
                { "action": "Cancel",  "timestamp": "2026-01-01T00:00:01Z" },
            ]
        })
    }

    /// Two real LLM cycles: user -> assistant(tool_use) -> tool_result ->
    /// assistant(final text).
    fn two_turn_session_jsonl() -> String {
        let lines = [
            json!({"id":"h-ss-1","parentId":null,"type":"header","tokens":0}),
            json!({
                "id":"u-ss-1-0","parentId":"h-ss-1","type":"message","role":"user",
                "content":"find the bug","tokens":3,"ts_ms":1_767_225_600_000_i64
            }),
            json!({
                "id":"a-1","parentId":"u-ss-1-0","type":"message","role":"assistant",
                "content":[
                    {"type":"thinking","thinking":"I should grep first"},
                    {"type":"text","text":"Looking now"},
                    {"type":"tool_use","id":"tc-1","name":"temper.bash","input":{"cmd":"rg TODO"}}
                ],
                "tokens":42,"ts_ms":1_767_225_601_000_i64,
                "input_tokens":100,"output_tokens":42
            }),
            json!({
                "id":"t-2","parentId":"a-1","type":"message","role":"user",
                "content":[
                    {"type":"tool_result","tool_use_id":"tc-1","content":"src/lib.rs:12: TODO","is_error":false}
                ],
                "tokens":9,"ts_ms":1_767_225_602_000_i64
            }),
            json!({
                "id":"a-3","parentId":"t-2","type":"message","role":"assistant",
                "content":[{"type":"text","text":"Found it on line 12"}],
                "tokens":11,"ts_ms":1_767_225_603_000_i64,
                "input_tokens":160,"output_tokens":11
            }),
        ];
        lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn two_turn_fields() -> Value {
        json!({
            "user_message": "find the bug",
            "model": "claude-sonnet-4-6",
            "provider": "anthropic",
            "session_mode": "execute",
            "has_result": true,
            "session_leaf_id": "a-3",
            "session_file_id": "session-entries:ss-1",
            "tool_spans_file_id": "file-spans-1",
            "turn_count": 2,
            "input_tokens": 260,
            "output_tokens": 53,
            "context_tokens": 160,
        })
    }

    #[test]
    fn derive_outcome_matrix() {
        assert_eq!(derive_outcome("Completed", true), "success");
        assert_eq!(derive_outcome("Completed", false), "partial_success");
        assert_eq!(derive_outcome("Failed", true), "failure");
        assert_eq!(derive_outcome("Failed", false), "failure");
        assert_eq!(derive_outcome("Cancelled", true), "partial_success");
        assert_eq!(derive_outcome("Cancelled", false), "partial_success");
        assert_eq!(derive_outcome("Thinking", true), "failure"); // defensive
    }

    #[test]
    fn classify_error_categories() {
        assert_eq!(classify_error(""), "unknown_error");
        assert_eq!(classify_error("request timed out after 30s"), "tool_timeout");
        assert_eq!(classify_error("Cedar denied action"), "cedar_denied");
        assert_eq!(classify_error("permission denied by policy"), "cedar_denied");
        assert_eq!(classify_error("subprocess failed"), "tool_error");
    }

    #[test]
    fn truncate_chars_respects_utf8() {
        let s = "héllo wörld";
        assert_eq!(truncate_chars(s, 5), "héllo");
        assert_eq!(truncate_chars(s, 100), s);
    }

    #[test]
    fn rfc3339_from_millis_formats_utc() {
        assert_eq!(rfc3339_from_millis(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339_from_millis(-5), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            rfc3339_from_millis(1_767_225_600_000),
            "2026-01-01T00:00:00.000Z"
        );
        // Leap day, with sub-second precision retained.
        assert_eq!(
            rfc3339_from_millis(1_709_209_845_123),
            "2024-02-29T12:30:45.123Z"
        );
    }

    #[test]
    fn span_to_decision_success_shape() {
        let span = json!({
            "tool_name": "temper.read",
            "tool_call_id": "tc-123",
            "arguments": "{\"path\":\"/tmp/foo\"}",
            "result": "ok",
            "duration_ms": 42,
            "is_error": false,
        });
        let dec = span_to_decision(&span);
        assert_eq!(dec["decision_id"], "tc-123");
        assert_eq!(dec["decision_type"], "tool_selection");
        assert_eq!(dec["cause_id"], "tc-123");
        assert_eq!(dec["choice"]["action"], "temper.read");
        assert_eq!(dec["choice"]["arguments"]["path"], "/tmp/foo");
        assert_eq!(dec["consequence"]["success"], true);
        assert_eq!(dec["consequence"]["result_summary"], "ok");
        assert!(dec["consequence"].get("error_type").is_none());
        assert_eq!(dec["_duration_ms"], 42);
    }

    #[test]
    fn span_to_decision_error_classifies() {
        let span = json!({
            "tool_name": "temper.bash",
            "tool_call_id": "tc-err",
            "arguments": "{}",
            "result": "Cedar denied bash execution",
            "duration_ms": 5,
            "is_error": true,
        });
        let dec = span_to_decision(&span);
        assert_eq!(dec["consequence"]["success"], false);
        assert_eq!(dec["consequence"]["error_type"], "cedar_denied");
    }

    #[test]
    fn span_to_decision_keeps_non_json_arguments_as_string() {
        let span = json!({
            "tool_name": "raw",
            "tool_call_id": "tc-raw",
            "arguments": "not-valid-json",
            "result": "",
            "duration_ms": 0,
            "is_error": false,
        });
        let dec = span_to_decision(&span);
        assert_eq!(dec["choice"]["arguments"], "not-valid-json");
    }

    #[test]
    fn span_to_decision_truncates_long_result() {
        let long_result = "x".repeat(1000);
        let span = json!({
            "tool_name": "temper.bash",
            "tool_call_id": "tc-long",
            "arguments": "{}",
            "result": long_result,
            "duration_ms": 1,
            "is_error": false,
        });
        let dec = span_to_decision(&span);
        let summary = dec["consequence"]["result_summary"].as_str().unwrap();
        assert_eq!(summary.chars().count(), MAX_RESULT_SUMMARY_CHARS);
    }

    #[test]
    fn parse_tool_spans_skips_invalid_lines() {
        let jsonl = "{\"tool_call_id\":\"a\",\"tool_name\":\"x\",\"result\":\"\",\"duration_ms\":0,\"is_error\":false}\nINVALID\n{\"tool_call_id\":\"b\",\"tool_name\":\"y\",\"result\":\"\",\"duration_ms\":0,\"is_error\":false}\n";
        let decisions: Vec<Value> = parse_tool_spans(jsonl).iter().map(span_to_decision).collect();
        assert_eq!(decisions.len(), 2);
        assert_eq!(decisions[0]["decision_id"], "a");
        assert_eq!(decisions[1]["decision_id"], "b");
    }

    #[test]
    fn resolve_chain_prefers_recorded_leaf() {
        let entries = parse_session_entries(&two_turn_session_jsonl());
        let chain = resolve_chain(&entries, "a-3");
        let ids: Vec<&str> = chain.iter().map(|i| entries[*i].id.as_str()).collect();
        assert_eq!(ids, vec!["h-ss-1", "u-ss-1-0", "a-1", "t-2", "a-3"]);
    }

    #[test]
    fn resolve_chain_falls_back_when_leaf_is_missing() {
        let entries = parse_session_entries(&two_turn_session_jsonl());
        let chain = resolve_chain(&entries, "a-999-never-written");
        let ids: Vec<&str> = chain.iter().map(|i| entries[*i].id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["h-ss-1", "u-ss-1-0", "a-1", "t-2", "a-3"],
            "a leaf ahead of durable rows must not empty the trajectory"
        );
    }

    #[test]
    fn resolve_chain_survives_parent_cycle() {
        let jsonl = [
            json!({"id":"a","parentId":"b","type":"message","role":"assistant","content":"x"}),
            json!({"id":"b","parentId":"a","type":"message","role":"user","content":"y"}),
        ]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let entries = parse_session_entries(&jsonl);
        let chain = resolve_chain(&entries, "a");
        assert!(chain.len() <= entries.len());
    }

    #[test]
    fn build_trajectory_reconstructs_real_turns() {
        let fields = two_turn_fields();
        let jsonl = two_turn_session_jsonl();
        let spans = "{\"tool_call_id\":\"tc-1\",\"tool_name\":\"temper.bash\",\"arguments\":\"{}\",\"result\":\"src/lib.rs:12: TODO\",\"duration_ms\":137,\"is_error\":false}\n";
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, &jsonl, spans, &state, "Completed"));

        let turns = t["turns"].as_array().unwrap();
        assert_eq!(turns.len(), 2, "two assistant messages means two turns");
        assert_eq!(turns[0]["turn_id"], 1);
        assert_eq!(turns[1]["turn_id"], 2);
        assert_eq!(turns[0]["span_id"], "ss-1:a-1");
        assert_eq!(turns[1]["span_id"], "ss-1:a-3");
        assert_eq!(turns[0]["timestamp"], "2026-01-01T00:00:01.000Z");
        assert_eq!(turns[1]["timestamp"], "2026-01-01T00:00:03.000Z");

        // Turn 1 carries the user prompt plus the assistant reply.
        let turn1_messages = turns[0]["messages"].as_array().unwrap();
        assert_eq!(turn1_messages.len(), 2);
        assert_eq!(turn1_messages[0]["role"], "user");
        assert_eq!(turn1_messages[0]["content"]["text"], "find the bug");
        assert_eq!(turn1_messages[1]["role"], "assistant");
        assert_eq!(turn1_messages[1]["content"]["type"], "tool_call");
        assert_eq!(turn1_messages[1]["content"]["text"], "Looking now");
        assert_eq!(turn1_messages[1]["reasoning"], "I should grep first");

        // Turn 2's prompt side is the tool result observation.
        let turn2_messages = turns[1]["messages"].as_array().unwrap();
        assert_eq!(turn2_messages.len(), 2);
        assert_eq!(turn2_messages[0]["role"], "tool");
        assert_eq!(turn2_messages[0]["content"]["type"], "tool_response");
        assert_eq!(
            turn2_messages[0]["content"]["data"]["tool_results"][0]["tool_call_id"],
            "tc-1"
        );
    }

    #[test]
    fn build_trajectory_populates_decisions_with_cause_id() {
        let fields = two_turn_fields();
        let jsonl = two_turn_session_jsonl();
        let spans = "{\"tool_call_id\":\"tc-1\",\"tool_name\":\"temper.bash\",\"arguments\":\"{}\",\"result\":\"ignored\",\"duration_ms\":137,\"is_error\":false}\n";
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, &jsonl, spans, &state, "Completed"));

        let decisions = t["turns"][0]["decisions"].as_array().unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0]["decision_id"], "tc-1");
        assert_eq!(decisions[0]["cause_id"], "tc-1");
        assert_eq!(decisions[0]["decision_type"], "tool_selection");
        assert_eq!(decisions[0]["choice"]["action"], "temper.bash");
        assert_eq!(decisions[0]["choice"]["arguments"]["cmd"], "rg TODO");
        assert_eq!(decisions[0]["consequence"]["success"], true);
        assert_eq!(
            decisions[0]["consequence"]["result_summary"], "src/lib.rs:12: TODO",
            "the tool_result observation wins over the span result"
        );
        assert_eq!(decisions[0]["_duration_ms"], 137);
        assert_eq!(t["turns"][0]["duration_ms"], 137.0);
        assert_eq!(t["turns"][0]["error"], false);
        assert!(
            t["turns"][1]["decisions"].as_array().unwrap().is_empty(),
            "the final text-only turn makes no tool decisions"
        );
    }

    #[test]
    fn build_trajectory_marks_turn_error_on_failed_tool_call() {
        let jsonl = [
            json!({"id":"u-1","parentId":null,"type":"message","role":"user","content":"go"}),
            json!({
                "id":"a-1","parentId":"u-1","type":"message","role":"assistant",
                "content":[{"type":"tool_use","id":"tc-9","name":"temper.bash","input":{}}]
            }),
            json!({
                "id":"t-2","parentId":"a-1","type":"message","role":"user",
                "content":[{"type":"tool_result","tool_use_id":"tc-9","content":"Cedar denied bash","is_error":true}]
            }),
        ]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let fields = json!({ "session_leaf_id": "t-2", "has_result": false });
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, &jsonl, "", &state, "Failed"));

        assert_eq!(t["turns"][0]["error"], true);
        assert_eq!(t["turns"][0]["decisions"][0]["consequence"]["success"], false);
        assert_eq!(
            t["turns"][0]["decisions"][0]["consequence"]["error_type"],
            "cedar_denied"
        );
    }

    #[test]
    fn build_trajectory_references_externalized_content_instead_of_inlining() {
        let jsonl = [
            json!({"id":"u-1","parentId":null,"type":"message","role":"user","content":"go"}),
            json!({
                "id":"a-1","parentId":"u-1","type":"message","role":"assistant",
                "content_file_id":"file-abc","content_file_version_id":"ver-1","tokens":9000
            }),
        ]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let fields = json!({ "session_leaf_id": "a-1", "has_result": true });
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, &jsonl, "", &state, "Completed"));

        let assistant = &t["turns"][0]["messages"][1];
        assert_eq!(assistant["content"]["data"]["content_file_id"], "file-abc");
        assert_eq!(
            assistant["content"]["data"]["content_file_version_id"],
            "ver-1"
        );
        assert_eq!(assistant["content"]["data"]["externalized"], true);
        assert!(
            assistant["content"].get("text").is_none(),
            "externalized bodies must never be inlined"
        );
    }

    #[test]
    fn build_trajectory_bounds_inline_text() {
        let huge = "x".repeat(MAX_MESSAGE_INLINE_CHARS * 3);
        let jsonl = [
            json!({"id":"u-1","parentId":null,"type":"message","role":"user","content":huge}),
            json!({
                "id":"a-1","parentId":"u-1","type":"message","role":"assistant",
                "content":[{"type":"text","text":"ok"}]
            }),
        ]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let fields = json!({ "session_leaf_id": "a-1", "has_result": true });
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, &jsonl, "", &state, "Completed"));

        let user = &t["turns"][0]["messages"][0];
        let text = user["content"]["text"].as_str().unwrap();
        assert_eq!(text.chars().count(), MAX_MESSAGE_INLINE_CHARS);
        assert_eq!(
            user["content"]["data"]["truncated_chars"],
            json!(MAX_MESSAGE_INLINE_CHARS * 2)
        );
    }

    #[test]
    fn build_trajectory_bounds_total_inline_text_across_messages() {
        let chunk = "y".repeat(MAX_MESSAGE_INLINE_CHARS);
        let mut lines: Vec<Value> = vec![json!({
            "id":"u-0","parentId":null,"type":"message","role":"user","content":"start"
        })];
        let mut parent = "u-0".to_string();
        // 40 assistant/user pairs of full-size bodies far exceed the global budget.
        for index in 1..40 {
            let assistant = format!("a-{index}");
            lines.push(json!({
                "id": assistant, "parentId": parent, "type": "message", "role": "assistant",
                "content": [{"type":"text","text": chunk}]
            }));
            let user = format!("u-{index}");
            lines.push(json!({
                "id": user, "parentId": assistant, "type": "message", "role": "user",
                "content": chunk
            }));
            parent = user;
        }
        let jsonl = lines
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let fields = json!({ "session_leaf_id": parent, "has_result": true });
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, &jsonl, "", &state, "Completed"));

        let inline_chars: usize = t["turns"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|turn| turn["messages"].as_array().unwrap())
            .filter_map(|message| message["content"].get("text").and_then(Value::as_str))
            .map(|text| text.chars().count())
            .sum();
        assert!(
            inline_chars <= MAX_TRAJECTORY_INLINE_CHARS,
            "inline text budget exceeded: {inline_chars}"
        );
    }

    #[test]
    fn build_trajectory_carries_token_signals_when_recorded() {
        let jsonl = [
            json!({"id":"u-1","parentId":null,"type":"message","role":"user","content":"go"}),
            json!({
                "id":"a-1","parentId":"u-1","type":"message","role":"assistant",
                "content":[{"type":"text","text":"done"}],
                "input_tokens": 12, "output_tokens": 3,
                "prompt_token_ids":[1,2,3],
                "completion_token_ids":[4,5],
                "response_mask":[1,1],
                "logprobs":[-0.25,-1.5]
            }),
        ]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let fields = json!({ "session_leaf_id": "a-1", "has_result": true });
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, &jsonl, "", &state, "Completed"));

        let turn = &t["turns"][0];
        assert_eq!(turn["prompt_token_ids"], json!([1, 2, 3]));
        assert_eq!(turn["completion_token_ids"], json!([4, 5]));
        assert_eq!(turn["response_mask"], json!([1, 1]));
        assert_eq!(turn["logprobs"], json!([-0.25, -1.5]));
        assert_eq!(turn["_prompt_tokens"], 12);
        assert_eq!(turn["_completion_tokens"], 3);
    }

    #[test]
    fn build_trajectory_omits_malformed_token_signals() {
        let jsonl = [
            json!({"id":"u-1","parentId":null,"type":"message","role":"user","content":"go"}),
            json!({
                "id":"a-1","parentId":"u-1","type":"message","role":"assistant",
                "content":[{"type":"text","text":"done"}],
                "prompt_token_ids":["not","numbers"],
                "response_mask":[7000],
                "logprobs":"nope"
            }),
        ]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let fields = json!({ "session_leaf_id": "a-1" });
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, &jsonl, "", &state, "Completed"));

        let turn = &t["turns"][0];
        assert!(turn.get("prompt_token_ids").is_none());
        assert!(turn.get("response_mask").is_none());
        assert!(turn.get("logprobs").is_none());
    }

    #[test]
    fn build_trajectory_sets_contract_metadata() {
        let fields = two_turn_fields();
        let jsonl = two_turn_session_jsonl();
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, &jsonl, "", &state, "Completed"));

        assert_eq!(t["trajectory_id"], "trj-ss-1");
        assert_eq!(t["version"], "0.1.0");
        assert_eq!(t["metadata"]["trajectory_id"], "trj-ss-1");
        assert_eq!(t["metadata"]["harness"], "temperpaw");
        assert_eq!(t["metadata"]["spec_version"], "paw-agent@0.1.0");
        assert_eq!(t["metadata"]["framework"], "temperpaw");
        assert_eq!(t["metadata"]["agent_id"], "aj-1");
        assert_eq!(t["metadata"]["outcome"], "success");
        assert_eq!(t["metadata"]["domain"], "temperpaw-agent");
        assert_eq!(t["metadata"]["task_description"], "find the bug");
        assert_eq!(
            t["metadata"]["timestamp_start"], "2026-01-01T00:00:00.000Z",
            "the first entry's own timestamp beats the event hot tail"
        );
        assert_eq!(t["metadata"]["timestamp_end"], "2026-01-01T00:00:01Z");
        let tags = t["metadata"]["tags"].as_array().unwrap();
        assert!(tags.iter().any(|v| v == "claude-sonnet-4-6"));
        assert!(tags.iter().any(|v| v == "anthropic"));
        assert!(tags.iter().any(|v| v == "execute"));
        assert_eq!(t["_session_turn_count"], 2);
        assert_eq!(t["_token_usage"]["input_tokens"], 260);
        assert_eq!(t["_token_usage"]["output_tokens"], 53);
        assert_eq!(t["_token_usage"]["context_tokens"], 160);
    }

    #[test]
    fn build_trajectory_references_session_artifacts_as_resources() {
        let fields = two_turn_fields();
        let jsonl = two_turn_session_jsonl();
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, &jsonl, "", &state, "Completed"));

        let resources = t["context"]["resources"].as_array().unwrap();
        let kinds: Vec<&str> = resources
            .iter()
            .map(|r| r["type"].as_str().unwrap())
            .collect();
        assert!(kinds.contains(&"session_tree"));
        assert!(kinds.contains(&"tool_spans"));
        assert_eq!(
            resources
                .iter()
                .find(|r| r["type"] == "tool_spans")
                .unwrap()["uri"],
            "temperfs://Files('file-spans-1')"
        );
    }

    #[test]
    fn build_trajectory_keeps_unclaimed_spans_as_decisions() {
        // Assistant body externalized: the tree cannot name the tool call, so
        // the span is the only evidence and must still become a decision.
        let jsonl = [
            json!({"id":"u-1","parentId":null,"type":"message","role":"user","content":"go"}),
            json!({
                "id":"a-1","parentId":"u-1","type":"message","role":"assistant",
                "content_file_id":"file-xyz","tokens":900
            }),
        ]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let spans = "{\"tool_call_id\":\"tc-orphan\",\"tool_name\":\"temper.read\",\"arguments\":\"{}\",\"result\":\"ok\",\"duration_ms\":4,\"is_error\":false}\n";
        let fields = json!({ "session_leaf_id": "a-1", "has_result": true });
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, &jsonl, spans, &state, "Completed"));

        let decisions = t["turns"][0]["decisions"].as_array().unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0]["decision_id"], "tc-orphan");
        assert_eq!(decisions[0]["cause_id"], "tc-orphan");
    }

    #[test]
    fn build_trajectory_without_session_tree_falls_back_to_spans() {
        let fields = json!({ "user_message": "x", "has_result": false });
        let spans = "{\"tool_call_id\":\"tc-a\",\"tool_name\":\"read\",\"arguments\":\"{}\",\"result\":\"ok\",\"duration_ms\":2,\"is_error\":false}\n";
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, "", spans, &state, "Cancelled"));

        assert_eq!(t["metadata"]["outcome"], "partial_success");
        let turns = t["turns"].as_array().unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0]["decisions"].as_array().unwrap().len(), 1);
        assert_eq!(turns[0]["decisions"][0]["decision_id"], "tc-a");
    }

    #[test]
    fn build_trajectory_empty_session_still_emits_one_turn() {
        let fields = json!({ "user_message": "", "has_result": false });
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, "", "", &state, "Failed"));

        assert_eq!(t["metadata"]["outcome"], "failure");
        let turns = t["turns"].as_array().unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0]["turn_id"], 1);
        assert_eq!(turns[0]["error"], true);
        assert!(turns[0]["decisions"].as_array().unwrap().is_empty());
        assert!(
            turns[0]["timestamp"].as_str().is_some_and(|s| !s.is_empty()),
            "turn timestamp must be non-empty"
        );
    }

    #[test]
    fn build_trajectory_includes_system_message_when_present() {
        let fields = json!({ "user_message": "", "system_prompt": "you are a helpful agent" });
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, "", "", &state, "Completed"));
        // OTSSystemMessage has { content, timestamp } only — no `role` field.
        assert_eq!(t["system_message"]["content"], "you are a helpful agent");
        assert!(
            t["system_message"]["timestamp"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "system_message.timestamp must be non-empty"
        );
        assert!(
            t["system_message"].get("role").is_none(),
            "role must be omitted"
        );
    }

    #[test]
    fn build_trajectory_skips_system_message_when_empty() {
        let fields = json!({ "user_message": "", "system_prompt": "" });
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, "", "", &state, "Completed"));
        assert!(t.get("system_message").is_none());
    }

    #[test]
    fn build_trajectory_missing_events_emits_epoch_fallback() {
        let fields = json!({ "user_message": "x" });
        let t = build_trajectory(&inputs(&fields, "", "", &json!({}), "Completed"));
        assert_eq!(t["metadata"]["timestamp_start"], "1970-01-01T00:00:00Z");
        assert_eq!(t["metadata"]["timestamp_end"], "1970-01-01T00:00:00Z");
    }

    #[test]
    fn build_trajectory_uses_event_log_when_entries_lack_timestamps() {
        let jsonl = [
            json!({"id":"u-1","parentId":null,"type":"message","role":"user","content":"go"}),
            json!({
                "id":"a-1","parentId":"u-1","type":"message","role":"assistant",
                "content":[{"type":"text","text":"done"}]
            }),
        ]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let state = json!({
            "events": [
                { "action": "Created", "timestamp": "2026-03-01T00:00:00Z" },
                { "action": "RecordResult", "timestamp": "2026-03-01T00:00:09Z" },
            ]
        });
        let fields = json!({ "session_leaf_id": "a-1" });
        let t = build_trajectory(&inputs(&fields, &jsonl, "", &state, "Completed"));
        assert_eq!(t["turns"][0]["timestamp"], "2026-03-01T00:00:09Z");
    }

    #[test]
    fn build_trajectory_bounds_oversized_tool_arguments() {
        let big_argument = "z".repeat(MAX_ARGUMENTS_CHARS * 2);
        let jsonl = [
            json!({"id":"u-1","parentId":null,"type":"message","role":"user","content":"go"}),
            json!({
                "id":"a-1","parentId":"u-1","type":"message","role":"assistant",
                "content":[{"type":"tool_use","id":"tc-big","name":"temper.write","input":{"body":big_argument}}]
            }),
        ]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let fields = json!({ "session_leaf_id": "a-1" });
        let state = entity_state_with_events();
        let t = build_trajectory(&inputs(&fields, &jsonl, "", &state, "Completed"));

        let arguments = &t["turns"][0]["decisions"][0]["choice"]["arguments"];
        assert_eq!(arguments["_truncated"], true);
        assert!(
            arguments["_preview"]
                .as_str()
                .unwrap()
                .chars()
                .count()
                <= MAX_ARGUMENTS_CHARS
        );
    }
}
