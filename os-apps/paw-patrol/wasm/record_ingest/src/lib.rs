//! record_ingest - stage 3 S0 shadow bridge (ARN-430). TEMPORARY, retired at S3.
//!
//! Parses a raw GitHub comment body's base64 record marker and RETURNS the
//! decoded record fields. It never creates entities and never dispatches
//! transitions - it parses and returns; the state machine's IngestRecord /
//! IngestProof transition writes the fields.
//!
//! Input: `comment_body` (string) from the triggering action's params.
//! Output: `{ kind, record, parse_ok, commit }` where `kind` is
//! "review" | "proof" | "none".
//!
//! `parse_ok` is the ingest boundary: it is true only for a well-formed,
//! acceptable record - the marker decodes, the payload is a JSON object with a
//! 40-character lowercase-hex `commit`, and the record-shape checks that the
//! guard grammar cannot express hold (a review has a non-empty `reviewers_ran`;
//! a proof has a non-empty `changed_surface` and no feature with
//! `verdict == "fail"`). A record that fails any of these cannot become a
//! Recorded run, because the write transition is only dispatched on `parse_ok`.

use temper_wasm_sdk::prelude::*;

const REVIEW_MARKER: &str = "sdlc-review-record-b64";
const PROOF_MARKER: &str = "sdlc-proof-record-b64";
const CLOSE: &str = "-->";

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

temper_module! {
    fn run(ctx: Context) -> Result<Value> {
        let body = ctx
            .trigger_params
            .get("comment_body")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let parsed = parse_record(body);
        Ok(json!({
            "kind": parsed.kind,
            "record": parsed.record,
            "parse_ok": parsed.parse_ok,
            "commit": parsed.commit,
        }))
    }
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
        "proof" => {
            non_empty_array(record.get("changed_surface"))
                && no_failing_feature(record.get("features"))
        }
        _ => false,
    }
}

fn non_empty_array(v: Option<&Value>) -> bool {
    v.and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
}

/// True when no feature carries `verdict == "fail"`. A proof with a failing
/// feature is refused. Absent `features` cannot be inspected, so it does not
/// reject here - `changed_surface` non-empty and the commit sha still gate.
fn no_failing_feature(v: Option<&Value>) -> bool {
    match v.and_then(|v| v.as_array()) {
        None => true,
        Some(features) => features
            .iter()
            .all(|f| f.get("verdict").and_then(|x| x.as_str()) != Some("fail")),
    }
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

    #[test]
    fn pr480_review_decodes_to_the_comment_record() {
        let p = parse_record(include_str!("../tests/fixtures/pr480_review.txt"));
        assert_eq!(p.kind, "review");
        assert!(p.parse_ok);
        assert_eq!(p.commit, "27a90bf7c6971263ea9858861d95f58d27e933f5");
        assert_eq!(p.record["effort"], json!("ARN-427"));
        assert_eq!(p.record["reviewers_ran"], json!(["grok", "codex", "fable"]));
    }

    #[test]
    fn pr480_proof_decodes_to_the_comment_record() {
        let p = parse_record(include_str!("../tests/fixtures/pr480_proof.txt"));
        assert_eq!(p.kind, "proof");
        assert!(p.parse_ok);
        assert_eq!(p.commit, "27a90bf7c6971263ea9858861d95f58d27e933f5");
        assert_eq!(p.record["changed_surface"], json!(["boot-and-health"]));
        assert_eq!(p.record["independent_verifier"]["agrees"], json!(true));
    }

    #[test]
    fn pr477_review_decodes_to_the_comment_record() {
        let p = parse_record(include_str!("../tests/fixtures/pr477_review.txt"));
        assert_eq!(p.kind, "review");
        assert!(p.parse_ok);
        assert_eq!(p.commit, "a8dba3452a498f271a985098eab77467403bbab7");
        assert_eq!(p.record["effort"], json!("ARN-422"));
        assert_eq!(p.record["reviewers_ran"], json!(["grok", "codex", "fable"]));
    }

    #[test]
    fn pr477_proof_decodes_to_the_comment_record() {
        let p = parse_record(include_str!("../tests/fixtures/pr477_proof.txt"));
        assert_eq!(p.kind, "proof");
        assert!(p.parse_ok);
        assert_eq!(p.commit, "a8dba3452a498f271a985098eab77467403bbab7");
        assert_eq!(p.record["changed_surface"], json!(["genesis-install"]));
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

    #[test]
    fn short_commit_sha_is_rejected() {
        assert!(is_full_sha("27a90bf7c6971263ea9858861d95f58d27e933f5"));
        assert!(!is_full_sha("33d5ab4c5")); // a real abbreviated sha seen on PR 480
        assert!(!is_full_sha("27A90BF7C6971263EA9858861D95F58D27E933F5")); // uppercase
        assert!(!is_full_sha(""));
    }

    #[test]
    fn proof_with_empty_changed_surface_is_refused() {
        let record = json!({
            "commit": "27a90bf7c6971263ea9858861d95f58d27e933f5",
            "changed_surface": [],
            "features": []
        });
        assert!(!record_shape_ok("proof", &record));
    }

    #[test]
    fn proof_with_a_failing_feature_is_refused() {
        let record = json!({
            "commit": "27a90bf7c6971263ea9858861d95f58d27e933f5",
            "changed_surface": ["x"],
            "features": [{ "key": "x", "verdict": "fail" }]
        });
        assert!(!record_shape_ok("proof", &record));
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
    fn base64_decodes_standard_input() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert!(base64_decode("not valid!!!").is_none());
    }
}
