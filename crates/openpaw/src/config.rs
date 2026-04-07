//! Configuration loaded from environment variables.

/// Open Paw daemon configuration.
pub struct Config {
    /// Discord bot token for the Paw agent.
    pub discord_bot_token: Option<String>,

    /// Discord application public key for interaction signature verification.
    pub discord_public_key: Option<String>,

    /// Discord guild (server) ID for observability channels.
    pub discord_guild_id: Option<String>,

    /// Discord text channel ID for the #feed stream.
    pub discord_feed_channel_id: Option<String>,

    /// Discord forum channel ID for per-agent threads.
    pub discord_forum_channel_id: Option<String>,

    /// Slack App-Level Token (xapp-...) for Socket Mode connection.
    pub slack_app_token: Option<String>,

    /// Slack Bot Token (xoxb-...) for Web API calls.
    pub slack_bot_token: Option<String>,

    /// Slack Signing Secret for webhook signature verification.
    pub slack_signing_secret: Option<String>,

    /// Turso database URL (local file or cloud).
    /// Default: ~/.local/share/openpaw/paw.db
    pub turso_url: Option<String>,

    /// Turso auth token (for Turso Cloud).
    pub turso_auth_token: Option<String>,

    /// Anthropic API key (sk-ant-api03-... from console.anthropic.com).
    pub anthropic_api_key: Option<String>,

    /// OpenAI Codex OAuth token (JWT from ~/.codex/auth.json or OPENAI_CODEX_TOKEN env).
    pub openai_codex_token: Option<String>,

    /// LLM provider name: "anthropic", "openrouter", or "openai-codex".
    /// Set during interactive setup. Defaults to "anthropic" if not set.
    pub llm_provider: Option<String>,

    /// Tensorlake API key for remote sandbox provisioning.
    pub tensorlake_api_key: Option<String>,

    /// GitHub token for repo cloning and PR flows.
    pub github_token: Option<String>,

    /// Datadog API key for monitor and events APIs.
    pub dd_api_key: Option<String>,

    /// Datadog application key for monitor and events APIs.
    pub dd_app_key: Option<String>,

    /// Datadog site suffix, for example `datadoghq.com`.
    pub dd_site: String,

    /// Bearer token for Temper API authentication.
    pub temper_api_key: Option<String>,

    /// 32-byte base64 key for secrets vault encryption.
    /// If not set, an ephemeral key is generated (secrets lost on restart).
    pub vault_key: Option<String>,

    /// Fly.io API token for Sprites provisioning.
    pub fly_api_token: Option<String>,

    /// Railway API token for deployment integrations.
    pub railway_token: Option<String>,

    /// Vercel API token for deployment integrations.
    pub vercel_token: Option<String>,

    /// Exa API key for web search.
    pub exa_api_key: Option<String>,

    /// Enable OpenTelemetry export to Datadog Agent via OTLP.
    pub otel_enabled: bool,

    /// OTLP gRPC endpoint (Datadog Agent).
    pub otel_endpoint: String,

    /// HTTP port for the OData API + webhook listener.
    pub port: u16,

    /// Default tenant ID.
    pub tenant: String,
}

impl Config {
    /// Load configuration from environment variables.
    pub fn from_env() -> anyhow::Result<Self> {
        let _ = dotenvy::dotenv();

        Ok(Self {
            discord_bot_token: optional_env("DISCORD_BOT_TOKEN"),
            discord_public_key: optional_env("DISCORD_PUBLIC_KEY"),
            discord_guild_id: optional_env("DISCORD_GUILD_ID"),
            discord_feed_channel_id: optional_env("DISCORD_FEED_CHANNEL_ID"),
            discord_forum_channel_id: optional_env("DISCORD_FORUM_CHANNEL_ID"),
            slack_app_token: optional_env("SLACK_APP_TOKEN"),
            slack_bot_token: optional_env("SLACK_BOT_TOKEN"),
            slack_signing_secret: optional_env("SLACK_SIGNING_SECRET"),
            turso_url: optional_env("TURSO_URL"),
            turso_auth_token: optional_env("TURSO_AUTH_TOKEN"),
            anthropic_api_key: optional_env("ANTHROPIC_API_KEY"),
            openai_codex_token: optional_env("OPENAI_CODEX_TOKEN"),
            llm_provider: optional_env("LLM_PROVIDER"),
            tensorlake_api_key: optional_env("TL_API_KEY"),
            github_token: optional_env("GITHUB_TOKEN"),
            dd_api_key: optional_env("DD_API_KEY"),
            dd_app_key: optional_env("DD_APP_KEY"),
            dd_site: std::env::var("DD_SITE").unwrap_or_else(|_| "datadoghq.com".to_string()),
            temper_api_key: optional_env("TEMPER_API_KEY"),
            vault_key: optional_env("TEMPER_VAULT_KEY"),
            fly_api_token: optional_env("FLY_API_TOKEN"),
            railway_token: optional_env("RAILWAY_TOKEN"),
            vercel_token: optional_env("VERCEL_TOKEN"),
            exa_api_key: optional_env("EXA_API_KEY"),
            otel_enabled: std::env::var("OTEL_ENABLED")
                .map(|v| v != "false" && v != "0")
                .unwrap_or(true),
            otel_endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3467),
            tenant: std::env::var("PAW_TENANT").unwrap_or_else(|_| "default".to_string()),
        })
    }
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
