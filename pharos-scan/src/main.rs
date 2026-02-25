/* ========================================================================
 * Project: pharos
 * Component: Network Scanner (pharos-scan)
 * File: pharos-scan/src/main.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This is the entry point for 'pharos-scan', a tool that automates the
 * discovery of network infrastructure and facilitates bulk provisioning
 * into the Pharos ecosystem.
 * * Traceability:
 * Related to Task 10.2 (Issue #40)
 * ======================================================================== */

use anyhow::Result;
use tracing::{info, Level, warn, error, debug};
use tracing_subscriber::FmtSubscriber;
use pharos_scan::engine::ScannerEngine;
use pharos_scan::fingerprint::Fingerprinter;
use pharos_client::{PharosClient, PharosResponse};
use std::env;
use inquire::{MultiSelect, Text};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting pharos-scan...");

    let engine = ScannerEngine::default();
    
    // 1. Discover nodes via mDNS
    let mut nodes = engine.discover_mdns().await?;
    if nodes.is_empty() {
        warn!("No nodes discovered via mDNS.");
        return Ok(());
    }
    info!("Found {} nodes via mDNS", nodes.len());

    // 2. Connect to Pharos to check for existing records
    let host = env::var("PHAROS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PHAROS_PORT").unwrap_or_else(|_| "1050".to_string());
    let addr = format!("{}:{}", host, port);

    let mut client = match PharosClient::connect(&addr, "pharos-scan").await {
        Ok(c) => Some(c),
        Err(e) => {
            warn!("Could not connect to Pharos server at {}: {}. Skipping existence check.", addr, e);
            None
        }
    };

    // 3. Probe and Fingerprint nodes
    for node in &mut nodes {
        debug!("Probing node: {:?}", node.ip);
        node.ports = engine.probe_node(node.ip).await;
        Fingerprinter::infer_role(node);
        
        if let Some(ref mut c) = client {
            let _ = engine.check_existing(node, c).await;
        }
    }

    // 4. Interactive Selection (TUI)
    let options: Vec<String> = nodes.iter().map(|n| {
        let status = if n.is_existing { "[EXISTING]" } else { "[NEW]" };
        format!("{} {} ({}) - {}", 
            status, 
            n.ip, 
            n.hostname.as_deref().unwrap_or("unknown"),
            n.role.as_deref().unwrap_or("unknown")
        )
    }).collect();

    let selected_options = MultiSelect::new("Select nodes to provision into Pharos:", options)
        .prompt()?;

    if selected_options.is_empty() {
        info!("No nodes selected. Exiting.");
        return Ok(());
    }

    // 5. Provisioning Workflow
    if let Some(mut c) = client {
        for selection in selected_options {
            // Find the original node based on the selected string
            if let Some(node) = nodes.iter().find(|n| selection.contains(&n.ip.to_string())) {
                if node.is_existing {
                    info!("Skipping {} as it already exists.", node.ip);
                    continue;
                }

                println!("\n--- Provisioning Node: {} ---", node.ip);
                let alias = Text::new("Alias:")
                    .with_default(&node.hostname.as_deref().unwrap_or("").replace(".local.", ""))
                    .prompt()?;
                let owner = Text::new("Owner:")
                    .with_default("admin")
                    .prompt()?;
                
                let mut add_cmd = format!("add ip={} hostname=\"{}\" alias=\"{}\" owner=\"{}\" type=machine", 
                    node.ip, 
                    node.hostname.as_deref().unwrap_or(""),
                    alias,
                    owner
                );
                
                if let Some(ref role) = node.role {
                    add_cmd.push_str(&format!(" notes=\"{}\"", role));
                }

                match c.execute_authenticated(&add_cmd).await {
                    Ok(resp) => {
                        match resp {
                            PharosResponse::Ok(msg) => info!("Successfully added {}: {}", node.ip, msg),
                            PharosResponse::Error { code, message } => warn!("Failed to add {}: {} ({})", node.ip, message, code),
                            _ => warn!("Unexpected response from server for {}.", node.ip),
                        }
                    }
                    Err(e) => warn!("Error provisioning {}: {}", node.ip, e),
                }
            }
        }
        let _ = c.quit().await;
    } else {
        error!("Cannot provision: No connection to Pharos server.");
    }

    Ok(())
}
