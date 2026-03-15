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
use std::env;
use std::process;
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
    let cli = Cli::parse();

    // Handle 'auth sign' locally without server connection
    if let Some(Commands::Auth { sub: AuthCommands::Sign { challenge } }) = &cli.command {
        match PharosClient::sign_message_async(challenge).await {
            Ok((pub_key, sig)) => {
                println!("Public Key: {}", pub_key);
                println!("Signature:  {}", sig);
                return Ok(());
            }
            Err(e) => {
                eprintln!("Error signing challenge: {}", e);
                process::exit(1);
            }
        }
    }

    // Legacy fallback/Direct query support
    let query_string = if let Some(_) = &cli.command {
        // If it was a recognized subcommand that didn't exit (none yet except auth)
        String::new() 
    } else if !cli.query.is_empty() {
        cli.query.join(" ")
    } else {
        // No command provided
        eprintln!("Usage: mdb [-H] <query>");
        process::exit(1);
    };

    if query_string.is_empty() {
        return Ok(());
    }

    let addr = if let Ok(server) = env::var("PHAROS_SERVER") {
        server
    } else {
        let host = env::var("PHAROS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("PHAROS_PORT").unwrap_or_else(|_| "2378".to_string());
        format!("{}:{}", host, port)
    };

    let mut client = PharosClient::connect(&addr, "mdb").await
        .with_context(|| format!("Failed to connect to Pharos server at {}", addr))?;

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

    match client.execute_authenticated(&cmd_to_send).await {
        Ok(resp) => {
            match resp {
                PharosResponse::Ok(msg) => println!("{}", msg),
                PharosResponse::Matches { records, .. } => {
                    for record in records {
                        for field in record.fields {
                            let value = if cli.human {
                                format_human(&field.key, &field.value)
                            } else {
                                field.value
                            };
                            println!("{:>15}: {}", field.key, value);
                        }
                    }
                }
                PharosResponse::Error { code, message } => {
                    eprintln!("{}: {}", code, message);
                    process::exit(1);
                }
                PharosResponse::AuthenticationRequired { .. } => {
                    eprintln!("Authentication failed.");
                    process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }

    let _ = client.quit().await;
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
