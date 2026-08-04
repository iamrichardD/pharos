/* ========================================================================
 * Project: pharos
 * Component: CLI-mdb
 * File: mdb/src/main.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This is the entry point for the 'mdb' CLI client, used for machine/infrastructure
 * assets using the RFC 2378 protocol. It leverages the shared 'pharos-client'
 * library for robust, async communication and authentication. 
 * Supports human-readable output formatting for units and timestamps.
 * * Traceability:
 * Related to Task 22.4 (Issue #141), implements human-readable flags.
 * ======================================================================== */

use pharos_client::{PharosClient, PharosResponse};
use std::process;
use std::io::{self, IsTerminal};
use anyhow::{Result, Context};
use clap::{Parser, Subcommand};
use chrono::DateTime;

#[derive(Parser)]
#[command(name = "mdb")]
#[command(about = "Pharos Machine Database (MDB) CLI", long_about = None)]
struct Cli {
    /// Enable human-readable output (units and timestamps)
    #[arg(short = 'H', long = "human")]
    human: bool,

    #[command(subcommand)]
    command: Option<Commands>,

    /// Raw query or command string (fallback)
    query: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate a challenge locally
    Auth {
        /// The sign command (e.g., 'sign [challenge]')
        #[command(subcommand)]
        sub: AuthCommands,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Sign a challenge string
    Sign {
        /// The challenge string to sign
        challenge: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_log::LogTracer::init().ok(); // bridges the `log` facade (used by pharos-client) into tracing
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .try_init();

    let cli = Cli::parse();

    // Handle 'auth sign' locally without server connection
    if let Some(Commands::Auth { sub: AuthCommands::Sign { challenge } }) = &cli.command {
        let res = PharosClient::sign_message_async(challenge).await;
        let (pub_key, sig) = match res {
            Ok(pair) => pair,
            Err(e) if pharos_cli_support::is_missing_key_error(&e) && io::stdin().is_terminal() => {
                let key_path = pharos_cli_support::default_personal_key_path()?;
                if pharos_cli_support::offer_to_generate_key(&key_path)? {
                    PharosClient::sign_message_async(challenge).await
                        .context("Error signing challenge")?
                } else {
                    return Err(e).context("Error signing challenge");
                }
            }
            Err(e) => return Err(e).context("Error signing challenge"),
        };
        println!("Public Key: {}", pub_key);
        println!("Signature:  {}", sig);
        return Ok(());
    }

    // Legacy fallback/Direct query support
    let query_string = if cli.command.is_some() {
        // If it was a recognized subcommand that didn't exit (none yet except auth)
        String::new() 
    } else if !cli.query.is_empty() {
        pharos_client::join_wire_args(&cli.query)
    } else {
        // No command provided
        eprintln!("Usage: mdb [-H] <query>");
        process::exit(1);
    };

    if query_string.is_empty() {
        return Ok(());
    }

    let (addr, addr_source) = pharos_cli_support::resolve_server_address();

    let mut client = PharosClient::connect(&addr, "mdb").await
        .with_context(|| format!("Failed to connect to Pharos server at {} (resolved from {})", addr, addr_source))?;

    let lower_cmd = query_string.to_lowercase();
    let is_query = lower_cmd.starts_with("query ") || lower_cmd.starts_with("ph ");
    
    let cmd_to_send = if is_query {
        query_string
    } else {
        let first_word = lower_cmd.split_whitespace().next().unwrap_or("");
        match first_word {
            "add" | "change" | "delete" | "status" | "siteinfo" | "quit" => query_string,
            _ => format!("query {}", query_string),
        }
    };

    let resp = pharos_cli_support::execute_with_interactive_setup(&mut client, &cmd_to_send).await
        .context("Error executing command")?;

    handle_response(resp, cli.human)?;

    let _ = client.quit().await;
    Ok(())
}

fn handle_response(resp: PharosResponse, human: bool) -> Result<()> {
    match resp {
        PharosResponse::Ok(msg) => println!("{}", msg),
        PharosResponse::Matches { records, .. } => {
            if records.is_empty() {
                println!("No matches found.");
            } else {
                for record in records {
                    for field in record.fields {
                        let value = if human {
                            format_human(&field.key, &field.value)
                        } else {
                            field.value
                        };
                        println!("{:>15}: {}", field.key, value);
                    }
                }
            }
        }
        PharosResponse::Error { code, message } => {
            anyhow::bail!("{}: {}", code, message);
        }
        PharosResponse::AuthenticationRequired { .. } => {
            anyhow::bail!("Authentication failed.");
        }
    }
    Ok(())
}

/// Formats raw protocol values into human-readable strings.
fn format_human(key: &str, value: &str) -> String {
    let lower_key = key.to_lowercase();
    
    // 1. Memory/Storage conversions
    if lower_key.ends_with("_kb") {
        if let Ok(kb) = value.parse::<f64>() {
            return format_bytes(kb * 1024.0);
        }
    } else if lower_key.ends_with("_bytes") {
        if let Ok(bytes) = value.parse::<f64>() {
            return format_bytes(bytes);
        }
    } else if lower_key.ends_with("_mb") {
        if let Ok(mb) = value.parse::<f64>() {
            return format_bytes(mb * 1024.0 * 1024.0);
        }
    }

    // 2. Timestamp conversions
    if lower_key.ends_with("_at") || lower_key == "created" || lower_key == "updated" {
        if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
            return dt.format("%Y-%m-%d %H:%M:%S").to_string();
        }
    }

    value.to_string()
}

/// Helper to scale bytes to human-readable units.
fn format_bytes(bytes: f64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut size = bytes;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < units.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", size, units[unit_idx])
    } else {
        format!("{:.1} {}", size, units[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_format_kb_to_gb_when_large() {
        let result = format_human("mem_total_kb", "16777216");
        assert_eq!(result, "16.0 GB");
    }

    #[test]
    fn test_should_format_bytes_to_mb_when_appropriate() {
        let result = format_human("disk_free_bytes", "1048576");
        assert_eq!(result, "1.0 MB");
    }

    #[test]
    fn test_should_format_iso_timestamp_to_clean_string() {
        let result = format_human("created_at", "2026-03-15T14:30:00Z");
        assert_eq!(result, "2026-03-15 14:30:00");
    }

    #[test]
    fn test_should_preserve_non_matching_keys() {
        let result = format_human("hostname", "pharos-main");
        assert_eq!(result, "pharos-main");
    }

    #[test]
    fn test_should_handle_invalid_numeric_values_gracefully() {
        let result = format_human("mem_total_kb", "invalid");
        assert_eq!(result, "invalid");
    }
}

