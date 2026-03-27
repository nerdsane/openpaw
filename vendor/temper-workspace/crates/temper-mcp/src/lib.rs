//! stdio MCP server exposing Temper Code Mode tools.

mod protocol;
mod runtime;

pub mod repl;
pub use runtime::run_stdio_server;

#[cfg(test)]
use protocol::dispatch_json_value;
use runtime::RuntimeContext;

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
const MCP_SERVER_NAME: &str = "temper-mcp";

/// Runtime config for the stdio MCP server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpConfig {
    /// Port where a local Temper server is running.
    /// Mutually exclusive with `temper_url`.
    pub temper_port: Option<u16>,
    /// Full URL of a remote Temper server (e.g. `https://api.temper.build`).
    /// Mutually exclusive with `temper_port`.
    pub temper_url: Option<String>,
    /// Agent instance ID. Resolved from the credential registry via
    /// `TEMPER_API_KEY` at startup (ADR-0033). Only used as an override
    /// when credential resolution is not available.
    pub agent_id: Option<String>,
    /// Agent software classification (e.g. `claude-code`). Resolved from
    /// the credential registry's `AgentType` entity at startup (ADR-0033).
    pub agent_type: Option<String>,
    /// Session ID (`X-Session-Id`). Auto-derived from `CLAUDE_SESSION_ID`.
    pub session_id: Option<String>,
    /// Bearer token for API authentication (`TEMPER_API_KEY`).
    /// When set, all requests include `Authorization: Bearer <key>`.
    /// The platform resolves this to a verified agent identity via the
    /// credential registry (ADR-0033).
    pub api_key: Option<String>,
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
