//! Platform-agnostic channel transport runtime for Paw.
//!
//! Transports bridge external messaging platforms (Discord, Slack, etc.) to
//! Paw's Channel entity architecture. Each transport is a Paw OData API
//! client — it dispatches `Channel.ReceiveMessage` for inbound messages and
//! watches for `Channel.SendReply` events to deliver outbound replies.
//!
//! No dependency on paw-server internals. Communicates via HTTP only.

pub mod discord;
pub mod slack;
pub mod webhook;

/// Configuration for connecting to a Paw server's OData API.
#[derive(Debug, Clone)]
pub struct PawApiConfig {
    /// Base URL of the Paw server (e.g., "http://127.0.0.1:3467").
    pub base_url: String,
    /// Tenant ID for all OData operations.
    pub tenant: String,
    /// API key for authentication (Bearer token). If empty, uses admin principal.
    pub api_key: Option<String>,
}

/// HTTP client for Paw OData API operations.
///
/// Wraps reqwest::Client with tenant-scoped headers and authentication.
#[derive(Debug, Clone)]
pub struct PawApiClient {
    http: reqwest::Client,
    config: PawApiConfig,
}

pub(crate) fn current_trace_context_ids() -> Option<(String, String)> {
    use opentelemetry::trace::TraceContextExt as _;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;

    let span_context = tracing::Span::current()
        .context()
        .span()
        .span_context()
        .clone();
    if !span_context.is_valid() {
        return None;
    }

    Some((
        span_context.trace_id().to_string(),
        span_context.span_id().to_string(),
    ))
}

fn current_traceparent_header() -> Option<String> {
    use opentelemetry::trace::TraceContextExt as _;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;

    let span_context = tracing::Span::current()
        .context()
        .span()
        .span_context()
        .clone();
    if !span_context.is_valid() {
        return None;
    }

    let flags = if span_context.trace_flags().is_sampled() {
        "01"
    } else {
        "00"
    };
    Some(format!(
        "00-{}-{}-{}",
        span_context.trace_id(),
        span_context.span_id(),
        flags
    ))
}

pub(crate) fn apply_current_trace_context(body: &mut serde_json::Value) {
    let Some(object) = body.as_object_mut() else {
        return;
    };
    let Some((trace_id, span_id)) = current_trace_context_ids() else {
        return;
    };

    object.insert("gen_ai_parent_trace_id".into(), serde_json::json!(trace_id));
    object.insert("gen_ai_parent_span_id".into(), serde_json::json!(span_id));
}

impl PawApiClient {
    /// Create a new API client.
    pub fn new(config: PawApiConfig) -> Self {
        Self {
            http: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(5))
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            config,
        }
    }

    /// Access the API configuration.
    pub fn config(&self) -> &PawApiConfig {
        &self.config
    }

    /// POST to an arbitrary URL with tenant/auth headers.
    pub async fn raw_post(
        &self,
        url: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let resp = self
            .build_request(reqwest::Method::POST, url)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("POST {url} failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("POST {url} returned {status}: {body}"));
        }

        resp.json()
            .await
            .map_err(|e| format!("parse response: {e}"))
    }

    /// GET an arbitrary URL with tenant/auth headers.
    pub async fn raw_get(&self, url: &str) -> Result<serde_json::Value, String> {
        let resp = self
            .build_request(reqwest::Method::GET, url)
            .send()
            .await
            .map_err(|e| format!("GET {url} failed: {e}"))?;

        resp.json()
            .await
            .map_err(|e| format!("parse response: {e}"))
    }

    /// Dispatch a bound action on an entity via OData.
    ///
    /// `action_path` should be the full OData action path including namespace,
    /// e.g. `"Paw.Channel.ReceiveMessage"`.
    pub async fn dispatch_action(
        &self,
        entity_set: &str,
        entity_id: &str,
        action_path: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let url = format!(
            "{}/tdata/{}('{}')/{}",
            self.config.base_url, entity_set, entity_id, action_path
        );
        let resp = self
            .build_request(reqwest::Method::POST, &url)
            .json(&params)
            .send()
            .await
            .map_err(|e| format!("dispatch {action_path} failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("{action_path} returned {status}: {body}"));
        }

        resp.json()
            .await
            .map_err(|e| format!("parse {action_path} response: {e}"))
    }

    /// Create an entity via OData POST.
    pub async fn create_entity(
        &self,
        entity_set: &str,
        fields: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let url = format!("{}/tdata/{}", self.config.base_url, entity_set);
        let resp = self
            .build_request(reqwest::Method::POST, &url)
            .header("content-type", "application/json")
            .json(&fields)
            .send()
            .await
            .map_err(|e| format!("create {entity_set} failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("create {entity_set} returned {status}: {body}"));
        }

        resp.json()
            .await
            .map_err(|e| format!("parse create response: {e}"))
    }

    /// Query entities via OData GET with $filter.
    pub async fn query_entities(
        &self,
        entity_set: &str,
        filter: &str,
    ) -> Result<Vec<serde_json::Value>, String> {
        let url = format!(
            "{}/tdata/{}?$filter={}",
            self.config.base_url, entity_set, filter
        );
        let resp = self
            .build_request(reqwest::Method::GET, &url)
            .send()
            .await
            .map_err(|e| format!("query {entity_set} failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("query {entity_set} returned {status}: {body}"));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("parse query response: {e}"))?;

        Ok(body
            .get("value")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default())
    }

    /// Get a single entity by ID.
    pub async fn get_entity(
        &self,
        entity_set: &str,
        entity_id: &str,
    ) -> Result<serde_json::Value, String> {
        let url = format!(
            "{}/tdata/{}('{}')",
            self.config.base_url, entity_set, entity_id
        );
        let resp = self
            .build_request(reqwest::Method::GET, &url)
            .send()
            .await
            .map_err(|e| format!("get {entity_set}('{entity_id}') failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "get {entity_set}('{entity_id}') returned {status}: {body}"
            ));
        }

        resp.json()
            .await
            .map_err(|e| format!("parse get response: {e}"))
    }

    /// Subscribe to entity state change events via SSE.
    pub async fn subscribe_events(&self) -> Result<reqwest::Response, String> {
        let url = format!("{}/observe/events/stream", self.config.base_url);
        self.build_request(reqwest::Method::GET, &url)
            .header("accept", "text/event-stream")
            .send()
            .await
            .map_err(|e| format!("subscribe events failed: {e}"))
    }

    /// Build a request with tenant and auth headers.
    fn build_request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        let mut req = self.http.request(method, url);
        req = req.header("x-tenant-id", &self.config.tenant);
        if self.uses_internal_loopback(url) {
            req = req.header("x-temper-principal-kind", "admin");
            req = req.header("x-temper-principal-id", "temperpaw-transport");
        } else if let Some(ref key) = self.config.api_key {
            req = req.header("authorization", format!("Bearer {key}"));
        } else {
            req = req.header("x-temper-principal-kind", "admin");
            req = req.header("x-temper-principal-id", "temperpaw-transport");
        }
        if let Some(traceparent) = current_traceparent_header() {
            req = req.header("traceparent", traceparent);
        }
        req
    }

    fn uses_internal_loopback(&self, url: &str) -> bool {
        reqwest::Url::parse(url)
            .ok()
            .and_then(|parsed| parsed.host_str().map(|host| host.to_ascii_lowercase()))
            .map(|host| host == "127.0.0.1" || host == "::1" || host == "localhost")
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use opentelemetry::trace::{TraceContextExt, TracerProvider as _};
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tracing_opentelemetry::OpenTelemetrySpanExt;
    use tracing_subscriber::prelude::*;

    use super::{PawApiClient, PawApiConfig};

    #[derive(Clone, Default)]
    struct HeaderProbe {
        last_kind: Arc<std::sync::Mutex<Option<String>>>,
        last_id: Arc<std::sync::Mutex<Option<String>>>,
        last_tenant: Arc<std::sync::Mutex<Option<String>>>,
        last_auth: Arc<std::sync::Mutex<Option<String>>>,
        last_traceparent: Arc<std::sync::Mutex<Option<String>>>,
    }

    async fn spawn_test_server(app: Router) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{}", addr)
    }

    #[tokio::test]
    async fn paw_api_client_without_api_key_includes_internal_admin_identity() {
        let probe = HeaderProbe::default();
        let app = Router::new()
            .route(
                "/tdata/Channels",
                post(
                    |State(probe): State<HeaderProbe>, headers: HeaderMap| async move {
                        *probe.last_kind.lock().unwrap() = headers
                            .get("x-temper-principal-kind")
                            .and_then(|v| v.to_str().ok())
                            .map(|v| v.to_string());
                        *probe.last_id.lock().unwrap() = headers
                            .get("x-temper-principal-id")
                            .and_then(|v| v.to_str().ok())
                            .map(|v| v.to_string());
                        *probe.last_tenant.lock().unwrap() = headers
                            .get("x-tenant-id")
                            .and_then(|v| v.to_str().ok())
                            .map(|v| v.to_string());
                        *probe.last_auth.lock().unwrap() = headers
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .map(|v| v.to_string());

                        (
                            StatusCode::CREATED,
                            Json(json!({"entity_id":"ch_123","ChannelType":"discord"})),
                        )
                    },
                ),
            )
            .with_state(probe.clone());

        let base_url = spawn_test_server(app).await;
        let client = PawApiClient::new(PawApiConfig {
            base_url,
            tenant: "default".to_string(),
            api_key: None,
        });

        let created = client
            .create_entity("Channels", json!({"ChannelType":"discord"}))
            .await
            .unwrap();

        assert_eq!(
            created.get("entity_id").and_then(|v| v.as_str()),
            Some("ch_123")
        );
        assert_eq!(
            probe.last_kind.lock().unwrap().as_deref(),
            Some("admin"),
            "internal loopback calls should advertise admin principal kind",
        );
        assert_eq!(
            probe.last_id.lock().unwrap().as_deref(),
            Some("temperpaw-transport"),
            "internal loopback calls must include a principal id so auth middleware treats them as pre-authenticated",
        );
        assert_eq!(
            probe.last_tenant.lock().unwrap().as_deref(),
            Some("default"),
        );
        assert_eq!(probe.last_auth.lock().unwrap().as_deref(), None);
    }

    #[tokio::test]
    async fn paw_api_client_with_api_key_still_uses_internal_admin_identity_for_loopback() {
        let probe = HeaderProbe::default();
        let app = Router::new()
            .route(
                "/tdata/Channels",
                post(
                    |State(probe): State<HeaderProbe>, headers: HeaderMap| async move {
                        *probe.last_kind.lock().unwrap() = headers
                            .get("x-temper-principal-kind")
                            .and_then(|v| v.to_str().ok())
                            .map(|v| v.to_string());
                        *probe.last_id.lock().unwrap() = headers
                            .get("x-temper-principal-id")
                            .and_then(|v| v.to_str().ok())
                            .map(|v| v.to_string());
                        *probe.last_tenant.lock().unwrap() = headers
                            .get("x-tenant-id")
                            .and_then(|v| v.to_str().ok())
                            .map(|v| v.to_string());
                        *probe.last_auth.lock().unwrap() = headers
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .map(|v| v.to_string());

                        (
                            StatusCode::CREATED,
                            Json(json!({"entity_id":"ch_456","ChannelType":"discord"})),
                        )
                    },
                ),
            )
            .with_state(probe.clone());

        let base_url = spawn_test_server(app).await;
        let client = PawApiClient::new(PawApiConfig {
            base_url,
            tenant: "default".to_string(),
            api_key: Some("test-token".to_string()),
        });

        let created = client
            .create_entity("Channels", json!({"ChannelType":"discord"}))
            .await
            .unwrap();

        assert_eq!(
            created.get("entity_id").and_then(|v| v.as_str()),
            Some("ch_456")
        );
        assert_eq!(probe.last_kind.lock().unwrap().as_deref(), Some("admin"));
        assert_eq!(
            probe.last_id.lock().unwrap().as_deref(),
            Some("temperpaw-transport")
        );
        assert_eq!(
            probe.last_tenant.lock().unwrap().as_deref(),
            Some("default"),
        );
        assert_eq!(
            probe.last_auth.lock().unwrap().as_deref(),
            None,
            "loopback requests should bypass bearer auth and use internal admin headers",
        );
    }

    #[tokio::test]
    async fn paw_api_query_entities_surfaces_non_success_responses() {
        let app = Router::new().route(
            "/tdata/Channels",
            get(|| async move {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error":"unauthorized"})),
                )
            }),
        );
        let base_url = spawn_test_server(app).await;
        let client = PawApiClient::new(PawApiConfig {
            base_url,
            tenant: "default".to_string(),
            api_key: None,
        });

        let error = client
            .query_entities("Channels", "ChannelType eq 'discord'")
            .await
            .expect_err("401 responses should surface as real bootstrap errors");

        assert!(
            error.contains("query Channels returned 401"),
            "expected status-bearing error, got: {error}"
        );
    }

    #[tokio::test]
    async fn paw_api_raw_post_surfaces_non_success_responses() {
        let app = Router::new().route(
            "/api/tenants/default/decisions/PD-123/approve",
            post(|| async move {
                (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error":{"message":"missing manage_policies"}})),
                )
            }),
        );
        let base_url = spawn_test_server(app).await;
        let client = PawApiClient::new(PawApiConfig {
            base_url,
            tenant: "default".to_string(),
            api_key: Some("test-token".to_string()),
        });

        let error = client
            .raw_post(
                &format!(
                    "{}/api/tenants/default/decisions/PD-123/approve",
                    client.config().base_url
                ),
                json!({"scope":{"principal":"this_agent","action":"this_action","resource":"any_of_type","duration":"always"}}),
            )
            .await
            .expect_err("403 responses should bubble up to the transport");

        assert!(
            error.contains("returned 403"),
            "expected status-bearing error, got: {error}"
        );
        assert!(
            error.contains("missing manage_policies"),
            "expected response body in error, got: {error}"
        );
    }

    #[tokio::test]
    async fn paw_api_client_includes_traceparent_from_active_span() {
        let probe = HeaderProbe::default();
        let app = Router::new()
            .route(
                "/tdata/Channels('ch_trace')/Paw.Channel.ReceiveMessage",
                post(
                    |State(probe): State<HeaderProbe>, headers: HeaderMap| async move {
                        *probe.last_traceparent.lock().unwrap() = headers
                            .get("traceparent")
                            .and_then(|value| value.to_str().ok())
                            .map(|value| value.to_string());
                        (StatusCode::OK, Json(json!({"status":"ok"})))
                    },
                ),
            )
            .with_state(probe.clone());

        let base_url = spawn_test_server(app).await;
        let client = PawApiClient::new(PawApiConfig {
            base_url,
            tenant: "default".to_string(),
            api_key: None,
        });

        let tracer_provider = SdkTracerProvider::builder().build();
        let subscriber = tracing_subscriber::registry().with(
            tracing_opentelemetry::layer()
                .with_tracer(tracer_provider.tracer("paw-transport-test")),
        );
        let _subscriber_guard = tracing::subscriber::set_default(subscriber);

        let span = tracing::info_span!("discord.receive");
        let expected_traceparent = {
            let _span_guard = span.enter();
            let span_context = tracing::Span::current()
                .context()
                .span()
                .span_context()
                .clone();
            let traceparent = format!(
                "00-{}-{}-01",
                span_context.trace_id(),
                span_context.span_id()
            );
            client
                .dispatch_action(
                    "Channels",
                    "ch_trace",
                    "Paw.Channel.ReceiveMessage",
                    json!({
                        "message_id": "msg_123",
                        "author_id": "user_456",
                        "thread_id": "thread_789",
                        "content": "hello",
                    }),
                )
                .await
                .expect("dispatch should succeed");
            traceparent
        };

        assert_eq!(
            probe.last_traceparent.lock().unwrap().as_deref(),
            Some(expected_traceparent.as_str()),
            "expected PawApiClient to propagate the active tracing span via traceparent",
        );

        let _ = tracer_provider.shutdown();
    }
}
