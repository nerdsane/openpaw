//! Slack transport — wires Slack Socket Mode to Paw Channel entities.
//!
//! On startup: bootstraps the Channel entity used for Slack delivery.
//! On message events: dispatches Channel.ReceiveMessage via OData API.
//! On Channel.SendReply events: delivers reply via Slack Web API.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use super::api;
use super::socket;
use super::types::*;
use crate::PawApiClient;

/// Configuration for the Slack transport.
#[derive(Debug, Clone)]
pub struct SlackConfig {
    /// Slack App-Level Token (xapp-...) for Socket Mode connection.
    pub app_token: String,
    /// Slack Bot Token (xoxb-...) for Web API calls.
    pub bot_token: String,
    /// Port for the webhook listener (receives replies from send_reply WASM).
    /// Defaults to 0 (auto-assign).
    pub webhook_port: u16,
    /// Slack Signing Secret for webhook signature verification (HMAC-SHA256).
    pub signing_secret: String,
}

/// Slack channel transport.
///
/// Connects to Slack via Socket Mode, dispatches messages to Paw Channel
/// entities via the OData API, and delivers replies via Slack Web API.
pub struct SlackTransport {
    config: SlackConfig,
    api: PawApiClient,
    http: reqwest::Client,
    /// Channel entity ID in Paw (populated on startup).
    channel_entity_id: Arc<RwLock<Option<String>>>,
    /// Bot's own user ID (populated after auth.test call).
    bot_user_id: Arc<RwLock<String>>,
}

impl SlackTransport {
    /// Create a new Slack transport.
    pub fn new(config: SlackConfig, api: PawApiClient) -> Self {
        Self {
            config,
            api,
            http: reqwest::Client::new(),
            channel_entity_id: Arc::new(RwLock::new(None)),
            bot_user_id: Arc::new(RwLock::new(String::new())),
        }
    }

    /// Run the transport indefinitely.
    pub async fn run(&self) -> Result<(), String> {
        // Phase 1: Start webhook listener for reply delivery.
        let webhook_port = self.spawn_webhook_listener().await?;
        let webhook_url = format!("http://127.0.0.1:{webhook_port}/reply");
        println!("  [slack] Webhook listener on port {webhook_port}");

        // Phase 2: Bootstrap the Channel entity.
        self.bootstrap_channel(&webhook_url).await?;

        // Phase 3: Get bot user ID via auth.test.
        match api::auth_test(&self.http, &self.config.bot_token).await {
            Ok(resp) => {
                let user_id = resp.user_id.unwrap_or_default();
                let user_name = resp.user.unwrap_or_default();
                println!("  [slack] Authenticated as {user_name} ({user_id})");
                *self.bot_user_id.write().await = user_id;
            }
            Err(e) => {
                return Err(format!("Slack auth.test failed: {e}"));
            }
        }

        // Phase 4: Connect Socket Mode and run event loop with reconnection.
        let mut backoff = Duration::from_secs(1);

        loop {
            let socket_url = match socket::fetch_socket_url(&self.http, &self.config.app_token).await {
                Ok(url) => url,
                Err(e) => {
                    eprintln!("  [slack] Failed to get Socket Mode URL: {e}");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(60));
                    continue;
                }
            };

            match self.connect_and_handle(&socket_url).await {
                Ok(()) => backoff = Duration::from_secs(1),
                Err(e) => {
                    eprintln!("  [slack] Socket Mode error: {e}");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(60));
                }
            }

            println!("  [slack] Reconnecting...");
        }
    }

    /// Bootstrap the Channel entity used by the Slack transport.
    async fn bootstrap_channel(&self, webhook_url: &str) -> Result<(), String> {
        // Archive any stale Channel entities from previous runs.
        let stale = self
            .api
            .query_entities(
                "Channels",
                "ChannelType eq 'slack' and Status ne 'Archived'",
            )
            .await
            .unwrap_or_default();
        for old in &stale {
            if let Some(old_id) = old
                .get("Id")
                .or_else(|| old.get("entity_id"))
                .and_then(|v| v.as_str())
            {
                let _ = self
                    .api
                    .dispatch_action(
                        "Channels",
                        old_id,
                        "Paw.Channel.Archive",
                        serde_json::json!({}),
                    )
                    .await;
            }
        }

        let channel_id = {
            // Create new Channel entity.
            let resp = self
                .api
                .create_entity(
                    "Channels",
                    serde_json::json!({
                        "ChannelType": "slack",
                    }),
                )
                .await?;
            let id = resp
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            println!("  [slack] Created Channel entity: {id}");

            // Configure the channel with webhook for reply delivery.
            let _ = self
                .api
                .dispatch_action(
                    "Channels",
                    &id,
                    "Paw.Channel.Configure",
                    serde_json::json!({
                        "channel_type": "slack",
                        "channel_id": "slack-socket-mode",
                        "webhook_url": webhook_url,
                    }),
                )
                .await;

            // Connect → Ready.
            let _ = self
                .api
                .dispatch_action(
                    "Channels",
                    &id,
                    "Paw.Channel.Connect",
                    serde_json::json!({}),
                )
                .await;

            id
        };

        if channel_id.is_empty() {
            return Err("Failed to bootstrap Channel entity".to_string());
        }

        *self.channel_entity_id.write().await = Some(channel_id);

        Ok(())
    }

    /// Connect to Socket Mode and handle events.
    async fn connect_and_handle(&self, url: &str) -> Result<(), String> {
        let channel_entity_id = self.channel_entity_id.clone();
        let bot_user_id = self.bot_user_id.clone();
        let api = self.api.clone();
        let http = self.http.clone();
        let bot_token = self.config.bot_token.clone();

        socket::connect_and_run(url, |envelope| {
            let channel_entity_id = channel_entity_id.clone();
            let bot_user_id = bot_user_id.clone();
            let api = api.clone();
            let http = http.clone();
            let bot_token = bot_token.clone();

            async move {
                match envelope.envelope_type.as_str() {
                    "events_api" => {
                        handle_events_api(
                            envelope.payload,
                            &channel_entity_id,
                            &bot_user_id,
                            &api,
                            &http,
                            &bot_token,
                        )
                        .await;
                    }
                    "interactive" => {
                        handle_interactive(
                            envelope.payload,
                            &api,
                            &http,
                            &bot_token,
                        )
                        .await;
                    }
                    other => {
                        println!("  [slack] Ignoring envelope type: {other}");
                    }
                }
            }
        })
        .await
    }

    /// Start a webhook HTTP listener that receives reply callbacks from
    /// the `send_reply` WASM module. Returns the bound port.
    async fn spawn_webhook_listener(&self) -> Result<u16, String> {
        use axum::{Router, extract::State, routing::post};

        #[derive(Clone)]
        struct WebhookState {
            http: reqwest::Client,
            bot_token: String,
        }

        /// Handle reply callbacks from send_reply WASM.
        async fn handle_reply(
            State(state): State<WebhookState>,
            axum::Json(body): axum::Json<serde_json::Value>,
        ) -> axum::http::StatusCode {
            let thread_id = body.get("thread_id").and_then(|v| v.as_str()).unwrap_or("");
            let content = body.get("content").and_then(|v| v.as_str()).unwrap_or("");

            if thread_id.is_empty() || content.is_empty() {
                eprintln!("  [slack] Webhook received empty reply (thread={thread_id})");
                return axum::http::StatusCode::BAD_REQUEST;
            }

            // In Slack, thread_id is the channel ID (DM or otherwise).
            // No channel mapping needed — Slack provides the channel directly.
            let channel_id = thread_id;

            // Check for Block Kit blocks.
            let blocks: Vec<SlackBlock> = body
                .get("blocks")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let has_blocks = !blocks.is_empty();

            if has_blocks {
                println!(
                    "  [slack] Delivering rich reply ({} chars, {} blocks to {})",
                    content.len(),
                    blocks.len(),
                    channel_id
                );

                match api::send_slack_message_with_blocks(
                    &state.http,
                    &state.bot_token,
                    channel_id,
                    content,
                    &blocks,
                )
                .await
                {
                    Ok(_) => axum::http::StatusCode::OK,
                    Err(e) => {
                        eprintln!("  [slack] Rich reply delivery failed: {e}");
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR
                    }
                }
            } else {
                println!(
                    "  [slack] Delivering reply ({} chars to {})",
                    content.len(),
                    channel_id
                );

                match api::send_slack_message(
                    &state.http,
                    &state.bot_token,
                    channel_id,
                    content,
                )
                .await
                {
                    Ok(()) => axum::http::StatusCode::OK,
                    Err(e) => {
                        eprintln!("  [slack] Reply delivery failed: {e}");
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR
                    }
                }
            }
        }

        let webhook_state = WebhookState {
            http: self.http.clone(),
            bot_token: self.config.bot_token.clone(),
        };

        let app = Router::new()
            .route("/reply", post(handle_reply))
            .with_state(webhook_state);

        let port = self.config.webhook_port;
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
            .await
            .map_err(|e| format!("Failed to bind Slack webhook listener: {e}"))?;
        let actual_port = listener
            .local_addr()
            .map_err(|e| format!("Failed to get listener address: {e}"))?
            .port();

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("  [slack] Webhook listener error: {e}");
            }
        });

        Ok(actual_port)
    }
}

/// Handle an Events API envelope — extract the inner message event and
/// dispatch Channel.ReceiveMessage.
async fn handle_events_api(
    payload: serde_json::Value,
    channel_entity_id: &Arc<RwLock<Option<String>>>,
    bot_user_id: &Arc<RwLock<String>>,
    api: &PawApiClient,
    http: &reqwest::Client,
    bot_token: &str,
) {
    // Extract the inner event from the Events API wrapper.
    let events_payload: EventsApiPayload = match serde_json::from_value(payload) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("  [slack] Failed to parse Events API payload: {e}");
            return;
        }
    };

    let event_type = events_payload
        .event
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if event_type != "message" {
        return;
    }

    let msg: SlackMessageEvent = match serde_json::from_value(events_payload.event) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("  [slack] Failed to parse message event: {e}");
            return;
        }
    };

    // Skip bot messages and message subtypes (edits, deletes, etc.).
    if msg.subtype.is_some() || msg.bot_id.is_some() {
        return;
    }

    let user_id = match &msg.user {
        Some(u) => u.clone(),
        None => return,
    };

    // Ignore bot's own messages.
    let my_id = bot_user_id.read().await.clone();
    if user_id == my_id {
        return;
    }

    // DMs only for now (channel_type = "im").
    let channel_type = msg.channel_type.as_deref().unwrap_or("");
    if channel_type != "im" {
        return;
    }

    let text = msg.text.unwrap_or_default();
    let channel = msg.channel.unwrap_or_default();
    let ts = msg.ts.unwrap_or_default();

    if text.is_empty() || channel.is_empty() {
        return;
    }

    api::log_message(&user_id, &text);

    // Dispatch Channel.ReceiveMessage — the WASM handles everything else.
    let entity_id = channel_entity_id.read().await.clone();
    let Some(entity_id) = entity_id else {
        eprintln!("  [slack] No Channel entity bootstrapped");
        return;
    };

    // For Slack DMs, use the channel ID as the thread_id.
    // This is the correct channel for reply delivery (no mapping needed).
    let params = serde_json::json!({
        "message_id": ts,
        "author_id": user_id,
        "thread_id": channel,
        "content": text,
    });

    match api
        .dispatch_action(
            "Channels",
            &entity_id,
            "Paw.Channel.ReceiveMessage",
            params,
        )
        .await
    {
        Ok(_) => {
            println!("  [slack] Dispatched ReceiveMessage for {user_id}");
        }
        Err(e) => {
            eprintln!("  [slack] ReceiveMessage failed: {e}");
            // Send error message to user.
            let _ = api::send_slack_message(
                http,
                bot_token,
                &channel,
                "Sorry, I encountered an error processing your message.",
            )
            .await;
        }
    }
}

/// Handle an interactive envelope (button clicks for approve/deny).
async fn handle_interactive(
    payload: serde_json::Value,
    api: &PawApiClient,
    http: &reqwest::Client,
    bot_token: &str,
) {
    let interaction: SlackInteractionPayload = match serde_json::from_value(payload) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("  [slack] Failed to parse interaction payload: {e}");
            return;
        }
    };

    if interaction.interaction_type != "block_actions" {
        return;
    }

    let Some(action) = interaction.actions.first() else {
        return;
    };

    let action_id = &action.action_id;
    let parts: Vec<&str> = action_id.splitn(2, ':').collect();
    if parts.len() != 2 {
        eprintln!("  [slack] Invalid action_id format: {action_id}");
        return;
    }

    let (action_type, decision_id) = (parts[0], parts[1]);
    if action_type != "approve" && action_type != "deny" {
        return;
    }

    let reviewer_id = interaction.user.id.clone();
    let reviewer_name = interaction
        .user
        .name
        .as_deref()
        .or(interaction.user.username.as_deref())
        .unwrap_or("unknown");

    println!(
        "  [slack] Interaction: {action_type} decision {decision_id} by {reviewer_name} ({reviewer_id})"
    );

    let base_url = api.config().base_url.clone();
    let tenant = api.config().tenant.clone();
    let is_approve = action_type == "approve";

    let (success, status_line) = if is_approve {
        let approve_url = format!(
            "{base_url}/api/tenants/{tenant}/decisions/{decision_id}/approve"
        );
        let scope = serde_json::json!({
            "scope": {
                "principal": "this_agent",
                "action": "this_action",
                "resource": "any_of_type",
                "duration": "always"
            },
            "decided_by": format!("slack:{reviewer_id}")
        });
        match api.raw_post(&approve_url, scope).await {
            Ok(_) => (true, format!("Approved by <@{reviewer_id}>")),
            Err(e) => (false, format!("Approval failed: {e}")),
        }
    } else {
        let deny_url = format!(
            "{base_url}/api/tenants/{tenant}/decisions/{decision_id}/deny"
        );
        let deny_body = serde_json::json!({
            "decided_by": format!("slack:{reviewer_id}")
        });
        match api.raw_post(&deny_url, deny_body).await {
            Ok(_) => (true, format!("Denied by <@{reviewer_id}>")),
            Err(e) => (false, format!("Deny failed: {e}")),
        }
    };

    // Resume or fail the agent based on decision.
    if success {
        let filter = format!(
            "pending_decision_id eq '{decision_id}' and Status eq 'WaitingForApproval'"
        );
        let agents_url = format!(
            "{base_url}/tdata/Sessions?$filter={filter}&$top=1"
        );
        if let Ok(agents_resp) = api.raw_get(&agents_url).await {
            if let Some(agent) = agents_resp
                .get("value")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
            {
                let agent_id = agent
                    .get("entity_id")
                    .or_else(|| agent.get("fields").and_then(|f| f.get("Id")))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !agent_id.is_empty() {
                    if is_approve {
                        let resume_url = format!(
                            "{base_url}/tdata/Sessions('{agent_id}')/OpenPaw.ResumeAfterApproval"
                        );
                        match api.raw_post(&resume_url, serde_json::json!({})).await {
                            Ok(_) => println!("  [slack] Resumed agent {agent_id} after approval"),
                            Err(e) => eprintln!("  [slack] Failed to resume agent {agent_id}: {e}"),
                        }
                    } else {
                        let fail_url = format!(
                            "{base_url}/tdata/Sessions('{agent_id}')/OpenPaw.Fail"
                        );
                        let _ = api.raw_post(&fail_url, serde_json::json!({
                            "error_message": "Action denied by human reviewer via Slack"
                        })).await;
                    }
                }
            }
        }
    }

    // Update the original message to show the decision result.
    let channel_id = interaction
        .channel
        .as_ref()
        .map(|c| c.id.as_str())
        .unwrap_or("");
    let message_ts = interaction
        .message
        .as_ref()
        .and_then(|m| m.get("ts"))
        .and_then(|ts| ts.as_str())
        .unwrap_or("");

    if !channel_id.is_empty() && !message_ts.is_empty() {
        // Update with result text and remove buttons.
        let result_blocks = vec![
            SlackBlock::Section {
                text: SlackTextObject::mrkdwn(&status_line),
                block_id: None,
                fields: None,
            },
        ];
        let _ = api::update_slack_message(
            http,
            bot_token,
            channel_id,
            message_ts,
            &status_line,
            Some(&result_blocks),
        )
        .await;
    }
}
