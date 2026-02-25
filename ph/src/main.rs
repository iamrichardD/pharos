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
 * Related to Task 10.1 (Issue #39), Refactoring from Task 3.1
 * ======================================================================== */

use pharos_client::{PharosClient, PharosResponse};
use std::env;
use std::process;
use anyhow::{Result, Context};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: ph <query>");
        process::exit(1);
    }

    let query_string = args.join(" ");
    
    let host = env::var("PHAROS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PHAROS_PORT").unwrap_or_else(|_| "1050".to_string());
    let addr = format!("{}:{}", host, port);

    let mut client = PharosClient::connect(&addr, "ph").await
        .context("Failed to connect to Pharos server")?;

    // Determine command type
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
                    // This should be handled by execute_authenticated
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
