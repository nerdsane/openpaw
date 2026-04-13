//! Interactive setup for Open Paw.
//!
//! Two phases:
//! - Phase A (pre-boot): API key + messaging platform config
//! - Phase B (post-boot): User interview + LLM soul generation → writes to TemperFS via OData
//!
//! Phase B requires the server to be running so it can write the personalized
//! soul directly to Temper entities — no intermediate files on disk.

use std::io::IsTerminal;
use std::path::Path;

use crate::config::Config;
use crate::setup_llm::{self, GeneratedSoul, LlmProvider, UserInterview};
use axum::http::HeaderMap;

/// Result of Phase A (config setup).
pub struct SetupResult {
    pub api_key: Option<String>,
    pub provider: Option<String>,
    pub discord_bot_token: Option<String>,
    pub discord_public_key: Option<String>,
    pub discord_guild_id: Option<String>,
    pub discord_feed_channel_id: Option<String>,
    pub discord_forum_channel_id: Option<String>,
    pub slack_app_token: Option<String>,
    pub slack_bot_token: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SetupRequestAuth {
    pub cookie: Option<String>,
    pub authorization: Option<String>,
}

impl SetupRequestAuth {
    pub fn from_headers(headers: &HeaderMap) -> Self {
        Self {
            cookie: headers
                .get(axum::http::header::COOKIE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
            authorization: headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
        }
    }

    pub fn from_cookie(cookie: impl Into<String>) -> Self {
        Self {
            cookie: Some(cookie.into()),
            authorization: None,
        }
    }

    pub fn apply(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let request = if let Some(cookie) = &self.cookie {
            request.header(reqwest::header::COOKIE, cookie)
        } else {
            request
        };

        if let Some(authorization) = &self.authorization {
            request.header(reqwest::header::AUTHORIZATION, authorization)
        } else {
            request
        }
    }
}

fn has_llm_credentials(config: &Config) -> bool {
    config.anthropic_api_key.is_some()
        || config.openrouter_api_key.is_some()
        || config.openai_api_key.is_some()
        || config.openai_codex_token.is_some()
}

/// Returns `true` if config setup should run automatically during boot.
///
/// Messaging setup stays opt-in via `openpaw setup`; we only block startup when
/// no usable LLM credentials are available.
pub fn needs_setup(_data_dir: &Path, config: &Config) -> bool {
    if !std::io::stdin().is_terminal() {
        return false;
    }
    !has_llm_credentials(config)
}

pub fn resolve_llm_credentials(config: &Config) -> Option<(String, String)> {
    match config.llm_provider.as_deref() {
        Some("openai") => config
            .openai_api_key
            .clone()
            .map(|key| ("openai".to_string(), key)),
        Some("openrouter") => config
            .openrouter_api_key
            .clone()
            .map(|key| ("openrouter".to_string(), key)),
        Some("anthropic") => config
            .anthropic_api_key
            .clone()
            .map(|key| ("anthropic".to_string(), key)),
        _ => config
            .anthropic_api_key
            .clone()
            .map(|key| ("anthropic".to_string(), key))
            .or_else(|| {
                config
                    .openrouter_api_key
                    .clone()
                    .map(|key| ("openrouter".to_string(), key))
            })
            .or_else(|| {
                config
                    .openai_api_key
                    .clone()
                    .map(|key| ("openai".to_string(), key))
            })
            .or_else(|| {
                config
                    .openai_codex_token
                    .clone()
                    .map(|key| ("openai".to_string(), key))
            }),
    }
}

/// Phase A: Collect API key and messaging config (runs pre-boot).
pub async fn run_setup_config(config: &Config) -> anyhow::Result<SetupResult> {
    let has_api_key = has_llm_credentials(config);
    let has_discord = config.discord_bot_token.is_some()
        && config.discord_public_key.is_some()
        && crate::transport_manager::can_resolve_discord_public_endpoint(
            config.public_base_url.as_deref(),
            &config.ngrok_bin,
        );
    let has_slack = config.slack_app_token.is_some() && config.slack_bot_token.is_some();

    let mut result = SetupResult {
        api_key: None,
        provider: None,
        discord_bot_token: None,
        discord_public_key: None,
        discord_guild_id: None,
        discord_feed_channel_id: None,
        discord_forum_channel_id: None,
        slack_app_token: None,
        slack_bot_token: None,
    };

    cliclack::intro("Open Paw")?;

    // ─── API Key ───

    if has_api_key {
        cliclack::log::success("API key configured")?;
    } else {
        let provider: &str = cliclack::select("Which AI provider do you use?")
            .item("anthropic", "Anthropic (Claude)", "")
            .item("openrouter", "OpenRouter", "")
            .item("openai", "OpenAI (GPT)", "")
            .interact()?;

        match provider {
            "anthropic" => {
                cliclack::log::info("Get your key at console.anthropic.com → API Keys")?;
            }
            "openrouter" => {
                cliclack::log::info("Get your key at openrouter.ai/keys")?;
            }
            "openai" => {
                cliclack::log::info(
                    "Get your key at platform.openai.com/api-keys\n  Note: ChatGPT Plus/Pro subscriptions don't include API access",
                )?;
            }
            _ => {}
        }

        let key: String = cliclack::password("API key")
            .mask('•')
            .validate(|input: &String| {
                if input.trim().is_empty() {
                    Err("API key is required")
                } else {
                    Ok(())
                }
            })
            .interact()?;

        let key = key.trim().to_string();
        result.api_key = Some(key);
        result.provider = Some(provider.to_string());
    }

    // ─── Messaging Platform ───

    let skip_messaging = if has_discord {
        let reconfigure: bool = cliclack::confirm("Discord is connected. Reconfigure?")
            .initial_value(false)
            .interact()?;
        !reconfigure
    } else if has_slack {
        let reconfigure: bool = cliclack::confirm("Slack is connected. Reconfigure?")
            .initial_value(false)
            .interact()?;
        !reconfigure
    } else {
        false
    };

    if !skip_messaging {
        let platform: &str = cliclack::select("How do you want to talk to Paw?")
            .item("discord", "Discord", "")
            .item("slack", "Slack", "")
            .item("api", "Just the API", "no messaging platform")
            .interact()?;

        match platform {
            "discord" => {
                if !crate::transport_manager::can_resolve_discord_public_endpoint(
                    config.public_base_url.as_deref(),
                    &config.ngrok_bin,
                ) {
                    anyhow::bail!(
                        "Discord setup requires a public interactions URL. Set PUBLIC_BASE_URL or install/configure ngrok (binary: {}) and rerun setup.",
                        config.ngrok_bin
                    );
                }

                let interaction_destination = if let Some(base_url) =
                    config.public_base_url.as_ref()
                {
                    format!("{}/discord/interaction", base_url.trim_end_matches('/'))
                } else {
                    format!(
                        "auto-created locally with {} and exposed as https://.../discord/interaction",
                        config.ngrok_bin
                    )
                };

                cliclack::note(
                    "Discord Bot Setup",
                    format!(
                        "1. Go to discord.com/developers/applications\n\
                     2. Click \"New Application\" → name it\n\
                     3. Click \"Bot\" in the left sidebar\n\
                     4. Click \"Reset Token\" → copy it\n\
                     5. Copy the application Public Key from General Information\n\
                     6. Turn on \"Message Content Intent\" under\n\
                        Privileged Gateway Intents\n\
                     7. Go to OAuth2 → URL Generator\n\
                        Select scope: \"bot\"\n\
                        Select permissions: \"Send Messages\" +\n\
                        \"Read Message History\"\n\
                     8. Copy the URL, open it, pick your server\n\
                     9. Set Interactions Endpoint URL to:\n\
                        {interaction_destination}"
                    ),
                )?;

                let token: String = cliclack::password("Bot token").mask('•').interact()?;
                let token = token.trim().to_string();

                if !token.is_empty() {
                    result.discord_bot_token = Some(token);

                    let public_key: String = cliclack::input("Application Public Key")
                        .placeholder("General Information → Public Key")
                        .validate(|input: &String| {
                            if input.trim().is_empty() {
                                Err("Discord public key is required for interactions")
                            } else {
                                Ok(())
                            }
                        })
                        .interact()?;
                    result.discord_public_key = Some(public_key.trim().to_string());

                    let guild: String = cliclack::input("Guild ID")
                        .placeholder("optional — right-click server → Copy Server ID")
                        .required(false)
                        .interact()?;
                    if !guild.is_empty() {
                        result.discord_guild_id = Some(guild);
                    }

                    let feed: String = cliclack::input("Feed Channel ID")
                        .placeholder("optional")
                        .required(false)
                        .interact()?;
                    if !feed.is_empty() {
                        result.discord_feed_channel_id = Some(feed);
                    }

                    let forum: String = cliclack::input("Forum Channel ID")
                        .placeholder("optional")
                        .required(false)
                        .interact()?;
                    if !forum.is_empty() {
                        result.discord_forum_channel_id = Some(forum);
                    }

                    cliclack::log::success("Discord configured")?;
                }
            }
            "slack" => {
                cliclack::note(
                    "Slack Bot Setup",
                    "1. Go to api.slack.com/apps → Create New App\n\
                     2. Enable Socket Mode → copy the App Token (xapp-...)\n\
                     3. Under OAuth & Permissions, copy the Bot Token (xoxb-...)\n\
                     4. Subscribe to events: message.channels, message.im",
                )?;

                let app_token: String = cliclack::password("App Token (xapp-...)")
                    .mask('•')
                    .interact()?;
                let app_token = app_token.trim().to_string();

                let bot_token: String = cliclack::password("Bot Token (xoxb-...)")
                    .mask('•')
                    .interact()?;
                let bot_token = bot_token.trim().to_string();

                if app_token.is_empty() || bot_token.is_empty() {
                    cliclack::log::warning("Skipped — both tokens required")?;
                } else {
                    result.slack_app_token = Some(app_token);
                    result.slack_bot_token = Some(bot_token);
                    cliclack::log::success("Slack configured")?;
                }
            }
            _ => {
                cliclack::log::info("API only — interact via REST or the dashboard")?;
            }
        }
    }

    cliclack::log::step("Booting server...")?;
    Ok(result)
}

/// Phase B: User interview + LLM soul generation (runs post-boot, writes to TemperFS via OData).
///
/// `api_port` is the local server port for OData calls.
/// `api_key` is the LLM provider key for generating the soul.
/// `provider` is "anthropic", "openrouter", or "openai".
pub async fn run_setup_soul_interview(
    api_key: &str,
    provider_name: &str,
) -> anyhow::Result<Option<GeneratedSoul>> {
    if !std::io::stdin().is_terminal() || api_key.is_empty() {
        return Ok(None);
    }

    cliclack::log::step("Let's make Paw yours.")?;

    let name: String = cliclack::input("What's your name?")
        .placeholder("your name")
        .interact()?;

    let about: String = cliclack::input("Tell Paw about yourself.")
        .placeholder("what you do, what you're working on, what you care about")
        .interact()?;

    let ideal: String = cliclack::input("What kind of Paw do you want?")
        .placeholder("how they think, how they talk, what makes them great to work with")
        .interact()?;

    let provider = LlmProvider::detect(api_key, provider_name);

    // Round 2: LLM-generated follow-ups
    let mut followup_answers = Vec::new();
    let followup_spinner = cliclack::spinner();
    followup_spinner.start("Thinking of something to ask you...");

    match setup_llm::generate_followup_questions(&provider, &name, &about, &ideal).await {
        Ok(questions) => {
            followup_spinner.stop("Got it.");
            for question in questions {
                let answer: String = cliclack::input(&question).placeholder("").interact()?;
                if !answer.is_empty() {
                    followup_answers.push((question, answer));
                }
            }
        }
        Err(e) => {
            followup_spinner.stop(format!("Skipped follow-ups: {e}"));
        }
    }

    // Generate soul
    let interview = UserInterview {
        name: name.clone(),
        about_you: about,
        ideal_paw: ideal,
        followup_answers,
    };

    let soul_spinner = cliclack::spinner();
    soul_spinner.start("Generating your Paw...");

    match setup_llm::generate_personalized_soul(&provider, &interview).await {
        Ok(mut generated) => {
            soul_spinner.stop("Done.");

            // Preview + iteration loop
            loop {
                let choice: &str = cliclack::select(&format!(
                    "Here's what Paw will be like:\n\n  \"{}\"\n",
                    generated.summary
                ))
                .item("accept", "Looks good", "")
                .item("adjust", "Almost — let me adjust", "")
                .item("redo", "Start over", "")
                .interact()?;

                match choice {
                    "accept" => break,
                    "adjust" => {
                        let feedback: String = cliclack::input("What should change?")
                            .placeholder("describe what to adjust")
                            .interact()?;

                        let refine_spinner = cliclack::spinner();
                        refine_spinner.start("Regenerating...");
                        match setup_llm::refine_soul(
                            &provider,
                            &interview,
                            &generated.summary,
                            &feedback,
                        )
                        .await
                        {
                            Ok(refined) => {
                                refine_spinner.stop("Done.");
                                generated = refined;
                            }
                            Err(e) => {
                                refine_spinner.stop(format!("Failed: {e}"));
                                break;
                            }
                        }
                    }
                    "redo" => {
                        let redo_spinner = cliclack::spinner();
                        redo_spinner.start("Regenerating from scratch...");
                        match setup_llm::generate_personalized_soul(&provider, &interview).await {
                            Ok(fresh) => {
                                redo_spinner.stop("Done.");
                                generated = fresh;
                            }
                            Err(e) => {
                                redo_spinner.stop(format!("Failed: {e}"));
                                break;
                            }
                        }
                    }
                    _ => break,
                }
            }
            Ok(Some(generated))
        }
        Err(e) => {
            soul_spinner.stop(format!("Soul generation failed: {e}"));
            cliclack::log::warning(
                "Using default Paw soul. Run `cargo run -- setup` later to personalize.",
            )?;
            Ok(None)
        }
    }
}

pub async fn run_setup_soul(
    api_port: u16,
    api_key: &str,
    provider_name: &str,
    tenant: &str,
    auth: SetupRequestAuth,
) -> anyhow::Result<()> {
    let base = format!("http://127.0.0.1:{api_port}");
    let client = reqwest::Client::new();

    if let Some(generated) = run_setup_soul_interview(api_key, provider_name).await? {
        let save_spinner = cliclack::spinner();
        save_spinner.start("Saving soul to Temper...");
        match save_soul_to_temper(&client, &base, tenant, &generated, &auth).await {
            Ok(()) => {
                save_spinner.stop("Soul saved.");
            }
            Err(e) => {
                save_spinner.stop(format!("Failed to save: {e}"));
                cliclack::log::warning(
                    "Soul generation succeeded but couldn't save to Temper. Run `cargo run -- setup` to retry.",
                )?;
            }
        }
    }

    cliclack::outro("Paw is ready.")?;
    Ok(())
}

fn entity_field_str<'a>(entity: &'a serde_json::Value, field_names: &[&str]) -> Option<&'a str> {
    field_names.iter().find_map(|field_name| {
        entity["fields"][*field_name]
            .as_str()
            .or_else(|| entity[*field_name].as_str())
    })
}

async fn resolve_paw_soul_entity(
    client: &reqwest::Client,
    base: &str,
    tenant: &str,
    auth: &SetupRequestAuth,
) -> anyhow::Result<serde_json::Value> {
    let agent_url = format!("{base}/tdata/Agents?$filter=name eq 'Paw' and Status eq 'Active'");
    let agent_response: serde_json::Value = auth
        .apply(client.get(&agent_url))
        .header("x-tenant-id", tenant)
        .header("x-temper-principal-kind", "admin")
        .send()
        .await?
        .json()
        .await?;

    if let Some(agent) = agent_response["value"].as_array().and_then(|items| items.first()) {
        if let Some(soul_id) = entity_field_str(agent, &["soul_id", "SoulId"]) {
            let soul_url = format!("{base}/tdata/Souls('{soul_id}')");
            let soul_response: serde_json::Value = auth
                .apply(client.get(&soul_url))
                .header("x-tenant-id", tenant)
                .header("x-temper-principal-kind", "admin")
                .send()
                .await?
                .json()
                .await?;

            if entity_field_str(&soul_response, &["ContentFileId", "content_file_id"]).is_some() {
                return Ok(soul_response);
            }
        }
    }

    for filter in ["Name eq 'Paw'", "name eq 'paw'"] {
        let soul_url = format!("{base}/tdata/Souls?$filter={filter}");
        let soul_response: serde_json::Value = auth
            .apply(client.get(&soul_url))
            .header("x-tenant-id", tenant)
            .header("x-temper-principal-kind", "admin")
            .send()
            .await?
            .json()
            .await?;

        if let Some(soul) = soul_response["value"].as_array().and_then(|items| items.first()) {
            return Ok(soul.clone());
        }
    }

    anyhow::bail!("Paw Soul entity not found");
}

pub(crate) async fn load_paw_soul_content(
    client: &reqwest::Client,
    base: &str,
    tenant: &str,
    auth: &SetupRequestAuth,
) -> anyhow::Result<(String, String)> {
    let soul = resolve_paw_soul_entity(client, base, tenant, auth).await?;
    let file_id = entity_field_str(&soul, &["ContentFileId", "content_file_id"])
        .ok_or_else(|| anyhow::anyhow!("Paw Soul has no ContentFileId"))?;

    let content = auth
        .apply(client.get(format!("{base}/tdata/Files('{file_id}')/$value")))
        .header("x-tenant-id", tenant)
        .header("x-temper-principal-kind", "admin")
        .send()
        .await?
        .text()
        .await?;

    let summary = content
        .lines()
        .find(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .unwrap_or("Paw is ready, but not yet personalized.")
        .trim()
        .to_string();

    Ok((summary, content))
}

/// Write the generated soul content to the existing Paw Soul entity in TemperFS.
pub(crate) async fn save_soul_to_temper(
    client: &reqwest::Client,
    base: &str,
    tenant: &str,
    soul: &GeneratedSoul,
    auth: &SetupRequestAuth,
) -> anyhow::Result<()> {
    let paw = resolve_paw_soul_entity(client, base, tenant, auth).await?;
    let file_id = entity_field_str(&paw, &["ContentFileId", "content_file_id"])
        .ok_or_else(|| anyhow::anyhow!("Paw Soul has no ContentFileId"))?;

    // Concatenate soul + style + user + the default AGENT.md (operational instructions)
    let agent_md =
        std::fs::read_to_string("os-apps/paw-agent/agents/paw/AGENT.md").unwrap_or_default();

    let full_content = format!(
        "{}\n\n{}\n\n{}\n\n{}",
        soul.soul_md, soul.style_md, soul.user_md, agent_md
    );

    // Upload to TemperFS via PUT $value
    let upload_url = format!("{base}/tdata/Files('{file_id}')/$value");
    let resp = auth
        .apply(client.put(&upload_url))
        .header("x-tenant-id", tenant)
        .header("x-temper-principal-kind", "admin")
        .header("content-type", "text/markdown")
        .body(full_content)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Upload failed ({status}): {body}");
    }

    Ok(())
}

/// Merge Phase A results into config.
pub fn merge_setup_into_config(config: &mut Config, setup: SetupResult) {
    if let Some(key) = setup.api_key {
        match setup.provider.as_deref() {
            Some("openai") => {
                if config.openai_api_key.is_none() {
                    config.openai_api_key = Some(key);
                }
            }
            Some("openrouter") => {
                if config.openrouter_api_key.is_none() {
                    config.openrouter_api_key = Some(key);
                }
            }
            _ => {
                if config.anthropic_api_key.is_none() {
                    config.anthropic_api_key = Some(key);
                }
            }
        }
    }
    if let Some(provider) = setup.provider {
        config.llm_provider = Some(provider);
    }
    if let Some(token) = setup.discord_bot_token {
        if config.discord_bot_token.is_none() {
            config.discord_bot_token = Some(token);
        }
    }
    if let Some(public_key) = setup.discord_public_key {
        if config.discord_public_key.is_none() {
            config.discord_public_key = Some(public_key);
        }
    }
    if let Some(id) = setup.discord_guild_id {
        if config.discord_guild_id.is_none() {
            config.discord_guild_id = Some(id);
        }
    }
    if let Some(id) = setup.discord_feed_channel_id {
        if config.discord_feed_channel_id.is_none() {
            config.discord_feed_channel_id = Some(id);
        }
    }
    if let Some(id) = setup.discord_forum_channel_id {
        if config.discord_forum_channel_id.is_none() {
            config.discord_forum_channel_id = Some(id);
        }
    }
    if let Some(token) = setup.slack_app_token {
        if config.slack_app_token.is_none() {
            config.slack_app_token = Some(token);
        }
    }
    if let Some(token) = setup.slack_bot_token {
        if config.slack_bot_token.is_none() {
            config.slack_bot_token = Some(token);
        }
    }
}

/// Print a diagnostic report.
pub fn run_doctor(data_dir: &Path, config: &Config) {
    let vault_key_path = data_dir.join("vault.key");
    let db_path = data_dir.join("paw.db");

    println!();
    println!("  Open Paw Doctor");
    println!();

    if data_dir.exists() {
        println!("  \u{2713} Data: {}", data_dir.display());
    } else {
        println!("  \u{2717} Data directory missing");
    }

    if vault_key_path.exists() {
        println!("  \u{2713} Vault key");
    } else if config.vault_key.is_some() {
        println!("  \u{2713} Vault key (env var)");
    } else {
        println!("  \u{2717} Vault key (generated on boot)");
    }

    if db_path.exists() {
        let size = std::fs::metadata(&db_path)
            .map(|m| format!("{:.1} MB", m.len() as f64 / 1_048_576.0))
            .unwrap_or_else(|_| "?".into());
        println!("  \u{2713} Database ({size})");
    } else {
        println!("  ~ Database (created on boot)");
    }

    println!();

    if has_llm_credentials(config) {
        println!("  \u{2713} API key");
    } else {
        println!("  \u{2717} API key — run `cargo run -- setup`");
    }

    if config.discord_bot_token.is_some() {
        println!("  \u{2713} Discord bot token");
        if config.discord_public_key.is_some() {
            println!("  \u{2713} Discord public key");
        } else {
            println!("  \u{2717} Discord public key — rerun `cargo run -- setup`");
        }
        if let Some(base_url) = config.public_base_url.as_ref() {
            println!(
                "  \u{2713} Discord interactions URL: {}/discord/interaction",
                base_url.trim_end_matches('/')
            );
        } else if crate::transport_manager::ngrok_available(&config.ngrok_bin) {
            println!(
                "  \u{2713} Discord interactions URL: auto-tunnel via {} on local runs",
                config.ngrok_bin
            );
        } else {
            println!(
                "  \u{2717} Discord interactions URL — set PUBLIC_BASE_URL or install/configure {}",
                config.ngrok_bin
            );
        }
    } else {
        println!("  \u{2717} Discord — run `cargo run -- setup`");
    }

    if config.slack_app_token.is_some() && config.slack_bot_token.is_some() {
        println!("  \u{2713} Slack");
    } else {
        println!("  ~ Slack");
    }

    println!();
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::Router;
    use axum::body::{Body, to_bytes};
    use axum::extract::State;
    use axum::http::{HeaderMap, Method, Request, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::any;
    use serde_json::json;

    use super::{GeneratedSoul, SetupRequestAuth, resolve_llm_credentials, save_soul_to_temper};

    #[test]
    fn resolve_llm_credentials_respects_provider_hint() {
        let mut config = crate::config::Config::from_env().unwrap();
        config.llm_provider = Some("openrouter".to_string());
        config.anthropic_api_key = Some("anthropic-key".to_string());
        config.openrouter_api_key = Some("openrouter-key".to_string());

        let resolved = resolve_llm_credentials(&config).unwrap();
        assert_eq!(resolved.0, "openrouter");
        assert_eq!(resolved.1, "openrouter-key");
    }

    #[tokio::test]
    async fn save_soul_to_temper_forwards_cookie_auth() {
        #[derive(Clone, Default)]
        struct SeenRequests {
            cookies: Arc<Mutex<Vec<String>>>,
            bodies: Arc<Mutex<Vec<String>>>,
        }

        async fn handler(
            State(state): State<SeenRequests>,
            headers: HeaderMap,
            request: Request<Body>,
        ) -> impl IntoResponse {
            if let Some(cookie) = headers.get("cookie").and_then(|value| value.to_str().ok()) {
                state.cookies.lock().unwrap().push(cookie.to_string());
            }

            match (request.method(), request.uri().path()) {
                (&Method::GET, "/tdata/Agents") => (
                    StatusCode::OK,
                    axum::Json(json!({
                        "value": [{
                            "fields": {
                                "soul_id": "soul-1"
                            }
                        }]
                    })),
                )
                    .into_response(),
                (&Method::GET, "/tdata/Souls('soul-1')") => (
                    StatusCode::OK,
                    axum::Json(json!({
                        "fields": {
                            "ContentFileId": "file-1"
                        }
                    })),
                )
                    .into_response(),
                (&Method::GET, "/tdata/Souls") => (
                    StatusCode::OK,
                    axum::Json(json!({
                        "value": [{
                            "fields": {
                                "ContentFileId": "file-1"
                            }
                        }]
                    })),
                )
                    .into_response(),
                (&Method::PUT, "/tdata/Files('file-1')/$value") => {
                    let body = to_bytes(request.into_body(), usize::MAX).await.unwrap();
                    state
                        .bodies
                        .lock()
                        .unwrap()
                        .push(String::from_utf8(body.to_vec()).unwrap());
                    StatusCode::OK.into_response()
                }
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }

        let seen = SeenRequests::default();
        let app = Router::new()
            .fallback(any(handler))
            .with_state(seen.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let soul = GeneratedSoul {
            summary: "Thoughtful collaborator".to_string(),
            soul_md: "# Soul".to_string(),
            style_md: "# Style".to_string(),
            user_md: "# User".to_string(),
        };

        save_soul_to_temper(
            &reqwest::Client::new(),
            &format!("http://{addr}"),
            "default",
            &soul,
            &SetupRequestAuth::from_cookie("paw_session=test-cookie"),
        )
        .await
        .unwrap();

        assert_eq!(
            seen.cookies.lock().unwrap().as_slice(),
            [
                "paw_session=test-cookie",
                "paw_session=test-cookie",
                "paw_session=test-cookie"
            ]
        );
        assert!(
            seen.bodies
                .lock()
                .unwrap()
                .first()
                .map(|body| body.contains("# Soul")
                    && body.contains("# Style")
                    && body.contains("# User"))
                .unwrap_or(false)
        );
    }

    #[tokio::test]
    async fn save_soul_to_temper_falls_back_to_named_paw_soul() {
        async fn handler(request: Request<Body>) -> impl IntoResponse {
            match (request.method(), request.uri().path(), request.uri().query()) {
                (&Method::GET, "/tdata/Agents", _) => (
                    StatusCode::OK,
                    axum::Json(json!({
                        "value": [{
                            "fields": {}
                        }]
                    })),
                )
                    .into_response(),
                (&Method::GET, "/tdata/Souls", Some(query))
                    if query.contains("Name%20eq%20%27Paw%27") =>
                {
                    (
                        StatusCode::OK,
                        axum::Json(json!({
                            "value": [{
                                "fields": {
                                    "ContentFileId": "file-1"
                                }
                            }]
                        })),
                    )
                        .into_response()
                }
                (&Method::GET, "/tdata/Souls", _) => (
                    StatusCode::OK,
                    axum::Json(json!({
                        "value": []
                    })),
                )
                    .into_response(),
                (&Method::PUT, "/tdata/Files('file-1')/$value", _) => {
                    StatusCode::OK.into_response()
                }
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }

        let app = Router::new().fallback(any(handler));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let soul = GeneratedSoul {
            summary: "Thoughtful collaborator".to_string(),
            soul_md: "# Soul".to_string(),
            style_md: "# Style".to_string(),
            user_md: "# User".to_string(),
        };

        save_soul_to_temper(
            &reqwest::Client::new(),
            &format!("http://{addr}"),
            "default",
            &soul,
            &SetupRequestAuth::default(),
        )
        .await
        .unwrap();
    }
}
