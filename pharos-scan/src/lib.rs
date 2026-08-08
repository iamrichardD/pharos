/* ========================================================================
 * Project: pharos
 * Component: Network Scanner (pharos-scan)
 * File: pharos-scan/src/lib.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This library provides the core discovery and fingerprinting logic for
 * pharos-scan. It integrates mDNS/DNS-SD, port probes, and OUI resolution
 * to automatically identify infrastructure assets.
 * * Traceability:
 * Related to Task 10.2 (Issue #40)
 * ======================================================================== */

pub mod discover;
pub mod engine;
pub mod fingerprint;
pub mod oui;
pub mod subnet;
pub mod sweep;
pub mod sync;

use std::net::IpAddr;

/// Represents a discovered network node.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DiscoveredNode {
    pub ip: IpAddr,
    pub hostname: Option<String>,
    pub mac: Option<String>,
    pub manufacturer: Option<String>,
    pub ports: Vec<u16>,
    pub role: Option<String>,
    pub is_existing: bool,
}

/// Represents the possible roles inferred by the fingerprinting logic.
pub enum NodeRole {
    Server,
    Workstation,
    NetworkDevice,
    IOT,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_discovered_node_serialization() {
        let node = DiscoveredNode {
            ip: "192.168.1.100".parse().unwrap(),
            hostname: Some("test-host".to_string()),
            mac: Some("00:11:22:33:44:55".to_string()),
            manufacturer: Some("TestVendor".to_string()),
            ports: vec![22, 80],
            role: Some("SSH Server".to_string()),
            is_existing: false,
        };

        let serialized = serde_json::to_value(&node).unwrap();
        assert_eq!(
            serialized,
            json!({
                "ip": "192.168.1.100",
                "hostname": "test-host",
                "mac": "00:11:22:33:44:55",
                "manufacturer": "TestVendor",
                "ports": [22, 80],
                "role": "SSH Server",
                "is_existing": false
            })
        );
    }
}
