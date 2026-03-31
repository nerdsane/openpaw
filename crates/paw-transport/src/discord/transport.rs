//! Discord transport — wires Discord Gateway to Paw Channel entities.
//!
//! On startup: bootstraps the Channel entity used for Discord delivery.
//! On MESSAGE_CREATE: dispatches Channel.ReceiveMessage via OData API.
//! On Channel.SendReply events: delivers reply via Discord REST API.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message;

use super::gateway::*;
use super::types::*;
use crate::PawApiClient;

/// Configuration for the Discord transport.
#[derive(Debug, Clone)]
pub struct DiscordConfig {
    /// Discord bot token.
    pub bot_token: String,
    /// Gateway intents bitmask.
    pub intents: u32,
    /// Port for the webhook listener (receives replies from send_reply WASM).
    /// Defaults to 0 (auto-assign).
    pub webhook_port: u16,
    /// Discord application public key (hex) for interaction signature verification.
    pub public_key: String,
}

/// Discord channel transport.
///
/// Connects to Discord Gateway, dispatches messages to Paw Channel entities
/// via the OData API, and delivers replies via Discord REST API.
pub struct DiscordTransport {
    config: DiscordConfig,
    api: PawApiClient,
    http: reqwest::Client,
    gateway: GatewayState,
    /// Channel entity ID in Paw (populated on startup).
    channel_entity_id: Arc<RwLock<Option<String>>>,
    /// Maps Discord channel_id (DM channel) → user_id for reply routing.
    dm_channels: Arc<RwLock<BTreeMap<String, String>>>,
}

impl DiscordTransport {
    /// Create a new Discord transport.
    pub fn new(config: DiscordConfig, api: PawApiClient) -> Self {
        Self {
            config,
            api,
            http: reqwest::Client::new(),
            gateway: GatewayState::new(),
            channel_entity_id: Arc::new(RwLock::new(None)),
            dm_channels: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Run the transport indefinitely.
    pub async fn run(&self) -> Result<(), String> {
        // Phase 1: Start webhook listener for reply delivery.
        let webhook_port = self.spawn_webhook_listener().await?;
        let webhook_url = format!("http://127.0.0.1:{webhook_port}/reply");
        println!("  [discord] Webhook listener on port {webhook_port}");

        // Phase 2: Bootstrap the Channel entity.
        self.bootstrap_channel(&webhook_url).await?;

        // Phase 3: Connect to Discord Gateway.
        let gateway_url = fetch_gateway_url(&self.http, &self.config.bot_token).await?;
        println!("  [discord] Gateway URL: {gateway_url}");

        // Phase 4: Event loop with reconnection.
        let mut backoff = Duration::from_secs(1);
        let mut url = format!("{gateway_url}/?v=10&encoding=json");

        loop {
            match self.connect_and_run(&url).await {
                Ok(()) => backoff = Duration::from_secs(1),
                Err(e) => {
                    eprintln!("  [discord] Gateway error: {e}");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(60));
                }
            }

            if let Some(resume) = self.gateway.resume_url.read().await.as_ref() {
                url = format!("{resume}/?v=10&encoding=json");
            }

            println!("  [discord] Reconnecting...");
        }
    }

    /// Bootstrap the Channel entity used by the Discord transport.
    ///
    /// Startup seeds the default fallback AgentRoute ahead of time, so the
    /// transport only needs to rotate the Channel entity and keep its
    /// webhook_url in sync with the current listener port.
    async fn bootstrap_channel(&self, webhook_url: &str) -> Result<(), String> {
        // Archive any stale Channel entities from previous runs.
        // The transport creates a fresh Channel each startup so the
        // webhook_url always matches the current listener port.
        let stale = self
            .api
            .query_entities(
                "Channels",
                "ChannelType eq 'discord' and Status ne 'Archived'",
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
                        "ChannelType": "discord",
                    }),
                )
                .await?;
            let id = resp
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            println!("  [discord] Created Channel entity: {id}");

            // Configure the channel with webhook for reply delivery.
            let _ = self
                .api
                .dispatch_action(
                    "Channels",
                    &id,
                    "Paw.Channel.Configure",
                    serde_json::json!({
                        "channel_type": "discord",
                        "channel_id": "discord-gateway",
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

        *self.channel_entity_id.write().await = Some(channel_id.clone());

        Ok(())
    }

    /// Connect to Gateway and run the event loop.
    async fn connect_and_run(&self, url: &str) -> Result<(), String> {
        let (ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|e| format!("WebSocket connect failed: {e}"))?;

        let (mut write, mut read) = ws.split();

        // Wait for Hello (op 10).
        let hello = read_payload(&mut read)
            .await?
            .ok_or("Connection closed before Hello")?;

        if hello.op != GatewayOpcode::Hello as u8 {
            return Err(format!("Expected Hello (op 10), got op {}", hello.op));
        }

        let hello_data: HelloData =
            serde_json::from_value(hello.d.ok_or("Hello missing data field")?)
                .map_err(|e| format!("Failed to parse Hello: {e}"))?;

        let heartbeat_interval = Duration::from_millis(hello_data.heartbeat_interval);

        // Send Identify or Resume.
        let can_resume = self.gateway.session_id.read().await.is_some();
        if can_resume {
            let sid = self.gateway.session_id.read().await.clone().unwrap();
            let seq = self.gateway.sequence.load(Ordering::Relaxed);
            send_resume(&mut write, &self.config.bot_token, &sid, seq).await?;
        } else {
            send_identify(&mut write, &self.config.bot_token, self.config.intents).await?;
        }

        // Send presence.
        let _ = send_presence_online(&mut write).await;

        // Heartbeat ticker.
        let (heartbeat_tx, mut heartbeat_rx) = tokio::sync::mpsc::channel::<()>(1);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(heartbeat_interval);
            loop {
                interval.tick().await;
                if heartbeat_tx.send(()).await.is_err() {
                    break;
                }
            }
        });

        // Main event loop.
        loop {
            tokio::select! {
                frame = read.next() => {
                    let Some(frame) = frame else {
                        return Ok(());
                    };
                    let frame = frame.map_err(|e| format!("WebSocket read error: {e}"))?;
                    let Some(payload) = parse_frame(frame)? else {
                        continue;
                    };
                    let should_reconnect = self.handle_payload(payload).await?;
                    if should_reconnect {
                        return Ok(());
                    }
                }
                Some(()) = heartbeat_rx.recv() => {
                    let s = self.gateway.sequence.load(Ordering::Relaxed);
                    let payload = HeartbeatPayload {
                        op: GatewayOpcode::Heartbeat as u8,
                        d: if s > 0 { Some(s) } else { None },
                    };
                    let json = serde_json::to_string(&payload).unwrap_or_default();
                    write
                        .send(Message::Text(json.into()))
                        .await
                        .map_err(|e| format!("Heartbeat send failed: {e}"))?;
                }
            }
        }
    }

    /// Handle a Gateway payload.
    async fn handle_payload(&self, payload: GatewayPayload) -> Result<bool, String> {
        if let Some(s) = payload.s {
            self.gateway.sequence.store(s, Ordering::Relaxed);
        }

        match GatewayOpcode::from_u8(payload.op) {
            Some(GatewayOpcode::Dispatch) => {
                let event_name = payload.t.as_deref().unwrap_or("");
                match event_name {
                    "READY" => {
                        if let Some(d) = payload.d {
                            handle_ready(&self.gateway, d).await?;
                        }
                    }
                    "MESSAGE_CREATE" => {
                        if let Some(d) = payload.d {
                            self.handle_message_create(d).await;
                        }
                    }
                    _ => {}
                }
                Ok(false)
            }
            Some(GatewayOpcode::HeartbeatAck) => Ok(false),
            Some(GatewayOpcode::Reconnect) => {
                println!("  [discord] Server requested reconnect");
                Ok(true)
            }
            Some(GatewayOpcode::InvalidSession) => {
                let resumable = payload.d.and_then(|v| v.as_bool()).unwrap_or(false);
                if !resumable {
                    *self.gateway.session_id.write().await = None;
                }
                println!("  [discord] Invalid session (resumable={resumable})");
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Handle MESSAGE_CREATE: dispatch Channel.ReceiveMessage via OData API.
    ///
    /// All routing, agent creation, and session management is handled by
    /// the route_message WASM module triggered by Channel.ReceiveMessage.
    async fn handle_message_create(&self, data: serde_json::Value) {
        let msg: MessageCreateData = match serde_json::from_value(data) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("  [discord] Failed to parse MESSAGE_CREATE: {e}");
                return;
            }
        };

        // Ignore bot's own messages.
        let bot_id = self.gateway.bot_user_id.read().await.clone();
        if msg.author.id == bot_id || msg.author.bot {
            return;
        }

        // DMs only for now.
        if msg.guild_id.is_some() {
            return;
        }

        log_message(&msg.author.username, &msg.content);

        // Track DM channel → user mapping for reply delivery.
        self.dm_channels
            .write()
            .await
            .insert(msg.author.id.clone(), msg.channel_id.clone());

        // Send typing indicator.
        send_typing(&self.http, &self.config.bot_token, &msg.channel_id).await;

        // Dispatch Channel.ReceiveMessage — the WASM handles everything else.
        let channel_entity_id = self.channel_entity_id.read().await.clone();
        let Some(channel_id) = channel_entity_id else {
            eprintln!("  [discord] No Channel entity bootstrapped");
            return;
        };

        let params = serde_json::json!({
            "message_id": msg.id,
            "author_id": msg.author.id,
            "thread_id": msg.author.id,  // DMs use author_id as thread
            "content": msg.content,
        });

        match self
            .api
            .dispatch_action(
                "Channels",
                &channel_id,
                "Paw.Channel.ReceiveMessage",
                params,
            )
            .await
        {
            Ok(_) => {
                println!(
                    "  [discord] Dispatched ReceiveMessage for {}",
                    msg.author.username
                );
            }
            Err(e) => {
                eprintln!("  [discord] ReceiveMessage failed: {e}");
                // Send error message to user.
                let _ = send_discord_message(
                    &self.http,
                    &self.config.bot_token,
                    &msg.channel_id,
                    "Sorry, I encountered an error processing your message.",
                )
                .await;
            }
        }
    }

    /// Start a webhook HTTP listener that receives reply callbacks from
    /// the `send_reply` WASM module and interaction callbacks from Discord.
    /// Returns the bound port.
    ///
    /// Routes:
    /// - POST /reply — receives reply callbacks from send_reply/request_approval WASM
    /// - POST /interaction — receives Discord button click interactions
    async fn spawn_webhook_listener(&self) -> Result<u16, String> {
        use axum::{Router, extract::State, routing::post};
        use super::types::*;

        #[derive(Clone)]
        struct WebhookState {
            http: reqwest::Client,
            bot_token: String,
            dm_channels: Arc<RwLock<BTreeMap<String, String>>>,
            api: crate::PawApiClient,
            public_key: String,
        }

        /// Handle reply callbacks from send_reply and request_approval WASM.
        /// Supports optional `components` field for button messages.
        async fn handle_reply(
            State(state): State<WebhookState>,
            axum::Json(body): axum::Json<serde_json::Value>,
        ) -> axum::http::StatusCode {
            let thread_id = body.get("thread_id").and_then(|v| v.as_str()).unwrap_or("");
            let content = body.get("content").and_then(|v| v.as_str()).unwrap_or("");

            if thread_id.is_empty() || content.is_empty() {
                eprintln!("  [discord] Webhook received empty reply (thread={thread_id})");
                return axum::http::StatusCode::BAD_REQUEST;
            }

            // thread_id is the Discord user ID (for DMs). Look up their DM channel.
            let channel_id = state.dm_channels.read().await.get(thread_id).cloned();
            let Some(channel_id) = channel_id else {
                eprintln!("  [discord] No DM channel found for thread_id={thread_id}");
                return axum::http::StatusCode::NOT_FOUND;
            };

            // Check for rich content (components, embeds).
            let components: Vec<ActionRow> = body
                .get("components")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let embeds: Vec<Embed> = body
                .get("embeds")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let has_rich_content = !components.is_empty() || !embeds.is_empty();

            if has_rich_content {
                println!(
                    "  [discord] Delivering rich reply ({} chars, {} components, {} embeds to {})",
                    content.len(),
                    components.len(),
                    embeds.len(),
                    thread_id
                );

                match send_discord_message_with_components(
                    &state.http,
                    &state.bot_token,
                    &channel_id,
                    content,
                    &components,
                    &embeds,
                )
                .await
                {
                    Ok(_msg) => axum::http::StatusCode::OK,
                    Err(e) => {
                        eprintln!("  [discord] Rich reply delivery failed: {e}");
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR
                    }
                }
            } else {
                println!(
                    "  [discord] Delivering reply via webhook ({} chars to {})",
                    content.len(),
                    thread_id
                );

                match send_discord_message(&state.http, &state.bot_token, &channel_id, content)
                    .await
                {
                    Ok(()) => axum::http::StatusCode::OK,
                    Err(e) => {
                        eprintln!("  [discord] Reply delivery failed: {e}");
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR
                    }
                }
            }
        }

        /// Handle Discord interaction webhooks (button clicks).
        ///
        /// Discord sends INTERACTION_CREATE when a user clicks a button.
        /// We verify the Ed25519 signature, respond with a deferred ack,
        /// then dispatch the Temper action asynchronously.
        async fn handle_interaction(
            State(state): State<WebhookState>,
            headers: axum::http::HeaderMap,
            body: axum::body::Bytes,
        ) -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
            // Verify Ed25519 signature
            if !state.public_key.is_empty() {
                let signature = headers
                    .get("x-signature-ed25519")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                let timestamp = headers
                    .get("x-signature-timestamp")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");

                if !verify_discord_signature(&state.public_key, signature, timestamp, &body) {
                    eprintln!("  [discord] Interaction signature verification failed");
                    return (
                        axum::http::StatusCode::UNAUTHORIZED,
                        axum::Json(serde_json::json!({"error": "invalid signature"})),
                    );
                }
            }

            let payload: InteractionPayload = match serde_json::from_slice(&body) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("  [discord] Failed to parse interaction: {e}");
                    return (
                        axum::http::StatusCode::BAD_REQUEST,
                        axum::Json(serde_json::json!({"error": "invalid payload"})),
                    );
                }
            };

            // Type 1 = PING (Discord verification handshake)
            if payload.interaction_type == 1 {
                println!("  [discord] Responding to PING verification");
                return (
                    axum::http::StatusCode::OK,
                    axum::Json(serde_json::json!({ "type": 1 })),
                );
            }

            // Type 3 = MESSAGE_COMPONENT (button click)
            if payload.interaction_type != 3 {
                return (axum::http::StatusCode::OK, axum::Json(serde_json::json!({
                    "type": 4,
                    "data": { "content": "Unsupported interaction type.", "flags": 64 }
                })));
            }

            let Some(ref data) = payload.data else {
                return (axum::http::StatusCode::OK, axum::Json(serde_json::json!({
                    "type": 4,
                    "data": { "content": "No interaction data.", "flags": 64 }
                })));
            };

            let custom_id = &data.custom_id;
            let parts: Vec<&str> = custom_id.splitn(2, ':').collect();
            if parts.len() != 2 {
                return (axum::http::StatusCode::OK, axum::Json(serde_json::json!({
                    "type": 4,
                    "data": { "content": "Invalid button ID.", "flags": 64 }
                })));
            }

            let (action, approval_id) = (parts[0], parts[1]);
            let action_name = match action {
                "approve" => "OpenPaw.Approve",
                "deny" => "OpenPaw.Deny",
                _ => {
                    return (axum::http::StatusCode::OK, axum::Json(serde_json::json!({
                        "type": 4,
                        "data": { "content": "Unknown action.", "flags": 64 }
                    })));
                }
            };

            // Extract reviewer info
            let reviewer_id = payload
                .user
                .as_ref()
                .map(|u| u.id.as_str())
                .or_else(|| {
                    payload
                        .member
                        .as_ref()
                        .and_then(|m| m.get("user"))
                        .and_then(|u| u.get("id"))
                        .and_then(|id| id.as_str())
                })
                .unwrap_or("unknown")
                .to_string();

            let display_action = if action == "approve" {
                "Approved"
            } else {
                "Denied"
            };

            println!(
                "  [discord] Interaction: {action} on PendingApproval {approval_id} by {reviewer_id}"
            );

            // Dispatch the action to Temper asynchronously
            let api = state.api.clone();
            let approval_id_owned = approval_id.to_string();
            let reviewer_id_owned = reviewer_id.clone();
            let token = payload.token.clone();
            let app_id = payload
                .application_id
                .clone()
                .unwrap_or_default();
            let http = state.http.clone();
            let _bot_token = state.bot_token.clone();

            tokio::spawn(async move {
                let body = serde_json::json!({ "reviewer_id": reviewer_id_owned });
                let url = format!(
                    "{}/tdata/PendingApprovals('{}')/{}",
                    api.config().base_url,
                    approval_id_owned,
                    action_name
                );
                let result = api.raw_post(&url, body).await;

                // Send follow-up message via interaction webhook
                let follow_up_content = match result {
                    Ok(_) => format!("**{display_action}** by <@{reviewer_id_owned}>"),
                    Err(ref e) => format!("Failed to process: {e}"),
                };

                if !app_id.is_empty() && !token.is_empty() {
                    let follow_up_url = format!(
                        "https://discord.com/api/v10/webhooks/{app_id}/{token}/messages/@original"
                    );
                    let _ = http
                        .patch(&follow_up_url)
                        .header("Content-Type", "application/json")
                        .json(&serde_json::json!({
                            "content": follow_up_content,
                            "components": []
                        }))
                        .send()
                        .await;
                }
            });

            // Respond with deferred update (type 6 = DEFERRED_UPDATE_MESSAGE)
            // This removes the "thinking" state and we'll edit the message in the spawn above.
            (axum::http::StatusCode::OK, axum::Json(serde_json::json!({ "type": 6 })))
        }

        let webhook_state = WebhookState {
            http: self.http.clone(),
            bot_token: self.config.bot_token.clone(),
            dm_channels: self.dm_channels.clone(),
            api: self.api.clone(),
            public_key: self.config.public_key.clone(),
        };

        /// Handle typing indicator requests from WASM modules.
        async fn handle_typing(
            State(state): State<WebhookState>,
            axum::Json(body): axum::Json<serde_json::Value>,
        ) -> axum::http::StatusCode {
            let thread_id = body.get("thread_id").and_then(|v| v.as_str()).unwrap_or("");
            if thread_id.is_empty() {
                return axum::http::StatusCode::BAD_REQUEST;
            }
            let channel_id = state.dm_channels.read().await.get(thread_id).cloned();
            let Some(channel_id) = channel_id else {
                return axum::http::StatusCode::NOT_FOUND;
            };
            send_typing(&state.http, &state.bot_token, &channel_id).await;
            axum::http::StatusCode::OK
        }

        let app = Router::new()
            .route("/reply", post(handle_reply))
            .route("/interaction", post(handle_interaction))
            .route("/typing", post(handle_typing))
            .with_state(webhook_state);

        let port = self.config.webhook_port;
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
            .await
            .map_err(|e| format!("Failed to bind webhook listener: {e}"))?;
        let actual_port = listener
            .local_addr()
            .map_err(|e| format!("Failed to get listener address: {e}"))?
            .port();

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("  [discord] Webhook listener error: {e}");
            }
        });

        Ok(actual_port)
    }
}

/// Verify a Discord interaction signature using Ed25519.
///
/// Discord sends X-Signature-Ed25519 and X-Signature-Timestamp headers.
/// The signed message is: timestamp + body.
fn verify_discord_signature(
    public_key_hex: &str,
    signature_hex: &str,
    timestamp: &str,
    body: &[u8],
) -> bool {
    use ed25519_dalek::{Signature, VerifyingKey, Verifier};

    let Ok(pk_bytes) = hex::decode(public_key_hex) else {
        return false;
    };
    let pk_bytes: [u8; 32] = match pk_bytes.try_into() {
        Ok(b) => b,
        Err(_) => return false,
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&pk_bytes) else {
        return false;
    };

    let Ok(sig_bytes) = hex::decode(signature_hex) else {
        return false;
    };
    let sig_bytes: [u8; 64] = match sig_bytes.try_into() {
        Ok(b) => b,
        Err(_) => return false,
    };
    let signature = Signature::from_bytes(&sig_bytes);

    let mut message = Vec::with_capacity(timestamp.len() + body.len());
    message.extend_from_slice(timestamp.as_bytes());
    message.extend_from_slice(body);

    verifying_key.verify(&message, &signature).is_ok()
}

/// Read one Gateway payload from the WebSocket with timeout.
async fn read_payload(read: &mut WsStream) -> Result<Option<GatewayPayload>, String> {
    let frame = tokio::time::timeout(Duration::from_secs(60), read.next())
        .await
        .map_err(|_| "Timed out waiting for Gateway payload".to_string())?;
    let Some(frame) = frame else {
        return Ok(None);
    };
    let frame = frame.map_err(|e| format!("WebSocket read error: {e}"))?;
    parse_frame(frame)
}
