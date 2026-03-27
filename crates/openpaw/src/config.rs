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

    /// Bearer token for Temper API authentication.
    pub temper_api_key: Option<String>,

    /// 32-byte base64 key for secrets vault encryption.
    /// If not set, an ephemeral key is generated (secrets lost on restart).
    pub vault_key: Option<String>,

    /// Fly.io API token for Sprites provisioning.
    pub fly_api_token: Option<String>,

    /// E2B sandbox API key.
    pub e2b_api_key: Option<String>,

    /// GitHub token for repo access.
    pub github_token: Option<String>,

    /// Logfire read token for querying alerts/traces.
    pub logfire_read_token: Option<String>,

    /// Logfire write token for sending traces.
    pub logfire_write_token: Option<String>,

    /// HTTP port for the OData API + webhook listener.
    pub port: u16,

    /// Default tenant ID.
    pub tenant: String,
}

impl Config {
    /// Load configuration from environment variables.
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            discord_bot_token: std::env::var("DISCORD_BOT_TOKEN").ok()
                .filter(|s| !s.is_empty()),
            turso_url: std::env::var("TURSO_URL").ok(),
            turso_auth_token: std::env::var("TURSO_AUTH_TOKEN").ok(),
            anthropic_api_key: std::env::var("ANTHROPIC_API_KEY").ok()
                .filter(|s| !s.is_empty()),
            temper_api_key: std::env::var("TEMPER_API_KEY").ok()
                .filter(|s| !s.is_empty()),
            vault_key: std::env::var("TEMPER_VAULT_KEY").ok()
                .filter(|s| !s.is_empty()),
            fly_api_token: std::env::var("FLY_API_TOKEN").ok()
                .filter(|s| !s.is_empty()),
            e2b_api_key: std::env::var("E2B_API_KEY").ok()
                .filter(|s| !s.is_empty()),
            github_token: std::env::var("GITHUB_TOKEN").ok()
                .filter(|s| !s.is_empty()),
            logfire_read_token: std::env::var("LOGFIRE_READ_TOKEN").ok()
                .filter(|s| !s.is_empty()),
            logfire_write_token: std::env::var("LOGFIRE_WRITE_TOKEN").ok()
                .filter(|s| !s.is_empty()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3467),
            tenant: std::env::var("PAW_TENANT")
                .unwrap_or_else(|_| "default".to_string()),
        })
    }
}
