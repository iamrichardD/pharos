/* ========================================================================
 * Project: pharos
 * Component: Network Scanner (pharos-scan)
 * File: pharos-scan/src/oui.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module resolves MAC address prefixes to manufacturers.
 * * Traceability:
 * Related to Task 10.2 (Issue #40)
 * ======================================================================== */

use std::collections::HashMap;

pub struct OUIResolver {
    prefixes: HashMap<String, String>,
}

impl Default for OUIResolver {
    fn default() -> Self {
        let mut prefixes = HashMap::new();
        // Common virtualization/server prefixes
        prefixes.insert("00:50:56".to_string(), "VMware, Inc.".to_string());
        prefixes.insert("08:00:27".to_string(), "Oracle (VirtualBox)".to_string());
        prefixes.insert("BC:24:11".to_string(), "Proxmox Server (Hypothetical)".to_string());
        prefixes.insert("B8:27:EB".to_string(), "Raspberry Pi Foundation".to_string());
        prefixes.insert("DC:A6:32".to_string(), "Raspberry Pi Foundation (4)".to_string());
        prefixes.insert("00:15:5D".to_string(), "Microsoft (Hyper-V)".to_string());
        
        OUIResolver { prefixes }
    }
}

impl OUIResolver {
    pub fn resolve(&self, mac: &str) -> Option<String> {
        if mac.len() < 8 {
            return None;
        }
        let prefix = mac[..8].to_uppercase();
        self.prefixes.get(&prefix).cloned()
    }
}
