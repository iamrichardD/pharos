/* ========================================================================
 * Project: pharos
 * Component: CLI-ph
 * File: ph/src/main.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This is the entry point for the 'ph' CLI client, used for people contacts
 * using the RFC 2378 protocol. It leverages the shared 'pharos-client'
 * library for robust, async communication and authentication.
 * * Traceability:
 * Related to Task 10.1 (Issue #39), refactored for clap (Task 22.4).
 * ======================================================================== */

use pharos_client::{PharosClient, PharosResponse};
use std::env;
use std::process;
use anyhow::{Result, Context};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ph")]
#[command(about = "Pharos People Contacts (Ph) CLI", long_about = None)]
struct Cli {
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
        String::new() 
    } else if !cli.query.is_empty() {
        cli.query.join(" ")
    } else {
        eprintln!("Usage: ph <query>");
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

    let mut client = PharosClient::connect(&addr, "ph").await
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
                            println!("{:>15}: {}", field.key, field.value);
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
