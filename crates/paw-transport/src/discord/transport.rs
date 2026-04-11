//! Discord transport — wires Discord Gateway to Paw Channel entities.
//!
//! On startup: bootstraps the Channel entity used for Discord delivery.
//! On MESSAGE_CREATE: dispatches Channel.ReceiveMessage via OData API.
//! On Channel.SendReply events: delivers reply via Discord REST API.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    /// Discord guild (server) ID for observability channels.
    pub guild_id: Option<String>,
    /// Discord text channel ID for the #feed stream.
    pub feed_channel_id: Option<String>,
    /// Discord forum channel ID for per-agent threads.
    pub forum_channel_id: Option<String>,
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
    /// Last processed Discord message snowflake ID (for catch-up on reconnect).
    last_message_cursor: Arc<RwLock<String>>,
    /// Cancel senders for active typing indicator loops, keyed by thread_id (user ID for DMs).
    /// When a reply is sent, the corresponding cancel sender is triggered to stop the typing loop.
    typing_cancels: Arc<RwLock<BTreeMap<String, tokio::sync::watch::Sender<bool>>>>,
}

fn embed_text_len(embed: &Embed) -> usize {
    let title_len = embed.title.as_ref().map(|value| value.len()).unwrap_or(0);
    let description_len = embed
        .description
        .as_ref()
        .map(|value| value.len())
        .unwrap_or(0);
    let field_len = embed
        .fields
        .as_ref()
        .map(|fields| {
            fields
                .iter()
                .map(|field| field.name.len() + field.value.len())
                .sum::<usize>()
        })
        .unwrap_or(0);
    let footer_len = embed
        .footer
        .as_ref()
        .map(|value| value.text.len())
        .unwrap_or(0);
    title_len + description_len + field_len + footer_len
}

fn embeds_exceed_limits(embeds: &[Embed]) -> bool {
    let total_chars = embeds.iter().map(embed_text_len).sum::<usize>();
    total_chars > 6000
        || embeds.iter().any(|embed| {
            embed
                .title
                .as_ref()
                .map(|value| value.len() > 256)
                .unwrap_or(false)
                || embed
                    .description
                    .as_ref()
                    .map(|value| value.len() > 4096)
                    .unwrap_or(false)
                || embed
                    .footer
                    .as_ref()
                    .map(|value| value.text.len() > 2048)
                    .unwrap_or(false)
                || embed
                    .fields
                    .as_ref()
                    .map(|fields| {
                        fields.len() > 25
                            || fields
                                .iter()
                                .any(|field| field.name.len() > 256 || field.value.len() > 1024)
                    })
                    .unwrap_or(false)
        })
}

fn flatten_embeds_for_plain_text(embeds: &[Embed]) -> String {
    let mut blocks = Vec::new();
    for embed in embeds {
        let mut parts = Vec::new();
        if let Some(title) = embed.title.as_ref().filter(|value| !value.is_empty()) {
            parts.push(format!("**{title}**"));
        }
        if let Some(description) = embed.description.as_ref().filter(|value| !value.is_empty()) {
            parts.push(description.clone());
        }
        if let Some(fields) = embed.fields.as_ref() {
            for field in fields {
                parts.push(format!("**{}**\n{}", field.name, field.value));
            }
        }
        if let Some(footer) = embed.footer.as_ref().filter(|value| !value.text.is_empty()) {
            parts.push(format!("_{text}_", text = footer.text));
        }
        if !parts.is_empty() {
            blocks.push(parts.join("\n\n"));
        }
    }
    blocks.join("\n\n")
}

fn build_rich_fallback_text(content: &str, embeds: &[Embed]) -> String {
    let mut parts = Vec::new();
    if !content.is_empty() {
        parts.push(content.to_string());
    }
    let embed_text = flatten_embeds_for_plain_text(embeds);
    if !embed_text.is_empty() {
        parts.push(embed_text);
    }
    parts.join("\n\n")
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
            last_message_cursor: Arc::new(RwLock::new(String::new())),
            typing_cancels: Arc::new(RwLock::new(BTreeMap::new())),
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

        // Phase 2b: Register slash commands (/plan, /execute).
        match fetch_application_id(&self.http, &self.config.bot_token).await {
            Ok(app_id) => {
                if let Err(e) = register_commands(
                    &self.http,
                    &self.config.bot_token,
                    &app_id,
                    self.config.guild_id.as_deref(),
                )
                .await
                {
                    eprintln!("  [discord] Failed to register slash commands: {e}");
                }
            }
            Err(e) => eprintln!("  [discord] Failed to fetch application ID: {e}"),
        }

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

            // Flush cursor before reconnecting so it survives if we crash.
            self.flush_cursor().await;

            if let Some(resume) = self.gateway.resume_url.read().await.as_ref() {
                url = format!("{resume}/?v=10&encoding=json");
            }

            println!("  [discord] Reconnecting...");
        }
    }

    /// Bootstrap the Channel entity used by the Discord transport.
    ///
    /// The transport is the SOLE OWNER of Channel entities. The soul bootstrap
    /// does not create, modify, or query Channels — only AgentRoutes and Souls.
    ///
    /// Reuses an existing Connected/Disconnected Channel if one exists
    /// (preserving ChannelSessions and conversation state across restarts).
    /// Archives any duplicates. Only creates a new Channel if none exists.
    async fn bootstrap_channel(&self, webhook_url: &str) -> Result<(), String> {
        let existing = self
            .api
            .query_entities(
                "Channels",
                "ChannelType eq 'discord' and Status ne 'Archived'",
            )
            .await
            .unwrap_or_default();

        // Find the best Channel to reuse: prefer Connected > Disconnected,
        // and among those, the one with the highest message_count (most active).
        let mut best_id = String::new();
        let mut best_msg_count: i64 = -1;
        let mut others_to_archive = Vec::new();

        for ch in &existing {
            let status = ch
                .get("status")
                .or_else(|| ch.get("fields").and_then(|f| f.get("Status")))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let id = ch
                .get("entity_id")
                .or_else(|| ch.get("Id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if id.is_empty() {
                continue;
            }

            if status == "Connected" || status == "Disconnected" {
                let msg_count = ch
                    .get("counters")
                    .and_then(|c| c.get("message_count"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                if msg_count > best_msg_count {
                    if !best_id.is_empty() {
                        others_to_archive.push(best_id.clone());
                    }
                    best_id = id;
                    best_msg_count = msg_count;

                    // Read cursor from the best channel
                    if let Some(cursor) = ch
                        .get("fields")
                        .and_then(|f| f.get("last_discord_message_id"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                    {
                        *self.last_message_cursor.write().await = cursor.to_string();
                    }
                } else {
                    others_to_archive.push(id);
                }
            } else {
                // Created, Connecting — not reusable, archive
                others_to_archive.push(id);
            }
        }

        // Archive duplicates and stale channels
        for old_id in &others_to_archive {
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

        let channel_id = if !best_id.is_empty() {
            println!("  [discord] Reusing Channel entity: {best_id} (messages: {best_msg_count})");
            // Update webhook_url so reply delivery uses the current port
            let _ = self
                .api
                .dispatch_action(
                    "Channels",
                    &best_id,
                    "Paw.Channel.UpdateConfig",
                    serde_json::json!({
                        "default_agent_config": serde_json::json!({"webhook_url": webhook_url}).to_string(),
                    }),
                )
                .await;
            best_id
        } else {
            // No reusable Channel — create fresh.
            let resp = self
                .api
                .create_entity("Channels", serde_json::json!({"ChannelType": "discord"}))
                .await?;
            let id = resp
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            println!("  [discord] Created Channel entity: {id}");

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

    /// Catch up on DMs missed while the bot was offline.
    ///
    /// Fetches messages newer than `last_message_cursor` from all DM channels
    /// and dispatches ReceiveMessage for each. Called after Gateway READY.
    async fn catch_up_missed_dms(&self) {
        let cursor = self.last_message_cursor.read().await.clone();
        if cursor.is_empty() {
            // No baseline cursor — first-ever run, nothing to catch up on.
            return;
        }

        let channel_entity_id = self.channel_entity_id.read().await.clone();
        let Some(ref entity_id) = channel_entity_id else {
            return;
        };

        let dm_channels = fetch_dm_channels(&self.http, &self.config.bot_token).await;
        if dm_channels.is_empty() {
            return;
        }

        let bot_id = self.gateway.bot_user_id.read().await.clone();
        let mut total_caught_up = 0u32;

        for dm in &dm_channels {
            let dm_channel_id = dm.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if dm_channel_id.is_empty() {
                continue;
            }

            // Fetch messages newer than our cursor
            let mut after = cursor.clone();
            loop {
                let messages = fetch_channel_messages(
                    &self.http,
                    &self.config.bot_token,
                    dm_channel_id,
                    &after,
                    100,
                )
                .await;

                if messages.is_empty() {
                    break;
                }

                for msg in &messages {
                    let author_id = msg
                        .get("author")
                        .and_then(|a| a.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let is_bot = msg
                        .get("author")
                        .and_then(|a| a.get("bot"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let msg_id = msg.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");

                    // Skip bot's own messages and empty messages
                    if author_id == bot_id || is_bot || content.is_empty() {
                        continue;
                    }

                    let username = msg
                        .get("author")
                        .and_then(|a| a.get("username"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    println!(
                        "  [discord] Catching up missed message from {username}: {}",
                        &content[..content.len().min(40)]
                    );

                    // Track DM channel mapping
                    self.dm_channels
                        .write()
                        .await
                        .insert(author_id.to_string(), dm_channel_id.to_string());

                    // Dispatch ReceiveMessage
                    let params = serde_json::json!({
                        "message_id": msg_id,
                        "author_id": author_id,
                        "thread_id": author_id,
                        "content": content,
                    });

                    match self
                        .api
                        .dispatch_action(
                            "Channels",
                            entity_id,
                            "Paw.Channel.ReceiveMessage",
                            params,
                        )
                        .await
                    {
                        Ok(_) => {
                            total_caught_up += 1;
                            let mut c = self.last_message_cursor.write().await;
                            if msg_id > c.as_str() {
                                *c = msg_id.to_string();
                            }
                        }
                        Err(e) => {
                            eprintln!("  [discord] Catch-up ReceiveMessage failed: {e}");
                        }
                    }
                }

                let last_id = messages
                    .last()
                    .and_then(|m| m.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if messages.len() < 100 || last_id.is_empty() {
                    break;
                }
                after = last_id.to_string();
            }

            // Rate limit: 200ms between channel fetches
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        if total_caught_up > 0 {
            println!("  [discord] Caught up {total_caught_up} missed messages");
            // Flush cursor to Channel entity
            self.flush_cursor().await;
        }
    }

    /// Persist the last_message_cursor to the Channel entity via UpdateCursor.
    async fn flush_cursor(&self) {
        let cursor = self.last_message_cursor.read().await.clone();
        if cursor.is_empty() {
            return;
        }
        let channel_entity_id = self.channel_entity_id.read().await.clone();
        let Some(ref entity_id) = channel_entity_id else {
            return;
        };
        let _ = self
            .api
            .dispatch_action(
                "Channels",
                entity_id,
                "Paw.Channel.UpdateCursor",
                serde_json::json!({ "last_discord_message_id": cursor }),
            )
            .await;
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
        let awaiting_heartbeat_ack = Arc::new(AtomicBool::new(false));
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
                        return Err("Discord gateway closed unexpectedly".to_string());
                    };
                    let frame = frame.map_err(|e| format!("WebSocket read error: {e}"))?;
                    if let Message::Close(close) = &frame {
                        let reason = close
                            .as_ref()
                            .map(|frame| format!("code={} reason={}", frame.code, frame.reason))
                            .unwrap_or_else(|| "no close frame".to_string());
                        return Err(format!("Discord gateway closed: {reason}"));
                    }
                    let Some(payload) = parse_frame(frame)? else {
                        continue;
                    };
                    let should_reconnect = self
                        .handle_payload(payload, &awaiting_heartbeat_ack)
                        .await?;
                    if should_reconnect {
                        return Ok(());
                    }
                }
                Some(()) = heartbeat_rx.recv() => {
                    if awaiting_heartbeat_ack.swap(true, Ordering::SeqCst) {
                        return Err(
                            "Discord gateway heartbeat ACK timeout (possible close code 1006)"
                                .to_string(),
                        );
                    }
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
    async fn handle_payload(
        &self,
        payload: GatewayPayload,
        awaiting_heartbeat_ack: &AtomicBool,
    ) -> Result<bool, String> {
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
                        // Catch up on messages missed while offline.
                        self.catch_up_missed_dms().await;
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
            Some(GatewayOpcode::HeartbeatAck) => {
                awaiting_heartbeat_ack.store(false, Ordering::SeqCst);
                Ok(false)
            }
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

        // Start typing indicator refresh loop.
        // Sends typing every 8 seconds (Discord expires it at ~10s) until cancelled by reply.
        send_typing(&self.http, &self.config.bot_token, &msg.channel_id).await;
        {
            let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
            self.typing_cancels
                .write()
                .await
                .insert(msg.author.id.clone(), cancel_tx);
            let http = self.http.clone();
            let bot_token = self.config.bot_token.clone();
            let channel_id = msg.channel_id.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(8)) => {
                            send_typing(&http, &bot_token, &channel_id).await;
                        }
                        _ = cancel_rx.changed() => {
                            break;
                        }
                    }
                }
            });
        }

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
                // Update cursor to track last processed message for catch-up
                let mut cursor = self.last_message_cursor.write().await;
                if msg.id.as_str() > cursor.as_str() {
                    *cursor = msg.id.clone();
                }
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
        use super::types::*;
        use axum::{Router, extract::State, routing::post};

        #[derive(Clone)]
        struct WebhookState {
            http: reqwest::Client,
            bot_token: String,
            dm_channels: Arc<RwLock<BTreeMap<String, String>>>,
            api: crate::PawApiClient,
            public_key: String,
            channel_entity_id: Arc<RwLock<Option<String>>>,
            typing_cancels: Arc<RwLock<BTreeMap<String, tokio::sync::watch::Sender<bool>>>>,
        }

        /// Handle reply callbacks from send_reply and request_approval WASM.
        /// Supports optional `components` field for button messages.
        async fn handle_reply(
            State(state): State<WebhookState>,
            axum::Json(body): axum::Json<serde_json::Value>,
        ) -> axum::http::StatusCode {
            let thread_id = body.get("thread_id").and_then(|v| v.as_str()).unwrap_or("");
            let content = body.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let components: Vec<ActionRow> = body
                .get("components")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let embeds: Vec<Embed> = body
                .get("embeds")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let has_rich_content = !components.is_empty() || !embeds.is_empty();

            if thread_id.is_empty() || (content.is_empty() && !has_rich_content) {
                tracing::error!("discord reply webhook missing content and rich payload");
                return axum::http::StatusCode::BAD_REQUEST;
            }

            // Cancel the typing indicator loop for this thread — reply is arriving.
            if let Some(cancel_tx) = state.typing_cancels.write().await.remove(thread_id) {
                let _ = cancel_tx.send(true);
            }

            // thread_id is the Discord user ID (for DMs). Look up their DM channel.
            let channel_id = state.dm_channels.read().await.get(thread_id).cloned();
            let Some(channel_id) = channel_id else {
                tracing::error!(thread_id, "discord reply webhook has no DM channel mapping");
                return axum::http::StatusCode::NOT_FOUND;
            };

            if has_rich_content {
                let fallback_text = build_rich_fallback_text(content, &embeds);
                let needs_fallback = content.len() > 2000 || embeds_exceed_limits(&embeds);

                if !needs_fallback {
                    tracing::info!(
                        thread_id,
                        content_len = content.len(),
                        component_count = components.len(),
                        embed_count = embeds.len(),
                        "delivering rich discord reply"
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
                        Ok(_msg) => return axum::http::StatusCode::OK,
                        Err(DiscordApiError::PayloadTooLarge(error)) => {
                            tracing::warn!(thread_id, %error, "rich discord reply exceeded payload limits; falling back to plain text");
                        }
                        Err(error) => {
                            tracing::error!(thread_id, %error, "rich discord reply delivery failed");
                            return axum::http::StatusCode::INTERNAL_SERVER_ERROR;
                        }
                    }
                }

                if !fallback_text.is_empty() {
                    if let Err(error) = send_discord_message(
                        &state.http,
                        &state.bot_token,
                        &channel_id,
                        &fallback_text,
                    )
                    .await
                    {
                        tracing::error!(thread_id, %error, "plain-text fallback delivery failed");
                        return axum::http::StatusCode::INTERNAL_SERVER_ERROR;
                    }
                }

                if !components.is_empty() {
                    let controls_text = if fallback_text.is_empty() {
                        "Interactive controls:".to_string()
                    } else {
                        "Interactive controls below:".to_string()
                    };
                    if let Err(error) = send_discord_message_with_components(
                        &state.http,
                        &state.bot_token,
                        &channel_id,
                        &controls_text,
                        &components,
                        &[],
                    )
                    .await
                    {
                        tracing::error!(thread_id, %error, "component follow-up delivery failed");
                        return axum::http::StatusCode::INTERNAL_SERVER_ERROR;
                    }
                }

                axum::http::StatusCode::OK
            } else {
                tracing::info!(
                    thread_id,
                    content_len = content.len(),
                    "delivering discord reply"
                );

                match send_discord_message(&state.http, &state.bot_token, &channel_id, content)
                    .await
                {
                    Ok(()) => axum::http::StatusCode::OK,
                    Err(e) => {
                        tracing::error!(thread_id, %e, "discord reply delivery failed");
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

            // Type 2 = APPLICATION_COMMAND (slash command)
            if payload.interaction_type == 2 {
                let empty = serde_json::json!({});
                let data = payload.data.as_ref().unwrap_or(&empty);
                let command_name = data.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let command = match command_name {
                    "plan" | "execute" => command_name.to_string(),
                    _ => {
                        return (
                            axum::http::StatusCode::OK,
                            axum::Json(serde_json::json!({
                                "type": 4,
                                "data": { "content": "Unknown command.", "flags": 64 }
                            })),
                        );
                    }
                };
                let task_text = data
                    .get("options")
                    .and_then(|v| v.as_array())
                    .and_then(|opts| {
                        opts.iter()
                            .find(|o| o.get("name").and_then(|n| n.as_str()) == Some("task"))
                    })
                    .and_then(|o| o.get("value").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string();

                // Extract user ID from interaction payload
                let user_id = payload
                    .user
                    .as_ref()
                    .map(|u| u.id.clone())
                    .or_else(|| {
                        payload
                            .member
                            .as_ref()
                            .and_then(|m| m.get("user"))
                            .and_then(|u| u.get("id"))
                            .and_then(|id| id.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_default();

                // Store DM channel mapping for reply routing
                let channel_id = payload.channel_id.clone().unwrap_or_default();
                if !user_id.is_empty() && !channel_id.is_empty() {
                    state
                        .dm_channels
                        .write()
                        .await
                        .insert(user_id.clone(), channel_id);
                }

                let entity_id = state.channel_entity_id.read().await.clone();
                let api = state.api.clone();
                let http = state.http.clone();
                let bot_token = state.bot_token.clone();
                let app_id = payload.application_id.clone().unwrap_or_default();
                let interaction_token = payload.token.clone();

                // Dispatch ReceiveMessage asynchronously (deferred response)
                tokio::spawn(async move {
                    let Some(entity_id) = entity_id else {
                        eprintln!("  [discord] No channel entity for slash command");
                        return;
                    };
                    let params = serde_json::json!({
                        "message_id": format!("cmd-{}", std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis())
                            .unwrap_or(0)),
                        "author_id": user_id,
                        "thread_id": user_id,
                        "content": task_text,
                        "command": command,
                    });
                    let preview = if task_text.len() > 80 {
                        &task_text[..80]
                    } else {
                        &task_text
                    };
                    println!("  [discord] /{command} from {user_id}: {preview}");
                    if let Err(e) = api
                        .dispatch_action(
                            "Channels",
                            &entity_id,
                            "Paw.Channel.ReceiveMessage",
                            params,
                        )
                        .await
                    {
                        eprintln!("  [discord] Slash command dispatch failed: {e}");
                        // Edit the deferred message with error
                        let _ = http
                            .patch(format!(
                                "{}/webhooks/{app_id}/{interaction_token}/messages/@original",
                                DISCORD_API_BASE
                            ))
                            .header("Authorization", format!("Bot {bot_token}"))
                            .json(&serde_json::json!({"content": format!("Failed to process /{command}: {e}")}))
                            .send()
                            .await;
                    }
                });

                // Respond with type 5 = DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE
                return (
                    axum::http::StatusCode::OK,
                    axum::Json(serde_json::json!({ "type": 5 })),
                );
            }

            // Type 3 = MESSAGE_COMPONENT (button click)
            if payload.interaction_type != 3 {
                return (
                    axum::http::StatusCode::OK,
                    axum::Json(serde_json::json!({
                        "type": 4,
                        "data": { "content": "Unsupported interaction type.", "flags": 64 }
                    })),
                );
            }

            let Some(ref data) = payload.data else {
                return (
                    axum::http::StatusCode::OK,
                    axum::Json(serde_json::json!({
                        "type": 4,
                        "data": { "content": "No interaction data.", "flags": 64 }
                    })),
                );
            };

            let custom_id = data.get("custom_id").and_then(|v| v.as_str()).unwrap_or("");
            let parts: Vec<&str> = custom_id.splitn(2, ':').collect();
            if parts.len() != 2 {
                return (
                    axum::http::StatusCode::OK,
                    axum::Json(serde_json::json!({
                        "type": 4,
                        "data": { "content": "Invalid button ID.", "flags": 64 }
                    })),
                );
            }

            let (action, target_id) = (parts[0], parts[1]);
            if action != "approve"
                && action != "deny"
                && action != "plan_approve"
                && action != "plan_request_changes"
            {
                return (
                    axum::http::StatusCode::OK,
                    axum::Json(serde_json::json!({
                        "type": 4,
                        "data": { "content": "Unknown action.", "flags": 64 }
                    })),
                );
            }

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

            println!("  [discord] Interaction: {action} target {target_id} by {reviewer_id}");

            // Process via Temper's native decisions API asynchronously
            let api = state.api.clone();
            let target_id_owned = target_id.to_string();
            let action_owned = action.to_string();
            let reviewer_id_owned = reviewer_id.clone();
            let token = payload.token.clone();
            let app_id = payload.application_id.clone().unwrap_or_default();
            let http = state.http.clone();

            tokio::spawn(async move {
                let base_url = api.config().base_url.clone();
                let tenant = api.config().tenant.clone();

                // Read the original message content so we can preserve it
                let original_content = {
                    let msg_url = format!(
                        "https://discord.com/api/v10/webhooks/{app_id}/{token}/messages/@original"
                    );
                    match http.get(&msg_url).send().await {
                        Ok(resp) => resp
                            .json::<serde_json::Value>()
                            .await
                            .ok()
                            .and_then(|v| {
                                v.get("content").and_then(|c| c.as_str()).map(String::from)
                            })
                            .unwrap_or_default(),
                        Err(_) => String::new(),
                    }
                };

                let (_success, status_line) = match action_owned.as_str() {
                    "approve" => {
                        let approve_url = format!(
                            "{base_url}/api/tenants/{tenant}/decisions/{target_id_owned}/approve"
                        );
                        let scope = serde_json::json!({
                            "scope": {
                                "principal": "this_agent",
                                "action": "this_action",
                                "resource": "any_of_type",
                                "duration": "always"
                            },
                            "decided_by": format!("discord:{reviewer_id_owned}")
                        });
                        match api.raw_post(&approve_url, scope).await {
                            Ok(_) => (true, format!("Approval recorded by <@{reviewer_id_owned}>")),
                            Err(e) => (false, format!("Approval failed: {e}")),
                        }
                    }
                    "deny" => {
                        let deny_url = format!(
                            "{base_url}/api/tenants/{tenant}/decisions/{target_id_owned}/deny"
                        );
                        let deny_body = serde_json::json!({
                            "decided_by": format!("discord:{reviewer_id_owned}")
                        });
                        match api.raw_post(&deny_url, deny_body).await {
                            Ok(_) => (true, format!("Denial recorded by <@{reviewer_id_owned}>")),
                            Err(e) => (false, format!("Deny failed: {e}")),
                        }
                    }
                    "plan_approve" => match api
                        .dispatch_action(
                            "Plans",
                            &target_id_owned,
                            "OpenPaw.Approve",
                            serde_json::json!({}),
                        )
                        .await
                    {
                        Ok(_) => (true, format!("Plan approved by <@{reviewer_id_owned}>")),
                        Err(e) => (false, format!("Plan approval failed: {e}")),
                    },
                    "plan_request_changes" => {
                        let review_notes = format!(
                            "Changes requested by discord:{reviewer_id_owned}. Review the plan, revise it, and resubmit for approval."
                        );
                        match api
                            .dispatch_action(
                                "Plans",
                                &target_id_owned,
                                "OpenPaw.RequestChanges",
                                serde_json::json!({ "review_notes": review_notes }),
                            )
                            .await
                        {
                            Ok(_) => (
                                true,
                                format!(
                                    "Plan changes requested by <@{reviewer_id_owned}>. Additional details can be sent in-thread."
                                ),
                            ),
                            Err(e) => (false, format!("Request changes failed: {e}")),
                        }
                    }
                    _ => (false, "Unknown action.".to_string()),
                };

                // Build the updated message: original context + decision result
                let message = if original_content.is_empty() {
                    status_line
                } else {
                    let updated = if original_content.contains("**Plan Review Required**") {
                        original_content.replace(
                            "**Plan Review Required**",
                            &format!("~~Plan Review Required~~ **{status_line}**"),
                        )
                    } else {
                        original_content.replace(
                            "**Permission Required**",
                            &format!("~~Permission Required~~ **{status_line}**"),
                        )
                    };
                    // Remove the "Click a button" instruction line if present
                    updated
                        .lines()
                        .filter(|l| !l.contains("Click a button"))
                        .collect::<Vec<_>>()
                        .join("\n")
                };

                // Session resume/fail is now handled by GovernanceDecision.DispatchCallback
                // effect — no transport-level orchestration needed.

                // Edit the Discord message to show result
                if !app_id.is_empty() && !token.is_empty() {
                    let follow_up_url = format!(
                        "https://discord.com/api/v10/webhooks/{app_id}/{token}/messages/@original"
                    );
                    let _ = http
                        .patch(&follow_up_url)
                        .header("Content-Type", "application/json")
                        .json(&serde_json::json!({
                            "content": message,
                            "components": []
                        }))
                        .send()
                        .await;
                }
            });

            // Respond with deferred update (type 6 = DEFERRED_UPDATE_MESSAGE)
            // This removes the "thinking" state and we'll edit the message in the spawn above.
            (
                axum::http::StatusCode::OK,
                axum::Json(serde_json::json!({ "type": 6 })),
            )
        }

        let webhook_state = WebhookState {
            http: self.http.clone(),
            bot_token: self.config.bot_token.clone(),
            dm_channels: self.dm_channels.clone(),
            api: self.api.clone(),
            public_key: self.config.public_key.clone(),
            channel_entity_id: self.channel_entity_id.clone(),
            typing_cancels: self.typing_cancels.clone(),
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
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

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
