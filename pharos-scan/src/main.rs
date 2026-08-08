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
use pharos_scan::sync::SyncOutcome;
use pharos_client::{PharosClient, PharosResponse};
use std::env;
use inquire::{MultiSelect, Text};
use clap::Parser;

#[derive(clap::Parser)]
#[command(name = "pharos-scan")]
#[command(about = "Pharos Network Discovery Scanner", long_about = None)]
struct Cli {
    /// Optional CIDR subnet to scan directly (e.g. 192.168.1.0/24) instead of mDNS discovery
    subnet: Option<String>,

    /// Skip the interactive TUI and print discovered nodes (with enrichment data) as a JSON
    /// array to stdout - for scripting. Pipe to `jq` for filtering.
    #[arg(long)]
    json: bool,

    /// Run one unattended discovery cycle (ping sweep + ARP read + OUI/hostname resolve + sync
    /// to the Pharos server) and exit, instead of the interactive/JSON discovery flow. Intended
    /// to be invoked on a schedule (e.g. a systemd timer), not interactively.
    #[arg(long)]
    auto: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let subscriber_builder = FmtSubscriber::builder()
        .with_max_level(Level::INFO);
    
    if cli.json {
        let subscriber = subscriber_builder.with_writer(std::io::stderr).finish();
        tracing::subscriber::set_global_default(subscriber)?;
    } else {
        let subscriber = subscriber_builder.finish();
        tracing::subscriber::set_global_default(subscriber)?;
    }

    if cli.auto {
        return run_auto_mode().await;
    }

    info!("Starting pharos-scan...");

    let engine = ScannerEngine::default();

    // 1. Discover nodes
    let mut nodes = if let Some(subnet) = cli.subnet.as_deref() {
        info!("Scanning subnet {}...", subnet);
        let found = engine.scan_subnet(subnet).await?;
        if found.is_empty() {
            if cli.json {
                println!("[]");
            } else {
                warn!("No live hosts found in subnet {}.", subnet);
            }
            return Ok(());
        }
        info!("Found {} live host(s) in {}", found.len(), subnet);
        found
    } else {
        let found = engine.discover_mdns().await?;
        if found.is_empty() {
            if cli.json {
                println!("[]");
            } else {
                warn!("No nodes discovered via mDNS.");
            }
            return Ok(());
        }
        info!("Found {} nodes via mDNS", found.len());
        found
    };

    // 2. Connect to Pharos to check for existing records
    let host = env::var("PHAROS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PHAROS_PORT").unwrap_or_else(|_| "2378".to_string());
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

    if cli.json {
        let json_output = serde_json::to_string_pretty(&nodes)?;
        println!("{}", json_output);
        if let Some(c) = client {
            let _ = c.quit().await;
        }
        return Ok(());
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
                
                if let Some(ref mac) = node.mac {
                    add_cmd.push_str(&format!(" mac=\"{}\"", mac));
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

async fn run_auto_mode() -> Result<()> {
    info!("Starting pharos-scan --auto discovery cycle...");

    let subnets = pharos_scan::subnet::local_ipv4_subnets();
    if subnets.is_empty() {
        warn!("No local IPv4 subnets detected to sweep - nothing to do this cycle.");
        return Ok(());
    }

    for subnet in &subnets {
        if let Err(e) = pharos_scan::sweep::ping_sweep(subnet).await {
            warn!("Ping sweep failed for subnet {}: {}", subnet, e);
        }
    }

    let arp_cache = pharos_scan::engine::read_arp_cache();
    let nodes = pharos_scan::discover::build_discovered_nodes(arp_cache).await;

    if nodes.is_empty() {
        info!(
            "Scan cycle complete: swept {} subnet(s), 0 devices found in ARP cache.",
            subnets.len()
        );
        return Ok(());
    }

    let host = env::var("PHAROS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PHAROS_PORT").unwrap_or_else(|_| "2378".to_string());
    let addr = format!("{}:{}", host, port);

    let mut client = match PharosClient::connect(&addr, "pharos-scan").await {
        Ok(c) => c,
        Err(e) => {
            // `e` already carries "Failed to connect to Pharos server at {addr}" as context
            // (added by PharosClient::connect itself) - log it directly rather than wrapping
            // the same message a second time.
            error!("{:#}", e);
            return Err(e);
        }
    };

    let mut outcomes = Vec::with_capacity(nodes.len());
    for node in &nodes {
        let outcome = pharos_scan::sync::sync_discovered_device(&mut client, node).await;
        if let SyncOutcome::Failed(ref msg) = outcome {
            warn!("Failed to sync device {}: {}", node.ip, msg);
        }
        outcomes.push(outcome);
    }

    let _ = client.quit().await;

    let (created, updated, skipped, failed) = tally_outcomes(&outcomes);
    info!(
        "Scan cycle complete: swept {} subnet(s), {} device(s) found - {} created, {} updated, {} skipped, {} failed.",
        subnets.len(),
        nodes.len(),
        created,
        updated,
        skipped,
        failed
    );

    Ok(())
}

fn tally_outcomes(outcomes: &[SyncOutcome]) -> (usize, usize, usize, usize) {
    let mut created = 0;
    let mut updated = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for outcome in outcomes {
        match outcome {
            SyncOutcome::Created => created += 1,
            SyncOutcome::Updated => updated += 1,
            SyncOutcome::Skipped => skipped += 1,
            SyncOutcome::Failed(_) => failed += 1,
        }
    }

    (created, updated, skipped, failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_parsing() {
        // 1. Cli::parse_from(["pharos-scan", "--json"]) -> json is true, subnet is None.
        let cli = Cli::parse_from(["pharos-scan", "--json"]);
        assert!(cli.json);
        assert_eq!(cli.subnet, None);
        assert!(!cli.auto);

        // 2. Cli::parse_from(["pharos-scan", "10.0.0.0/24", "--json"]) -> json is true, subnet is Some("10.0.0.0/24").
        let cli = Cli::parse_from(["pharos-scan", "10.0.0.0/24", "--json"]);
        assert!(cli.json);
        assert_eq!(cli.subnet.as_deref(), Some("10.0.0.0/24"));
        assert!(!cli.auto);

        // 3. Cli::parse_from(["pharos-scan", "--json", "10.0.0.0/24"]) -> same result as #2.
        let cli = Cli::parse_from(["pharos-scan", "--json", "10.0.0.0/24"]);
        assert!(cli.json);
        assert_eq!(cli.subnet.as_deref(), Some("10.0.0.0/24"));
        assert!(!cli.auto);

        // 4. Cli::parse_from(["pharos-scan"]) -> json is false, subnet is None.
        let cli = Cli::parse_from(["pharos-scan"]);
        assert!(!cli.json);
        assert_eq!(cli.subnet, None);
        assert!(!cli.auto);

        // 5. Cli::parse_from(["pharos-scan", "10.0.0.0/24"]) -> json is false, subnet is Some("10.0.0.0/24").
        let cli = Cli::parse_from(["pharos-scan", "10.0.0.0/24"]);
        assert!(!cli.json);
        assert_eq!(cli.subnet.as_deref(), Some("10.0.0.0/24"));
        assert!(!cli.auto);

        // 6. Cli::parse_from(["pharos-scan", "--auto"]) -> auto is true, json is false, subnet is None.
        let cli = Cli::parse_from(["pharos-scan", "--auto"]);
        assert!(cli.auto);
        assert!(!cli.json);
        assert_eq!(cli.subnet, None);
    }

    #[test]
    fn test_should_tally_outcomes_correctly() {
        let outcomes = vec![
            SyncOutcome::Created,
            SyncOutcome::Created,
            SyncOutcome::Updated,
            SyncOutcome::Updated,
            SyncOutcome::Updated,
            SyncOutcome::Skipped,
            SyncOutcome::Failed("connection reset".to_string()),
            SyncOutcome::Failed("timeout".to_string()),
        ];
        assert_eq!(tally_outcomes(&outcomes), (2, 3, 1, 2));
    }

    #[test]
    fn test_should_tally_empty_slice_as_all_zeros() {
        assert_eq!(tally_outcomes(&[]), (0, 0, 0, 0));
    }
}

