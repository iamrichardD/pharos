/* ========================================================================
 * Project: pharos
 * Component: Network Scanner (pharos-scan)
 * File: pharos-scan/src/engine.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module implements the scanning engines for mDNS discovery and
 * port fingerprinting, providing the core discovery functionality.
 * * Traceability:
 * Related to Task 10.2 (Issue #40)
 * ======================================================================== */

use std::net::IpAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use anyhow::{Result, Context};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use tracing::{info, debug, warn};
use crate::DiscoveredNode;
use pharos_client::{PharosClient, PharosResponse};

pub struct ScannerEngine {
    timeout: Duration,
    common_ports: Vec<u16>,
}

impl Default for ScannerEngine {
    fn default() -> Self {
        ScannerEngine {
            timeout: Duration::from_millis(500),
            common_ports: vec![22, 80, 443, 8006, 32400],
        }
    }
}

impl ScannerEngine {
    /// Checks if a node already exists in the Pharos server.
    pub async fn check_existing(&self, node: &mut DiscoveredNode, client: &mut PharosClient) -> Result<()> {
        let query = format!("ip={}", node.ip);
        let resp = client.execute(&query).await?;
        if let PharosResponse::Matches { count, .. } = resp {
            node.is_existing = count > 0;
        }
        Ok(())
    }

    /// Perform mDNS discovery on the local network.
    pub async fn discover_mdns(&self) -> Result<Vec<DiscoveredNode>> {
        let mdns = ServiceDaemon::new().context("Failed to start mDNS daemon")?;
        let service_type = "_ssh._tcp.local.";
        let receiver = mdns.browse(service_type).context("Failed to browse mDNS")?;

        info!("Browsing for mDNS services (_ssh._tcp.local.)...");

        let mut nodes = std::collections::HashMap::new();
        let scan_duration = Duration::from_secs(5);
        let start = std::time::Instant::now();

        while start.elapsed() < scan_duration {
            if let Ok(event) = receiver.recv_timeout(Duration::from_millis(100)) {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        for ip in info.get_addresses() {
                            let node = DiscoveredNode {
                                ip: *ip,
                                hostname: Some(info.get_fullname().to_string()),
                                mac: None,
                                manufacturer: None,
                                ports: Vec::new(),
                                role: None,
                                is_existing: false,
                            };
                            nodes.insert(*ip, node);
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(nodes.into_values().collect())
    }

    /// Perform a fast port scan on a specific IP.
    pub async fn probe_node(&self, ip: IpAddr) -> Vec<u16> {
        let mut open_ports = Vec::new();
        for port in &self.common_ports {
            let addr = format!("{}:{}", ip, port);
            if timeout(self.timeout, TcpStream::connect(&addr)).await.is_ok() {
                debug!("Port {} is open on {}", port, ip);
                open_ports.push(*port);
            }
        }
        open_ports
    }

    /// Perform a full scan of a subnet (ARP or similar).
    /// Note: Subnet scanning in containers can be restricted.
    pub async fn scan_subnet(&self, _subnet: &str) -> Result<Vec<DiscoveredNode>> {
        // Placeholder for now as ARP scanning requires elevated privileges.
        warn!("Subnet scanning not yet fully implemented (requires raw sockets).");
        Ok(Vec::new())
    }
}
