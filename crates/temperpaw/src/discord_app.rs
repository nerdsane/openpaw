use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const DISCORD_API_BASE: &str = "https://discord.com/api/v10";

#[derive(Debug, Deserialize)]
struct CurrentApplicationResponse {
    verify_key: String,
}

pub async fn resolve_verify_key(
    bot_token: &str,
    configured_public_key: Option<&str>,
) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    resolve_verify_key_with_client(&client, DISCORD_API_BASE, bot_token, configured_public_key)
        .await
}

async fn resolve_verify_key_with_client(
    client: &reqwest::Client,
    api_base: &str,
    bot_token: &str,
    configured_public_key: Option<&str>,
) -> Result<String> {
    let configured_public_key = configured_public_key
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());

    match fetch_verify_key_with_client(client, api_base, bot_token).await {
        Ok(verify_key) => {
            if let Some(configured_public_key) = configured_public_key.as_deref()
                && configured_public_key != verify_key
            {
                tracing::warn!(
                    configured_public_key,
                    fetched_verify_key = %verify_key,
                    "Discord public key override differs from Discord's canonical verify_key; using the fetched value"
                );
            }
            Ok(verify_key)
        }
        Err(error) => {
            if let Some(configured_public_key) = configured_public_key {
                tracing::warn!(
                    %error,
                    "Failed to fetch Discord verify_key from the API; falling back to the configured public key"
                );
                Ok(configured_public_key)
            } else {
                Err(error.context(
                    "Discord public key is unavailable and could not be fetched automatically",
                ))
            }
        }
    }
}

async fn fetch_verify_key_with_client(
    client: &reqwest::Client,
    api_base: &str,
    bot_token: &str,
) -> Result<String> {
    let response = client
        .get(format!(
            "{}/oauth2/applications/@me",
            api_base.trim_end_matches('/')
        ))
        .header(reqwest::header::AUTHORIZATION, format!("Bot {bot_token}"))
        .send()
        .await
        .context("Failed to fetch Discord application metadata")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!(
            "Discord application lookup failed with {}: {}",
            status,
            body.trim()
        );
    }

    let payload: CurrentApplicationResponse = response
        .json()
        .await
        .context("Failed to decode Discord application metadata")?;
    let verify_key = payload.verify_key.trim().to_ascii_lowercase();
    if verify_key.is_empty() {
        bail!("Discord application metadata returned an empty verify_key");
    }

    Ok(verify_key)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::Json;
    use axum::Router;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::get;
    use serde_json::json;

    use super::{fetch_verify_key_with_client, resolve_verify_key_with_client};

    #[derive(Clone)]
    struct TestState {
        expected_auth: String,
        verify_key: String,
    }

    async fn current_application(
        State(state): State<TestState>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        let auth = headers
            .get(reqwest::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        if auth != state.expected_auth {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "message": "bad authorization" })),
            );
        }

        (
            StatusCode::OK,
            Json(json!({ "verify_key": state.verify_key })),
        )
    }

    async fn spawn_discord_api(verify_key: &str) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let app = Router::new()
            .route("/oauth2/applications/@me", get(current_application))
            .with_state(TestState {
                expected_auth: "Bot test-token".to_string(),
                verify_key: verify_key.to_string(),
            });
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test API");
        });
        (format!("http://{addr}"), handle)
    }

    #[tokio::test]
    async fn fetch_verify_key_reads_the_canonical_verify_key() {
        let client = reqwest::Client::new();
        let (base_url, handle) =
            spawn_discord_api("ABCDEF1234567890ABCDEF1234567890ABCDEF1234567890ABCDEF1234567890")
                .await;

        let verify_key = fetch_verify_key_with_client(&client, &base_url, "test-token")
            .await
            .expect("fetch verify key");

        handle.abort();
        assert_eq!(
            verify_key,
            "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
        );
    }

    #[tokio::test]
    async fn resolve_verify_key_falls_back_to_configured_override_when_lookup_fails() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(100))
            .build()
            .expect("test client");
        let verify_key = resolve_verify_key_with_client(
            &client,
            "http://127.0.0.1:9",
            "test-token",
            Some("FEDCBA1234567890FEDCBA1234567890FEDCBA1234567890FEDCBA1234567890"),
        )
        .await
        .expect("fallback verify key");

        assert_eq!(
            verify_key,
            "fedcba1234567890fedcba1234567890fedcba1234567890fedcba1234567890"
        );
    }
}
