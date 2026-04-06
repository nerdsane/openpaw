//! First-run interactive setup for Open Paw.
//!
//! When no vault key file exists and no critical env vars are set, the binary
//! enters an interactive setup flow before booting the platform. This lets a
//! human (or their agent piping stdin) configure the essentials in one shot.

use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::config::Config;

/// Result of the first-run setup wizard.
pub struct SetupResult {
    pub anthropic_api_key: String,
    pub discord_bot_token: Option<String>,
    pub discord_public_key: Option<String>,
    pub discord_guild_id: Option<String>,
    pub discord_feed_channel_id: Option<String>,
    pub discord_forum_channel_id: Option<String>,
}

/// Returns `true` if the platform has never been configured and needs first-run setup.
///
/// Detection: no vault.key file AND no ANTHROPIC_API_KEY env var AND no TEMPER_VAULT_KEY env var.
pub fn needs_first_run_setup(data_dir: &Path, config: &Config) -> bool {
    let vault_key_path = data_dir.join("vault.key");
    !vault_key_path.exists() && config.vault_key.is_none() && config.anthropic_api_key.is_none()
}

/// Run the interactive first-run setup wizard.
///
/// Prompts the user for essential configuration and returns the collected values.
/// The caller is responsible for merging these into the `Config`.
pub fn run_first_run_setup() -> anyhow::Result<SetupResult> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = io::stdout();

    println!();
    println!("  Welcome to Open Paw!");
    println!("  First time? Let's get you set up.");
    println!();

    // Anthropic API key (required)
    let anthropic_api_key = loop {
        print!("  Anthropic API Key: ");
        stdout.flush()?;
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let key = line.trim().to_string();
        if key.is_empty() {
            println!("  API key is required to run agents.");
            continue;
        }
        println!("  \u{2713} Key saved");
        println!();
        break key;
    };

    // Discord (optional)
    print!("  Connect Discord? (y/N): ");
    stdout.flush()?;
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let connect_discord = line.trim().eq_ignore_ascii_case("y");

    let (
        discord_bot_token,
        discord_public_key,
        discord_guild_id,
        discord_feed_channel_id,
        discord_forum_channel_id,
    ) = if connect_discord {
        println!();

        print!("  Discord Bot Token: ");
        stdout.flush()?;
        let mut token_line = String::new();
        reader.read_line(&mut token_line)?;
        let bot_token = token_line.trim().to_string();
        if bot_token.is_empty() {
            println!("  Skipping Discord (no token provided).");
            (None, None, None, None, None)
        } else {
            println!("  \u{2713} Token saved");

            let public_key = prompt_optional(
                &mut reader,
                &mut stdout,
                "  Discord Public Key (optional): ",
            )?;
            let guild_id =
                prompt_optional(&mut reader, &mut stdout, "  Discord Guild ID (optional): ")?;
            let feed_channel_id = prompt_optional(
                &mut reader,
                &mut stdout,
                "  Discord Feed Channel ID (optional): ",
            )?;
            let forum_channel_id = prompt_optional(
                &mut reader,
                &mut stdout,
                "  Discord Forum Channel ID (optional): ",
            )?;

            (
                Some(bot_token),
                public_key,
                guild_id,
                feed_channel_id,
                forum_channel_id,
            )
        }
    } else {
        (None, None, None, None, None)
    };

    println!();
    println!("  Starting Open Paw...");
    println!();

    Ok(SetupResult {
        anthropic_api_key,
        discord_bot_token,
        discord_public_key,
        discord_guild_id,
        discord_feed_channel_id,
        discord_forum_channel_id,
    })
}

/// Merge setup results into the config (only sets fields that are currently None).
pub fn merge_setup_into_config(config: &mut Config, setup: SetupResult) {
    if config.anthropic_api_key.is_none() {
        config.anthropic_api_key = Some(setup.anthropic_api_key);
    }
    if config.discord_bot_token.is_none() {
        config.discord_bot_token = setup.discord_bot_token;
    }
    if config.discord_public_key.is_none() {
        config.discord_public_key = setup.discord_public_key;
    }
    if config.discord_guild_id.is_none() {
        config.discord_guild_id = setup.discord_guild_id;
    }
    if config.discord_feed_channel_id.is_none() {
        config.discord_feed_channel_id = setup.discord_feed_channel_id;
    }
    if config.discord_forum_channel_id.is_none() {
        config.discord_forum_channel_id = setup.discord_forum_channel_id;
    }
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
