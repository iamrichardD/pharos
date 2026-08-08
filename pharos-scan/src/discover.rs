/* ========================================================================
 * Project: pharos
 * Component: Network Scanner (pharos-scan)
 * File: pharos-scan/src/discover.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module converts raw ARP cache maps into DiscoveredNode structs by performing
 * OUI manufacturer resolution and concurrent reverse-DNS hostname lookups.
 * * Traceability:
 * Related to Unattended Discovery Mode (Step 2 of 3)
 * ======================================================================== */

use std::collections::HashMap;
use std::net::IpAddr;
use futures::stream::{self, StreamExt};
use crate::engine::lookup_hostname;
use crate::oui::OUIResolver;
use crate::DiscoveredNode;

/// Converts a raw ARP-cache map (`IpAddr` -> `MAC string`) into `DiscoveredNode`s by performing OUI
/// manufacturer resolution and reverse-DNS hostname lookups.
///
/// Processing is performed concurrently using a 64-way `buffer_unordered` stream. Concurrency is
/// critical here because this function powers the unattended discovery mode (running on a systemd
/// timer every 10 minutes), where up to 254 reverse-DNS lookups against slow or unresponsive DNS
/// resolvers could otherwise consume the entire execution time budget if performed sequentially.
pub async fn build_discovered_nodes(
    arp_cache: HashMap<IpAddr, String>,
) -> Vec<DiscoveredNode> {
    let oui = OUIResolver::default();

    stream::iter(arp_cache)
        .map(|(ip, mac)| {
            let manufacturer = oui.resolve(&mac);
            async move {
                let hostname = lookup_hostname(ip).await;
                DiscoveredNode {
                    ip,
                    hostname,
                    mac: Some(mac),
                    manufacturer,
                    ports: Vec::new(),
                    role: None,
                    is_existing: false,
                }
            }
        })
        .buffer_unordered(64)
        .collect()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_should_resolve_manufacturer_for_known_oui_prefix() {
        let mut arp_cache = HashMap::new();
        let ip: IpAddr = "192.168.1.10".parse().unwrap();
        arp_cache.insert(ip, "00:50:56:AB:CD:EF".to_string());

        let nodes = build_discovered_nodes(arp_cache).await;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].manufacturer, Some("VMware, Inc.".to_string()));
    }

    #[tokio::test]
    async fn test_should_return_none_manufacturer_for_unregistered_prefix() {
        let mut arp_cache = HashMap::new();
        let ip: IpAddr = "192.168.1.11".parse().unwrap();
        arp_cache.insert(ip, "AA:AA:AA:AA:AA:AA".to_string());

        let nodes = build_discovered_nodes(arp_cache).await;
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].manufacturer.is_none());
    }

    #[tokio::test]
    async fn test_should_preserve_ip_and_mac_from_arp_entry() {
        let mut arp_cache = HashMap::new();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        let mac = "12:34:56:78:90:AB".to_string();
        arp_cache.insert(ip, mac.clone());

        let nodes = build_discovered_nodes(arp_cache).await;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].ip, ip);
        assert_eq!(nodes[0].mac, Some(mac));
    }

    #[tokio::test]
    async fn test_should_default_ports_role_and_is_existing_correctly() {
        let mut arp_cache = HashMap::new();
        let ip: IpAddr = "10.0.0.2".parse().unwrap();
        arp_cache.insert(ip, "00:11:22:33:44:55".to_string());

        let nodes = build_discovered_nodes(arp_cache).await;
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].ports.is_empty());
        assert!(nodes[0].role.is_none());
        assert_eq!(nodes[0].is_existing, false);
    }

    #[tokio::test]
    async fn test_should_process_multiple_arp_entries_into_separate_nodes() {
        let mut arp_cache = HashMap::new();
        let ip1: IpAddr = "192.168.1.1".parse().unwrap();
        let ip2: IpAddr = "192.168.1.2".parse().unwrap();
        let ip3: IpAddr = "192.168.1.3".parse().unwrap();

        arp_cache.insert(ip1, "00:50:56:00:00:01".to_string());
        arp_cache.insert(ip2, "00:50:56:00:00:02".to_string());
        arp_cache.insert(ip3, "00:50:56:00:00:03".to_string());

        let nodes = build_discovered_nodes(arp_cache).await;
        assert_eq!(nodes.len(), 3);

        let node1 = nodes.iter().find(|n| n.ip == ip1).expect("ip1 node missing");
        assert_eq!(node1.mac, Some("00:50:56:00:00:01".to_string()));

        let node2 = nodes.iter().find(|n| n.ip == ip2).expect("ip2 node missing");
        assert_eq!(node2.mac, Some("00:50:56:00:00:02".to_string()));

        let node3 = nodes.iter().find(|n| n.ip == ip3).expect("ip3 node missing");
        assert_eq!(node3.mac, Some("00:50:56:00:00:03".to_string()));
    }

    #[tokio::test]
    async fn test_should_return_empty_vec_for_empty_arp_cache() {
        let arp_cache = HashMap::new();
        let nodes = build_discovered_nodes(arp_cache).await;
        assert!(nodes.is_empty());
    }

    #[tokio::test]
    async fn test_should_complete_within_reasonable_time_for_many_entries() {
        let mut arp_cache = HashMap::new();
        for i in 1..=50 {
            let ip: IpAddr = format!("192.0.2.{}", i).parse().unwrap();
            let mac = format!("00:50:56:00:00:{:02X}", i);
            arp_cache.insert(ip, mac);
        }

        let res = timeout(Duration::from_secs(15), build_discovered_nodes(arp_cache)).await;
        assert!(res.is_ok(), "build_discovered_nodes timed out");
        let nodes = res.unwrap();
        assert_eq!(nodes.len(), 50);
    }
}
