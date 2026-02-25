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

pub mod engine;
pub mod fingerprint;
pub mod oui;

use std::net::IpAddr;

/// Represents a discovered network node.
#[derive(Debug, Clone, PartialEq, Eq)]
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
