//! Configuration loaded from environment variables.

/// Open Paw daemon configuration.
pub struct Config {
    /// Discord bot token for the Paw agent.
    pub discord_bot_token: Option<String>,

    /// Discord application public key for interaction signature verification.
    pub discord_public_key: Option<String>,

    /// Turso database URL (local file or cloud).
    /// Default: ~/.local/share/openpaw/paw.db
    pub turso_url: Option<String>,

    /// Turso auth token (for Turso Cloud).
    pub turso_auth_token: Option<String>,

    /// Anthropic API key for LLM calls.
    pub anthropic_api_key: Option<String>,

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

    /// Shared secret used to validate webhook request signatures.
    pub webhook_secret: Option<String>,

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
            turso_url: optional_env("TURSO_URL"),
            turso_auth_token: optional_env("TURSO_AUTH_TOKEN"),
            anthropic_api_key: optional_env("ANTHROPIC_API_KEY"),
            tensorlake_api_key: optional_env("TL_API_KEY"),
            github_token: optional_env("GITHUB_TOKEN"),
            dd_api_key: optional_env("DD_API_KEY"),
            dd_app_key: optional_env("DD_APP_KEY"),
            dd_site: std::env::var("DD_SITE").unwrap_or_else(|_| "datadoghq.com".to_string()),
            temper_api_key: optional_env("TEMPER_API_KEY"),
            vault_key: optional_env("TEMPER_VAULT_KEY"),
            fly_api_token: optional_env("FLY_API_TOKEN"),
            webhook_secret: optional_env("WEBHOOK_SECRET"),
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
