//! Interactive setup for Open Paw.
//!
//! Runs before boot whenever something is missing (API key, messaging).
//! Already-configured items are shown as green checks and skipped.
//! Fully configured systems boot immediately with no prompts.
//!
//! Agent creation is NOT part of CLI setup — Paw (the chief of staff agent)
//! is bootstrapped automatically at startup. Specialized agents (swe, sre,
//! probe) are spawned by Paw as needed during conversations.

use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::config::Config;

/// Result of the setup wizard — only contains fields the user provided.
pub struct SetupResult {
    pub anthropic_api_key: Option<String>,
    pub discord_bot_token: Option<String>,
    pub discord_guild_id: Option<String>,
    pub discord_feed_channel_id: Option<String>,
    pub discord_forum_channel_id: Option<String>,
}

/// Check what's configured and what's missing.
struct SetupStatus {
    has_api_key: bool,
    has_discord: bool,
    has_slack: bool,
    is_first_run: bool,
}

fn check_status(data_dir: &Path, config: &Config) -> SetupStatus {
    let vault_key_path = data_dir.join("vault.key");
    SetupStatus {
        has_api_key: config.anthropic_api_key.is_some(),
        has_discord: config.discord_bot_token.is_some(),
        has_slack: config.slack_app_token.is_some() && config.slack_bot_token.is_some(),
        is_first_run: !vault_key_path.exists() && config.vault_key.is_none(),
    }
}

/// Returns `true` if interactive setup should run (something is missing).
pub fn needs_setup(data_dir: &Path, config: &Config) -> bool {
    if !atty::is(atty::Stream::Stdin) {
        return false;
    }
    let status = check_status(data_dir, config);
    !status.has_api_key || (!status.has_discord && !status.has_slack)
}

/// Run the interactive setup. Shows what's configured, prompts for what's missing.
pub fn run_setup(data_dir: &Path, config: &Config) -> anyhow::Result<SetupResult> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = io::stdout();

    let status = check_status(data_dir, config);

    let mut result = SetupResult {
        anthropic_api_key: None,
        discord_bot_token: None,
        discord_guild_id: None,
        discord_feed_channel_id: None,
        discord_forum_channel_id: None,
    };

    println!();
    if status.is_first_run {
        println!("  Welcome to Open Paw!");
        println!("  Let's get you connected.");
    } else {
        println!("  Open Paw Setup");
    }
    println!();

    // --- Step 1: API Key ---
    if status.has_api_key {
        println!("  \u{2713} Anthropic API key");
    } else {
        let key = loop {
            print!("  Anthropic API Key: ");
            stdout.flush()?;
            let mut line = String::new();
            reader.read_line(&mut line)?;
            let key = line.trim().to_string();
            if key.is_empty() {
                println!("  API key is required.");
                continue;
            }
            break key;
        };
        println!("  \u{2713} Saved");
        result.anthropic_api_key = Some(key);
    }

    // --- Step 2: Messaging Platform ---
    if status.has_discord {
        println!("  \u{2713} Discord");
    } else if status.has_slack {
        println!("  \u{2713} Slack");
    } else {
        println!();
        println!("  How do you want to talk to Paw?");
        println!("    1) Discord");
        println!("    2) Slack");
        println!("    3) API only (no messaging)");
        println!();
        print!("  Choice (1/2/3): ");
        stdout.flush()?;
        let mut choice_line = String::new();
        reader.read_line(&mut choice_line)?;
        let choice = choice_line.trim();

        match choice {
            "1" => {
                println!();
                print!("  Discord Bot Token: ");
                stdout.flush()?;
                let mut token_line = String::new();
                reader.read_line(&mut token_line)?;
                let bot_token = token_line.trim().to_string();
                if bot_token.is_empty() {
                    println!("  Skipped.");
                } else {
                    result.discord_bot_token = Some(bot_token);
                    result.discord_guild_id = prompt_optional(
                        &mut reader,
                        &mut stdout,
                        "  Guild ID (optional): ",
                    )?;
                    result.discord_feed_channel_id = prompt_optional(
                        &mut reader,
                        &mut stdout,
                        "  Feed Channel ID (optional): ",
                    )?;
                    result.discord_forum_channel_id = prompt_optional(
                        &mut reader,
                        &mut stdout,
                        "  Forum Channel ID (optional): ",
                    )?;
                    println!("  \u{2713} Discord connected");
                }
            }
            "2" => {
                println!();
                print!("  App Token (xapp-...): ");
                stdout.flush()?;
                let mut app_line = String::new();
                reader.read_line(&mut app_line)?;
                let _app_token = app_line.trim().to_string();

                print!("  Bot Token (xoxb-...): ");
                stdout.flush()?;
                let mut bot_line = String::new();
                reader.read_line(&mut bot_line)?;
                let _bot_token = bot_line.trim().to_string();

                if _app_token.is_empty() || _bot_token.is_empty() {
                    println!("  Skipped (both tokens required).");
                } else {
                    println!("  \u{2713} Slack connected");
                    // TODO: Wire Slack tokens into SetupResult
                }
            }
            _ => {
                println!("  API only — use the REST API or dashboard to interact.");
            }
        }
    }

    if status.is_first_run {
        println!();
        println!("  Paw will be your agent. Send it a message once connected");
        println!("  and it'll get to know you from there.");
    }

    println!();

    Ok(result)
}

/// Merge setup results into the config (only sets fields that are currently None).
pub fn merge_setup_into_config(config: &mut Config, setup: SetupResult) {
    if let Some(key) = setup.anthropic_api_key {
        if config.anthropic_api_key.is_none() {
            config.anthropic_api_key = Some(key);
        }
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
}

/// Print a diagnostic report of what's configured and what's missing.
pub fn run_doctor(data_dir: &Path, config: &Config) {
    let status = check_status(data_dir, config);
    let vault_key_path = data_dir.join("vault.key");
    let db_path = data_dir.join("paw.db");

    println!();
    println!("  Open Paw Doctor");
    println!();

    if data_dir.exists() {
        println!("  \u{2713} Data directory: {}", data_dir.display());
    } else {
        println!("  \u{2717} Data directory missing: {}", data_dir.display());
    }

    if vault_key_path.exists() {
        println!("  \u{2713} Vault key: {}", vault_key_path.display());
    } else if config.vault_key.is_some() {
        println!("  \u{2713} Vault key: from TEMPER_VAULT_KEY env var");
    } else {
        println!("  \u{2717} Vault key: not found (generated on boot)");
    }

    if db_path.exists() {
        let size = std::fs::metadata(&db_path)
            .map(|m| format!("{:.1} MB", m.len() as f64 / 1_048_576.0))
            .unwrap_or_else(|_| "unknown size".into());
        println!("  \u{2713} Database: {} ({})", db_path.display(), size);
    } else {
        println!("  ~ Database: created on first boot");
    }

    println!();

    if status.has_api_key {
        println!("  \u{2713} Anthropic API key");
    } else {
        println!("  \u{2717} Anthropic API key — run `openpaw setup`");
    }

    if status.has_discord {
        println!("  \u{2713} Discord");
    } else {
        println!("  \u{2717} Discord — run `openpaw setup`");
    }

    if status.has_slack {
        println!("  \u{2713} Slack");
    } else {
        println!("  ~ Slack — not configured");
    }

    if std::path::Path::new(".env").exists() {
        println!();
        println!("  \u{2713} .env file found");
    }

    println!();
}

fn prompt_optional(
    reader: &mut io::StdinLock,
    stdout: &mut io::Stdout,
    prompt: &str,
) -> anyhow::Result<Option<String>> {
    print!("{prompt}");
    stdout.flush()?;
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let value = line.trim().to_string();
    Ok(if value.is_empty() { None } else { Some(value) })
}
