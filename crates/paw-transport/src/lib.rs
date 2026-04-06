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

impl PawApiClient {
    /// Create a new API client.
    pub fn new(config: PawApiConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
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

        resp.json()
            .await
            .map_err(|e| format!("parse response: {e}"))
    }

    /// GET an arbitrary URL with tenant/auth headers.
    pub async fn raw_get(
        &self,
        url: &str,
    ) -> Result<serde_json::Value, String> {
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
            return Ok(Vec::new());
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
        if let Some(ref key) = self.config.api_key {
            req = req.header("authorization", format!("Bearer {key}"));
        } else {
            req = req.header("x-temper-principal-kind", "admin");
        }
        req
    }
}
pub mod slack;
