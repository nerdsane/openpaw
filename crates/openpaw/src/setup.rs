//! Interactive setup for Open Paw.
//!
//! Beautiful cliclack-powered TUI that:
//! 1. Collects API key (auto-detects provider) and messaging platform config
//! 2. Interviews the user to understand who they are and what they want
//! 3. Calls the LLM to generate a personalized Paw soul + user profile
//! 4. Lets the user preview and iterate on the generated soul

use std::io::IsTerminal;
use std::path::Path;

use crate::config::Config;
use crate::setup_llm::{
    self, GeneratedSoul, LlmProvider, UserInterview,
};

/// Result of the setup wizard.
pub struct SetupResult {
    /// LLM API key (Anthropic, OpenRouter, or OpenAI)
    pub api_key: Option<String>,
    /// Provider name: "anthropic", "openrouter", or "openai"
    pub provider: Option<String>,
    pub discord_bot_token: Option<String>,
    pub discord_guild_id: Option<String>,
    pub discord_feed_channel_id: Option<String>,
    pub discord_forum_channel_id: Option<String>,
    pub slack_app_token: Option<String>,
    pub slack_bot_token: Option<String>,
}

/// Returns `true` if interactive setup should run.
pub fn needs_setup(_data_dir: &Path, config: &Config) -> bool {
    if !std::io::stdin().is_terminal() {
        return false;
    }
    let has_api_key = config.anthropic_api_key.is_some();
    let has_messaging = config.discord_bot_token.is_some()
        || (config.slack_app_token.is_some() && config.slack_bot_token.is_some());
    !has_api_key || !has_messaging
}

/// Run the full interactive setup wizard.
pub async fn run_setup(data_dir: &Path, config: &Config) -> anyhow::Result<SetupResult> {
    let has_api_key = config.anthropic_api_key.is_some();
    let has_discord = config.discord_bot_token.is_some();
    let has_slack = config.slack_app_token.is_some() && config.slack_bot_token.is_some();
    let generated_dir = data_dir.join("generated");
    let has_personalization = generated_dir.join("paw-soul.md").exists();

    let mut result = SetupResult {
        api_key: None,
        provider: None,
        discord_bot_token: None,
        discord_guild_id: None,
        discord_feed_channel_id: None,
        discord_forum_channel_id: None,
        slack_app_token: None,
        slack_bot_token: None,
    };

    cliclack::intro("Open Paw")?;

    // ─── Part A: API Key ───

    let (api_key, provider_name) = if has_api_key {
        cliclack::log::success("API key configured")?;
        (config.anthropic_api_key.clone().unwrap_or_default(), "anthropic".to_string())
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
                    "Get your key at platform.openai.com/api-keys\n  Note: ChatGPT Plus/Pro subscriptions don't include API access"
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
        result.api_key = Some(key.clone());
        result.provider = Some(provider.to_string());
        (key, provider.to_string())
    };

    // ─── Part A2: Messaging Platform ───

    if has_discord {
        cliclack::log::success("Discord connected")?;
    } else if has_slack {
        cliclack::log::success("Slack connected")?;
    } else {
        let platform: &str = cliclack::select("How do you want to talk to Paw?")
            .item("discord", "Discord", "")
            .item("slack", "Slack", "")
            .item("api", "Just the API", "no messaging platform")
            .interact()?;

        match platform {
            "discord" => {
                cliclack::note(
                    "Discord Bot Setup",
                    "1. Go to discord.com/developers/applications\n\
                     2. Click \"New Application\" → name it\n\
                     3. Click \"Bot\" in the left sidebar\n\
                     4. Click \"Reset Token\" → copy it\n\
                     5. Turn on \"Message Content Intent\" under\n\
                        Privileged Gateway Intents\n\
                     6. Go to OAuth2 → URL Generator\n\
                        Select scope: \"bot\"\n\
                        Select permissions: \"Send Messages\" +\n\
                        \"Read Message History\"\n\
                     7. Copy the URL, open it, pick your server"
                )?;

                let token: String = cliclack::password("Bot token")
                    .mask('•')
                    .interact()?;
                let token = token.trim().to_string();

                if !token.is_empty() {
                    result.discord_bot_token = Some(token);

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
                     4. Subscribe to events: message.channels, message.im"
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

    // ─── Part B: User Interview + Soul Generation ───

    if has_personalization {
        let redo: bool = cliclack::confirm("Paw is already personalized. Redo?")
            .initial_value(false)
            .interact()?;
        if !redo {
            cliclack::outro("Setup complete.")?;
            return Ok(result);
        }
    }

    // Only run interview if we have an API key to call the LLM
    if !api_key.is_empty() {
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

        let provider = LlmProvider::detect(&api_key, &provider_name);

        // Round 2: LLM-generated follow-ups
        let mut followup_answers = Vec::new();
        let followup_spinner = cliclack::spinner();
        followup_spinner.start("Thinking of something to ask you...");

        match setup_llm::generate_followup_questions(&provider, &name, &about, &ideal).await {
            Ok(questions) => {
                followup_spinner.stop("Got it.");
                for question in questions {
                    let answer: String = cliclack::input(&question)
                        .placeholder("")
                        .interact()?;
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
                            match setup_llm::generate_personalized_soul(&provider, &interview).await
                            {
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

                // Save generated files
                save_generated_soul(data_dir, &generated)?;
                cliclack::log::success("Soul saved")?;
            }
            Err(e) => {
                soul_spinner.stop(format!("Soul generation failed: {e}"));
                cliclack::log::warning("Using default Paw soul. Run `openpaw setup` later to personalize.")?;
            }
        }
    }

    cliclack::outro("Paw is ready.")?;

    Ok(result)
}

/// Merge setup results into the config.
pub fn merge_setup_into_config(config: &mut Config, setup: SetupResult) {
    // Store the API key. For now all providers' keys go to anthropic_api_key
    // in config (it's used by the vault seeding in startup.rs). The provider
    // name is stored separately so the LLM caller can use the right API.
    //
    if let Some(key) = setup.api_key {
        if config.anthropic_api_key.is_none() {
            config.anthropic_api_key = Some(key);
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
    let generated_dir = data_dir.join("generated");

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

    if config.anthropic_api_key.is_some() {
        println!("  \u{2713} API key");
    } else {
        println!("  \u{2717} API key — run `openpaw setup`");
    }

    if config.discord_bot_token.is_some() {
        println!("  \u{2713} Discord");
    } else {
        println!("  \u{2717} Discord — run `openpaw setup`");
    }

    if config.slack_app_token.is_some() && config.slack_bot_token.is_some() {
        println!("  \u{2713} Slack");
    } else {
        println!("  ~ Slack");
    }

    if generated_dir.join("paw-soul.md").exists() {
        println!("  \u{2713} Personalized soul");
    } else {
        println!("  ~ Default soul (run `openpaw setup` to personalize)");
    }

    println!();
}

fn save_generated_soul(data_dir: &Path, soul: &GeneratedSoul) -> anyhow::Result<()> {
    let dir = data_dir.join("generated");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("paw-soul.md"), &soul.soul_md)?;
    std::fs::write(dir.join("paw-style.md"), &soul.style_md)?;
    std::fs::write(dir.join("user.md"), &soul.user_md)?;
    Ok(())
}
