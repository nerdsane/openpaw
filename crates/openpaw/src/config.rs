//! Configuration loaded from environment variables.

/// Open Paw daemon configuration.
pub struct Config {
    /// Discord bot token for the Paw agent.
    pub discord_bot_token: Option<String>,

    /// Turso database URL (local file or cloud).
    /// Default: ~/.local/share/openpaw/paw.db
    pub turso_url: Option<String>,

    /// Turso auth token (for Turso Cloud).
    pub turso_auth_token: Option<String>,

    /// Anthropic API key for LLM calls.
    pub anthropic_api_key: Option<String>,

    /// Modal token ID for sandbox provisioning.
    pub modal_token_id: Option<String>,

    /// Modal token secret for sandbox provisioning.
    pub modal_token_secret: Option<String>,

    /// GitHub token for repo cloning and PR flows.
    pub github_token: Option<String>,

    /// Datadog API key for monitor management and queries.
    pub dd_api_key: Option<String>,

    /// Datadog application key for monitor management.
    pub dd_app_key: Option<String>,

    /// Datadog site (default: datadoghq.com).
    pub dd_site: String,

    /// Shared secret for webhook signature verification.
    pub webhook_secret: Option<String>,

    /// Bearer token for Temper API authentication.
    pub temper_api_key: Option<String>,

    /// 32-byte base64 key for secrets vault encryption.
    /// If not set, an ephemeral key is generated (secrets lost on restart).
    pub vault_key: Option<String>,

    /// Fly.io API token for Sprites provisioning.
    pub fly_api_token: Option<String>,

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
            turso_url: optional_env("TURSO_URL"),
            turso_auth_token: optional_env("TURSO_AUTH_TOKEN"),
            anthropic_api_key: optional_env("ANTHROPIC_API_KEY"),
            modal_token_id: optional_env("MODAL_TOKEN_ID"),
            modal_token_secret: optional_env("MODAL_TOKEN_SECRET"),
            github_token: optional_env("GITHUB_TOKEN"),
            dd_api_key: optional_env("DD_API_KEY"),
            dd_app_key: optional_env("DD_APP_KEY"),
            dd_site: std::env::var("DD_SITE").unwrap_or_else(|_| "datadoghq.com".to_string()),
            webhook_secret: optional_env("WEBHOOK_SECRET"),
            temper_api_key: optional_env("TEMPER_API_KEY"),
            vault_key: optional_env("TEMPER_VAULT_KEY"),
            fly_api_token: optional_env("FLY_API_TOKEN"),
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
