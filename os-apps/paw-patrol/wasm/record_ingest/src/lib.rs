//! record_ingest - stage 3 S0 shadow bridge (ARN-430). TEMPORARY, retired at S3.
//!
//! Fired by ReviewRun.Ingest / ProofPacket.Ingest on a raw GitHub comment body.
//! It parses the comment's base64 record marker and RETURNS a callback: the
//! write action (`IngestRecord` for a review record, `IngestProof` for a proof
//! record) plus the decoded fields as its params. The kernel applies that
//! callback - the state machine writes the fields, not the module. The module
//! creates no entities and makes no OData calls of its own.
//!
//! Gating: the module returns the write action ONLY when the record is
//! well-formed AND the record kind matches the entity it was fired on
//! (a review record on ReviewRun, a proof record on ProofPacket). Otherwise it
//! returns the inert default "callback" action, which the kernel does not
//! dispatch - so a comment with no record, a malformed record, or a record fed
//! to the wrong entity writes nothing and raises no error.
//!
//! "Well-formed" is the ingest boundary the declarative guard grammar cannot
//! express (no string/array predicates; guards cannot read action params):
//! - a 40-character lowercase-hex `commit`;
//! - a review record has a non-empty `reviewers_ran`;
//! - a proof record satisfies the stack proof rules (proof/validate.py):
//!   non-empty `changed_surface`; every changed + blast_radius feature present
//!   in `features[]` with `verification == "rerun"`; `independent_verifier`
//!   agrees and re-ran every changed + blast feature; no feature
//!   `verdict == "fail"`; a `verified-unreachable` feature carries a reason;
//!   every UI feature has screenshots and no failed judgment; `tests.result`
//!   is "pass".

use std::collections::BTreeSet;

use temper_wasm_sdk::prelude::*;

const REVIEW_MARKER: &str = "sdlc-review-record-b64";
const PROOF_MARKER: &str = "sdlc-proof-record-b64";
const CLOSE: &str = "-->";

/// Entity automaton names this module is fired on.
const REVIEW_ENTITY: &str = "ReviewRun";
const PROOF_ENTITY: &str = "ProofPacket";

/// Write actions the module asks the kernel to dispatch.
const INGEST_REVIEW_ACTION: &str = "IngestRecord";
const INGEST_PROOF_ACTION: &str = "IngestProof";
/// The inert default: the kernel does not dispatch a bare "callback".
const NOOP_ACTION: &str = "callback";

/// The decoded result of one comment body.
pub struct ParsedRecord {
    /// "review", "proof", or "none".
    pub kind: &'static str,
    /// The decoded record as JSON (an empty object when none/malformed).
    pub record: Value,
    /// Whether a well-formed, acceptable record was decoded.
    pub parse_ok: bool,
    /// The record's commit sha (empty when none/malformed).
    pub commit: String,
}

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let ctx = match Context::from_host() {
        Ok(c) => c,
        Err(e) => {
            set_error_result(&e.to_string());
            return 0;
        }
    };
    let body = ctx
        .trigger_params
        .get("comment_body")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let parsed = parse_record(body);
    let action = ingest_action(&ctx.entity_type, &parsed);
    if action == NOOP_ACTION {
        // Inert callback: nothing to write, and no error - a comment with no
        // record for this entity is a normal, silent outcome.
        set_success_result(NOOP_ACTION, &json!({}));
    } else {
        set_success_result(action, &write_params(&parsed));
    }
    0
}

/// The write action the kernel should dispatch for this entity + record, or
/// the inert `NOOP_ACTION` when there is nothing valid to write here.
pub fn ingest_action(entity_type: &str, parsed: &ParsedRecord) -> &'static str {
    if !parsed.parse_ok {
        return NOOP_ACTION;
    }
    match (entity_type, parsed.kind) {
        (REVIEW_ENTITY, "review") => INGEST_REVIEW_ACTION,
        (PROOF_ENTITY, "proof") => INGEST_PROOF_ACTION,
        _ => NOOP_ACTION,
    }
}

/// The decoded record flattened into the write action's params. Arrays and
/// objects are serialized to JSON strings to match the entities' string fields;
/// `open_act_on_count` is derived (unresolved act-on findings).
pub fn write_params(parsed: &ParsedRecord) -> Value {
    let r = &parsed.record;
    match parsed.kind {
        "review" => json!({
            "commit": parsed.commit,
            "reviewers_ran": json_string(r.get("reviewers_ran")),
            "findings": json_string(r.get("findings")),
            "risk": r.get("risk").and_then(|v| v.as_str()).unwrap_or(""),
            "open_act_on_count": open_act_on_count(r).to_string(),
        }),
        "proof" => json!({
            "commit": parsed.commit,
            "changed_surface": json_string(r.get("changed_surface")),
            "blast_radius": json_string(r.get("blast_radius")),
            "features": json_string(r.get("features")),
            "tests": json_string(r.get("tests")),
            "independent_verifier": json_string(r.get("independent_verifier")),
        }),
        _ => json!({}),
    }
}

/// Number of unresolved act-on findings (`severity == "act-on"` and not
/// `resolved`).
pub fn open_act_on_count(record: &Value) -> usize {
    record
        .get("findings")
        .and_then(|v| v.as_array())
        .map(|findings| {
            findings
                .iter()
                .filter(|f| {
                    f.get("severity").and_then(|s| s.as_str()) == Some("act-on")
                        && f.get("resolved").and_then(|r| r.as_bool()) != Some(true)
                })
                .count()
        })
        .unwrap_or(0)
}

fn json_string(v: Option<&Value>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

/// Parse a comment body into its record fields. Review marker wins over proof
/// when both are present (they never are in one real comment).
pub fn parse_record(body: &str) -> ParsedRecord {
    if let Some(payload) = extract_payload(body, REVIEW_MARKER) {
        return decode("review", &payload);
    }
    if let Some(payload) = extract_payload(body, PROOF_MARKER) {
        return decode("proof", &payload);
    }
    ParsedRecord {
        kind: "none",
        record: json!({}),
        parse_ok: false,
        commit: String::new(),
    }
}

/// The base64 payload between a `<marker>` tag and the next `-->`, whitespace
/// stripped. `None` when the marker is absent or unterminated.
fn extract_payload(body: &str, marker: &str) -> Option<String> {
    let after_marker = body.find(marker)? + marker.len();
    let rest = &body[after_marker..];
    let end = rest.find(CLOSE)?;
    Some(rest[..end].chars().filter(|c| !c.is_whitespace()).collect())
}

fn decode(kind: &'static str, b64: &str) -> ParsedRecord {
    let failed = ParsedRecord {
        kind,
        record: json!({}),
        parse_ok: false,
        commit: String::new(),
    };
    let bytes = match base64_decode(b64) {
        Some(b) => b,
        None => return failed,
    };
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => return failed,
    };
    let record: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return failed,
    };
    if !record.is_object() {
        return failed;
    }
    let commit = record
        .get("commit")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let parse_ok = is_full_sha(&commit) && record_shape_ok(kind, &record);
    ParsedRecord {
        kind,
        record,
        parse_ok,
        commit,
    }
}

/// A 40-character lowercase-hex git sha.
fn is_full_sha(s: &str) -> bool {
    s.len() == 40
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The record-shape checks the guard grammar cannot express, enforced here at
/// the ingest boundary.
fn record_shape_ok(kind: &str, record: &Value) -> bool {
    match kind {
        "review" => non_empty_array(record.get("reviewers_ran")),
        "proof" => proof_shape_ok(record),
        _ => false,
    }
}

/// The stack proof rules (proof/validate.py), restricted to what a single
/// record can be checked against (the features-dir and URL-evidence rules are
/// CI-only and out of scope here).
fn proof_shape_ok(record: &Value) -> bool {
    let features = match record.get("features").and_then(|v| v.as_array()) {
        Some(f) if !f.is_empty() => f,
        _ => return false,
    };
    let changed = string_set(record.get("changed_surface"));
    if changed.is_empty() {
        return false;
    }
    let blast = string_set(record.get("blast_radius"));

    let mut by_key: std::collections::BTreeMap<&str, &Value> = std::collections::BTreeMap::new();
    for f in features {
        if let Some(key) = f.get("key").and_then(|k| k.as_str()) {
            by_key.insert(key, f);
        }
    }

    // Every changed + blast feature is present and marked rerun.
    for key in changed.iter().chain(blast.iter()) {
        match by_key.get(key.as_str()) {
            None => return false,
            Some(f) => {
                if f.get("verification").and_then(|v| v.as_str()) != Some("rerun") {
                    return false;
                }
            }
        }
    }

    // The independent verifier agrees and re-ran everything changed + blast.
    let iv = match record.get("independent_verifier") {
        Some(v) => v,
        None => return false,
    };
    if iv.get("agrees").and_then(|a| a.as_bool()) != Some(true) {
        return false;
    }
    let reran = string_set(iv.get("reran"));
    if changed
        .iter()
        .chain(blast.iter())
        .any(|k| !reran.contains(k))
    {
        return false;
    }

    // Per-feature: no failing verdict; unreachable needs a reason; UI evidence.
    for f in features {
        match f.get("verdict").and_then(|v| v.as_str()) {
            Some("fail") => return false,
            Some("verified-unreachable") => {
                if f.get("unreachable_reason")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .is_empty()
                {
                    return false;
                }
            }
            _ => {}
        }
        if let Some(ui) = f.get("ui") {
            let has_shots = ui
                .get("screenshots")
                .and_then(|s| s.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            if !has_shots {
                return false;
            }
            for judgment in ["works", "usable", "looks_good"] {
                if ui.get(judgment).and_then(|j| j.as_bool()) == Some(false) {
                    return false;
                }
            }
        }
    }

    // Tests passed.
    record
        .get("tests")
        .and_then(|t| t.get("result"))
        .and_then(|r| r.as_str())
        == Some("pass")
}

fn non_empty_array(v: Option<&Value>) -> bool {
    v.and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
}

fn string_set(v: Option<&Value>) -> BTreeSet<String> {
    v.and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Standard RFC 4648 base64 decode (with `+` / `/` and `=` padding). Returns
/// `None` on any non-alphabet character. Input is expected whitespace-free.
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn sextet(b: u8) -> Option<u32> {
        match b {
            b'A'..=b'Z' => Some((b - b'A') as u32),
            b'a'..=b'z' => Some((b - b'a' + 26) as u32),
            b'0'..=b'9' => Some((b - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes = s.as_bytes();
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1] == b'=' {
        end -= 1;
    }
    let data = &bytes[..end];
    let mut out = Vec::with_capacity(data.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut nbits: u32 = 0;
    for &b in data {
        acc = (acc << 6) | sextet(b)?;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((acc >> nbits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review_480() -> ParsedRecord {
        parse_record(include_str!("../tests/fixtures/pr480_review.txt"))
    }
    fn review_477() -> ParsedRecord {
        parse_record(include_str!("../tests/fixtures/pr477_review.txt"))
    }
    fn proof_480() -> ParsedRecord {
        parse_record(include_str!("../tests/fixtures/pr480_proof.txt"))
    }
    fn proof_477() -> ParsedRecord {
        parse_record(include_str!("../tests/fixtures/pr477_proof.txt"))
    }

    #[test]
    fn pr480_review_decodes_to_the_comment_record() {
        let p = review_480();
        assert_eq!(p.kind, "review");
        assert!(p.parse_ok);
        assert_eq!(p.commit, "27a90bf7c6971263ea9858861d95f58d27e933f5");
        assert_eq!(p.record["effort"], json!("ARN-427"));
        assert_eq!(p.record["reviewers_ran"], json!(["grok", "codex", "fable"]));
    }

    #[test]
    fn pr480_proof_decodes_and_passes_the_proof_rules() {
        let p = proof_480();
        assert_eq!(p.kind, "proof");
        assert!(p.parse_ok);
        assert_eq!(p.commit, "27a90bf7c6971263ea9858861d95f58d27e933f5");
        assert_eq!(p.record["changed_surface"], json!(["boot-and-health"]));
        assert_eq!(p.record["independent_verifier"]["agrees"], json!(true));
    }

    #[test]
    fn pr477_review_decodes_to_the_comment_record() {
        let p = review_477();
        assert_eq!(p.kind, "review");
        assert!(p.parse_ok);
        assert_eq!(p.commit, "a8dba3452a498f271a985098eab77467403bbab7");
        assert_eq!(p.record["effort"], json!("ARN-422"));
    }

    #[test]
    fn pr477_proof_decodes_and_passes_the_proof_rules() {
        let p = proof_477();
        assert_eq!(p.kind, "proof");
        assert!(p.parse_ok);
        assert_eq!(p.commit, "a8dba3452a498f271a985098eab77467403bbab7");
        assert_eq!(p.record["changed_surface"], json!(["genesis-install"]));
        assert_eq!(p.record["blast_radius"], json!(["boot-and-health"]));
    }

    #[test]
    fn malformed_marker_is_not_parse_ok() {
        let p = parse_record(include_str!("../tests/fixtures/malformed_review.txt"));
        assert_eq!(p.kind, "review"); // the marker is detected
        assert!(!p.parse_ok); // but the payload does not decode
        assert_eq!(p.commit, "");
        assert_eq!(p.record, json!({}));
    }

    #[test]
    fn no_marker_is_none() {
        let p = parse_record(include_str!("../tests/fixtures/no_marker.txt"));
        assert_eq!(p.kind, "none");
        assert!(!p.parse_ok);
        assert_eq!(p.commit, "");
    }

    // --- the write callback: action routing + field mapping ---

    #[test]
    fn ingest_action_routes_by_entity_and_kind() {
        assert_eq!(
            ingest_action(REVIEW_ENTITY, &review_480()),
            INGEST_REVIEW_ACTION
        );
        assert_eq!(
            ingest_action(PROOF_ENTITY, &proof_480()),
            INGEST_PROOF_ACTION
        );
        // A record fed to the wrong entity writes nothing.
        assert_eq!(ingest_action(PROOF_ENTITY, &review_480()), NOOP_ACTION);
        assert_eq!(ingest_action(REVIEW_ENTITY, &proof_480()), NOOP_ACTION);
        // An unparseable record writes nothing.
        let none = parse_record(include_str!("../tests/fixtures/no_marker.txt"));
        assert_eq!(ingest_action(REVIEW_ENTITY, &none), NOOP_ACTION);
    }

    #[test]
    fn review_write_params_carry_the_ingestrecord_fields() {
        let params = write_params(&review_477());
        assert_eq!(
            params["commit"],
            json!("a8dba3452a498f271a985098eab77467403bbab7")
        );
        // reviewers_ran is serialized to a JSON string for the string field.
        assert_eq!(
            params["reviewers_ran"],
            json!("[\"grok\",\"codex\",\"fable\"]")
        );
        assert!(params["findings"].as_str().unwrap().starts_with('['));
        assert_eq!(params["risk"], json!("high"));
        // pr477 has one unresolved act-on finding.
        assert_eq!(params["open_act_on_count"], json!("1"));
    }

    #[test]
    fn proof_write_params_carry_the_ingestproof_fields() {
        let params = write_params(&proof_480());
        assert_eq!(
            params["commit"],
            json!("27a90bf7c6971263ea9858861d95f58d27e933f5")
        );
        assert_eq!(params["changed_surface"], json!("[\"boot-and-health\"]"));
        assert!(params["features"].as_str().unwrap().starts_with('['));
        assert!(params["tests"].as_str().unwrap().contains("result"));
        assert!(
            params["independent_verifier"]
                .as_str()
                .unwrap()
                .contains("agrees")
        );
    }

    #[test]
    fn open_act_on_count_matches_the_real_records_and_derivation() {
        assert_eq!(open_act_on_count(&review_480().record), 0); // all consider/nit
        assert_eq!(open_act_on_count(&review_477().record), 1); // one unresolved act-on
        let synthetic = json!({ "findings": [
            { "severity": "act-on", "resolved": false },
            { "severity": "act-on", "resolved": true },
            { "severity": "act-on" },
            { "severity": "consider", "resolved": false },
        ] });
        assert_eq!(open_act_on_count(&synthetic), 2); // two unresolved act-ons
    }

    // --- proof rejections (each aligns with a proof/validate.py rule) ---

    fn good_proof() -> Value {
        json!({
            "commit": "27a90bf7c6971263ea9858861d95f58d27e933f5",
            "changed_surface": ["x"],
            "blast_radius": [],
            "features": [{ "key": "x", "verification": "rerun", "verdict": "pass", "steps": [] }],
            "tests": { "result": "pass" },
            "independent_verifier": { "reran": ["x"], "agrees": true }
        })
    }

    #[test]
    fn good_proof_passes() {
        assert!(proof_shape_ok(&good_proof()));
    }

    #[test]
    fn proof_with_empty_changed_surface_is_refused() {
        let mut r = good_proof();
        r["changed_surface"] = json!([]);
        assert!(!proof_shape_ok(&r));
    }

    #[test]
    fn proof_with_empty_features_is_refused() {
        let mut r = good_proof();
        r["features"] = json!([]);
        assert!(!proof_shape_ok(&r));
    }

    #[test]
    fn proof_with_a_changed_feature_missing_from_features_is_refused() {
        let mut r = good_proof();
        r["changed_surface"] = json!(["x", "y"]);
        assert!(!proof_shape_ok(&r)); // y has no features[] entry
    }

    #[test]
    fn proof_with_a_non_rerun_changed_feature_is_refused() {
        let mut r = good_proof();
        r["features"] =
            json!([{ "key": "x", "verification": "review", "verdict": "pass", "steps": [] }]);
        assert!(!proof_shape_ok(&r));
    }

    #[test]
    fn proof_where_verifier_disagrees_is_refused() {
        let mut r = good_proof();
        r["independent_verifier"]["agrees"] = json!(false);
        assert!(!proof_shape_ok(&r));
    }

    #[test]
    fn proof_where_verifier_did_not_rerun_a_changed_feature_is_refused() {
        let mut r = good_proof();
        r["independent_verifier"]["reran"] = json!([]);
        assert!(!proof_shape_ok(&r));
    }

    #[test]
    fn proof_with_a_failing_feature_is_refused() {
        let mut r = good_proof();
        r["features"] =
            json!([{ "key": "x", "verification": "rerun", "verdict": "fail", "steps": [] }]);
        assert!(!proof_shape_ok(&r));
    }

    #[test]
    fn proof_with_failing_tests_is_refused() {
        let mut r = good_proof();
        r["tests"]["result"] = json!("fail");
        assert!(!proof_shape_ok(&r));
    }

    #[test]
    fn proof_with_unreachable_feature_without_reason_is_refused() {
        let mut r = good_proof();
        r["features"] = json!([{ "key": "x", "verification": "rerun", "verdict": "verified-unreachable", "steps": [] }]);
        assert!(!proof_shape_ok(&r));
        r["features"] = json!([{ "key": "x", "verification": "rerun", "verdict": "verified-unreachable", "unreachable_reason": "no prod path", "steps": [] }]);
        assert!(proof_shape_ok(&r));
    }

    #[test]
    fn proof_with_a_failed_ui_judgment_is_refused() {
        let mut r = good_proof();
        r["features"] = json!([{ "key": "x", "verification": "rerun", "verdict": "pass", "steps": [],
            "ui": { "screenshots": ["http://x"], "works": false, "usable": true, "looks_good": true } }]);
        assert!(!proof_shape_ok(&r));
    }

    #[test]
    fn review_with_no_reviewers_is_refused() {
        let record = json!({
            "commit": "27a90bf7c6971263ea9858861d95f58d27e933f5",
            "reviewers_ran": []
        });
        assert!(!record_shape_ok("review", &record));
    }

    #[test]
    fn short_commit_sha_is_rejected() {
        assert!(is_full_sha("27a90bf7c6971263ea9858861d95f58d27e933f5"));
        assert!(!is_full_sha("33d5ab4c5")); // a real abbreviated sha seen on PR 480
        assert!(!is_full_sha("27A90BF7C6971263EA9858861D95F58D27E933F5")); // uppercase
        assert!(!is_full_sha(""));
    }

    #[test]
    fn base64_decodes_standard_input() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert!(base64_decode("not valid!!!").is_none());
    }
}
