/* ========================================================================
 * Project: pharos
 * Component: Network Scanner (pharos-scan)
 * File: pharos-scan/src/fingerprint.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module provides logic to guess the device role or manufacturer
 * based on open ports and other discovered metadata.
 * * Traceability:
 * Related to Task 10.2 (Issue #40)
 * ======================================================================== */

use crate::DiscoveredNode;

pub struct Fingerprinter;

impl Fingerprinter {
    /// Guesses a node's role based on its open ports and hostname.
    pub fn infer_role(node: &mut DiscoveredNode) {
        if node.ports.contains(&8006) {
            node.role = Some("Proxmox Virtualization Server".to_string());
        } else if node.ports.contains(&32400) {
            node.role = Some("Plex Media Server".to_string());
        } else if node.ports.contains(&80) || node.ports.contains(&443) {
            node.role = Some("Web Server".to_string());
        } else if node.ports.contains(&22) {
            node.role = Some("SSH-enabled Linux/UNIX Server".to_string());
        } else {
            node.role = Some("Unknown Infrastructure Asset".to_string());
        }
    }
}
