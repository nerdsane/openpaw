//! Runtime transport lifecycle management.
//!
//! Provides start/stop/status control over Discord and Slack transports
//! without requiring a process restart. Stores a `CancellationToken` per
//! transport so they can be gracefully shut down via the setup API.

use std::sync::Arc;

use serde::Serialize;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Manages the lifecycle of Discord and Slack transports at runtime.
pub struct TransportManager {
    discord: Arc<RwLock<TransportHandle>>,
    slack: Arc<RwLock<TransportHandle>>,
    tenant: String,
    port: u16,
    api_key: Option<String>,
}

struct TransportHandle {
    status: TransportStatus,
    cancel: Option<CancellationToken>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TransportStatus {
    Disconnected,
    Connecting,
    Connected {
        #[serde(skip_serializing_if = "Option::is_none")]
        guild_id: Option<String>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct AllTransportStatus {
    pub discord: TransportStatus,
    pub slack: TransportStatus,
}

/// Discord connection parameters.
pub struct DiscordConnectParams {
    pub bot_token: String,
    pub public_key: String,
    pub guild_id: Option<String>,
    pub feed_channel_id: Option<String>,
    pub forum_channel_id: Option<String>,
}

/// Slack connection parameters.
pub struct SlackConnectParams {
    pub app_token: String,
    pub bot_token: String,
    pub signing_secret: String,
}

impl TransportManager {
    pub fn new(tenant: String, port: u16, api_key: Option<String>) -> Self {
        Self {
            discord: Arc::new(RwLock::new(TransportHandle {
                status: TransportStatus::Disconnected,
                cancel: None,
            })),
            slack: Arc::new(RwLock::new(TransportHandle {
                status: TransportStatus::Disconnected,
                cancel: None,
            })),
            tenant,
            port,
            api_key,
        }
    }

    /// Start the Discord transport. If already connected, disconnects first.
    pub async fn connect_discord(&self, params: DiscordConnectParams) {
        // Disconnect existing if any
        self.disconnect_discord().await;

        let cancel = CancellationToken::new();
        {
            let mut handle = self.discord.write().await;
            handle.status = TransportStatus::Connecting;
            handle.cancel = Some(cancel.clone());
        }

        let discord_handle = self.discord.clone();
        let tenant = self.tenant.clone();
        let port = self.port;
        let api_key = self.api_key.clone();
        let guild_id_for_status = params.guild_id.clone();

        tokio::spawn(async move {
            use paw_transport::PawApiConfig;
            use paw_transport::discord::types::intents;
            use paw_transport::discord::{DiscordConfig, DiscordTransport};

            let api_url = format!("http://127.0.0.1:{port}");
            tracing::info!("Discord transport: connecting (tenant={tenant})...");

            let api = paw_transport::PawApiClient::new(PawApiConfig {
                base_url: api_url,
                tenant,
                api_key,
            });
            let config = DiscordConfig {
                bot_token: params.bot_token,
                public_key: params.public_key,
                intents: intents::DEFAULT,
                webhook_port: 3488,
                guild_id: params.guild_id,
                feed_channel_id: params.feed_channel_id,
                forum_channel_id: params.forum_channel_id,
            };

            // Mark as connected once we start the run loop
            {
                let mut handle = discord_handle.write().await;
                handle.status = TransportStatus::Connected {
                    guild_id: guild_id_for_status,
                };
            }

            let transport = DiscordTransport::new(config, api);
            tokio::select! {
                result = transport.run() => {
                    let mut handle = discord_handle.write().await;
                    match result {
                        Ok(()) => {
                            handle.status = TransportStatus::Disconnected;
                        }
                        Err(e) => {
                            tracing::error!("Discord transport error: {e}");
                            handle.status = TransportStatus::Error {
                                message: e.to_string(),
                            };
                        }
                    }
                    handle.cancel = None;
                }
                _ = cancel.cancelled() => {
                    tracing::info!("Discord transport: shutting down (cancelled)");
                    let mut handle = discord_handle.write().await;
                    handle.status = TransportStatus::Disconnected;
                    handle.cancel = None;
                }
            }
        });
    }

    /// Stop the Discord transport gracefully.
    pub async fn disconnect_discord(&self) {
        let mut handle = self.discord.write().await;
        if let Some(cancel) = handle.cancel.take() {
            cancel.cancel();
        }
        handle.status = TransportStatus::Disconnected;
    }

    /// Start the Slack transport. If already connected, disconnects first.
    pub async fn connect_slack(&self, params: SlackConnectParams) {
        self.disconnect_slack().await;

        let cancel = CancellationToken::new();
        {
            let mut handle = self.slack.write().await;
            handle.status = TransportStatus::Connecting;
            handle.cancel = Some(cancel.clone());
        }

        let slack_handle = self.slack.clone();
        let tenant = self.tenant.clone();
        let port = self.port;
        let api_key = self.api_key.clone();

        tokio::spawn(async move {
            use paw_transport::PawApiConfig;
            use paw_transport::slack::{SlackConfig, SlackTransport};

            let api_url = format!("http://127.0.0.1:{port}");
            tracing::info!("Slack transport: connecting (tenant={tenant})...");

            let api = paw_transport::PawApiClient::new(PawApiConfig {
                base_url: api_url,
                tenant,
                api_key,
            });
            let config = SlackConfig {
                app_token: params.app_token,
                bot_token: params.bot_token,
                signing_secret: params.signing_secret,
                webhook_port: 3489,
            };

            {
                let mut handle = slack_handle.write().await;
                handle.status = TransportStatus::Connected { guild_id: None };
            }

            let transport = SlackTransport::new(config, api);
            tokio::select! {
                result = transport.run() => {
                    let mut handle = slack_handle.write().await;
                    match result {
                        Ok(()) => {
                            handle.status = TransportStatus::Disconnected;
                        }
                        Err(e) => {
                            tracing::error!("Slack transport error: {e}");
                            handle.status = TransportStatus::Error {
                                message: e.to_string(),
                            };
                        }
                    }
                    handle.cancel = None;
                }
                _ = cancel.cancelled() => {
                    tracing::info!("Slack transport: shutting down (cancelled)");
                    let mut handle = slack_handle.write().await;
                    handle.status = TransportStatus::Disconnected;
                    handle.cancel = None;
                }
            }
        });
    }

    /// Stop the Slack transport gracefully.
    pub async fn disconnect_slack(&self) {
        let mut handle = self.slack.write().await;
        if let Some(cancel) = handle.cancel.take() {
            cancel.cancel();
        }
        handle.status = TransportStatus::Disconnected;
    }

    /// Get the current status of all transports.
    pub async fn status(&self) -> AllTransportStatus {
        AllTransportStatus {
            discord: self.discord.read().await.status.clone(),
            slack: self.slack.read().await.status.clone(),
        }
    }
}
