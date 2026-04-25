use std::collections::BTreeMap;

use serde_json::Value;

pub const OPENAI_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";
pub const OPENAI_CODEX_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
pub const OPENAI_CODEX_ORIGINATOR: &str = "openpaw";
pub const OPENAI_CODEX_USER_AGENT: &str = "openpaw";

const JWT_CLAIM_PATH: &str = "https://api.openai.com/auth";
const B64_URL_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn select_openai_responses_url(config: &BTreeMap<String, String>, provider: &str) -> String {
    match provider {
        "openai_codex" => config
            .get("openai_codex_api_url")
            .filter(|value| !value.trim().is_empty() && !value.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| OPENAI_CODEX_RESPONSES_URL.to_string()),
        _ => config
            .get("openai_api_url")
            .filter(|value| !value.trim().is_empty() && !value.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| OPENAI_RESPONSES_URL.to_string()),
    }
}

pub fn build_openai_headers(
    provider: &str,
    api_key: &str,
    account_id: Option<&str>,
) -> Vec<(String, String)> {
    let mut headers = vec![
        ("authorization".to_string(), format!("Bearer {api_key}")),
        ("content-type".to_string(), "application/json".to_string()),
    ];

    if provider == "openai_codex" {
        if let Some(account_id) = account_id.filter(|value| !value.trim().is_empty()) {
            headers.push(("chatgpt-account-id".to_string(), account_id.to_string()));
        }
        headers.push(("originator".to_string(), OPENAI_CODEX_ORIGINATOR.to_string()));
        headers.push(("User-Agent".to_string(), OPENAI_CODEX_USER_AGENT.to_string()));
        headers.push((
            "OpenAI-Beta".to_string(),
            "responses=experimental".to_string(),
        ));
        headers.push(("accept".to_string(), "text/event-stream".to_string()));
    }

    headers
}

pub fn extract_chatgpt_account_id_from_jwt(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = base64_url_decode(payload)?;
    let json: Value = serde_json::from_slice(&decoded).ok()?;
    json.get(JWT_CLAIM_PATH)?
        .get("chatgpt_account_id")?
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

pub fn base64_url_no_pad(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut chunks = bytes.chunks_exact(3);
    for chunk in &mut chunks {
        let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | chunk[2] as u32;
        out.push(B64_URL_ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(B64_URL_ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        out.push(B64_URL_ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        out.push(B64_URL_ALPHABET[(n & 0x3f) as usize] as char);
    }

    let rem = chunks.remainder();
    if rem.len() == 1 {
        let n = (rem[0] as u32) << 16;
        out.push(B64_URL_ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(B64_URL_ALPHABET[((n >> 12) & 0x3f) as usize] as char);
    } else if rem.len() == 2 {
        let n = ((rem[0] as u32) << 16) | ((rem[1] as u32) << 8);
        out.push(B64_URL_ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(B64_URL_ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        out.push(B64_URL_ALPHABET[((n >> 6) & 0x3f) as usize] as char);
    }

    out
}

fn base64_url_decode(input: &str) -> Option<Vec<u8>> {
    let mut sextets = Vec::with_capacity(input.len());
    for b in input.bytes() {
        let value = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            b'=' => continue,
            _ => return None,
        };
        sextets.push(value);
    }

    let mut out = Vec::with_capacity(sextets.len() * 3 / 4);
    let mut chunks = sextets.chunks_exact(4);
    for chunk in &mut chunks {
        let n = ((chunk[0] as u32) << 18)
            | ((chunk[1] as u32) << 12)
            | ((chunk[2] as u32) << 6)
            | chunk[3] as u32;
        out.push(((n >> 16) & 0xff) as u8);
        out.push(((n >> 8) & 0xff) as u8);
        out.push((n & 0xff) as u8);
    }

    match chunks.remainder() {
        [] => {}
        [a, b] => {
            let n = ((*a as u32) << 18) | ((*b as u32) << 12);
            out.push(((n >> 16) & 0xff) as u8);
        }
        [a, b, c] => {
            let n = ((*a as u32) << 18) | ((*b as u32) << 12) | ((*c as u32) << 6);
            out.push(((n >> 16) & 0xff) as u8);
            out.push(((n >> 8) & 0xff) as u8);
        }
        _ => return None,
    }

    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_url_round_trips_unpadded_payloads() {
        let value = br#"{"k":"v"}"#;
        let encoded = base64_url_no_pad(value);
        assert!(!encoded.contains('='));
        assert_eq!(base64_url_decode(&encoded).as_deref(), Some(&value[..]));
    }
}
