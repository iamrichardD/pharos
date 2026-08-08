/* ========================================================================
 * Project: pharos
 * Component: Network Scanner (pharos-scan)
 * File: pharos-scan/src/oui.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module resolves MAC address prefixes to manufacturers using real IEEE
 * MA-L, MA-M, and MA-S registries.
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
        prefixes.extend(parse_oui_csv(include_str!("../data/oui-ma-l.csv")));
        prefixes.extend(parse_oui_csv(include_str!("../data/oui-ma-m.csv")));
        prefixes.extend(parse_oui_csv(include_str!("../data/oui-ma-s.csv")));
        OUIResolver { prefixes }
    }
}

impl OUIResolver {
    pub fn resolve(&self, mac: &str) -> Option<String> {
        let normalized: String = mac
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .collect::<String>()
            .to_uppercase();
        for &len in &[9usize, 7, 6] {
            if normalized.len() >= len {
                if let Some(org) = self.prefixes.get(&normalized[..len]) {
                    return Some(org.clone());
                }
            }
        }
        None
    }
}

/// Parses raw IEEE OUI CSV data into a MAC-prefix-to-organization-name mapping.
///
/// IEEE MAC registries (MA-L, MA-M, MA-S) are formatted as CSV with headers:
/// `Registry,Assignment,Organization Name,Organization Address`. This helper extracts
/// Column 1 (Assignment / MAC prefix) and Column 2 (Organization Name), defensively
/// upper-casing prefixes and ignoring empty or unparseable records to ensure reliable resolution.
fn parse_oui_csv(csv_content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(csv_content.as_bytes());

    for result in reader.records() {
        let record = match result {
            Ok(r) => r,
            Err(_) => continue,
        };

        let prefix = match record.get(1) {
            Some(p) => p.trim(),
            None => continue,
        };

        let org_name = match record.get(2) {
            Some(o) => o.trim(),
            None => continue,
        };

        if prefix.is_empty() || org_name.is_empty() {
            continue;
        }

        map.insert(prefix.to_uppercase(), org_name.to_string());
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_parse_single_unquoted_organization_name() {
        let csv = "Registry,Assignment,Organization Name,Organization Address\n\
                   MA-L,38E2CA,Katun Corporation,7760 France Ave S Suite 340";
        let map = parse_oui_csv(csv);
        assert_eq!(map.get("38E2CA"), Some(&"Katun Corporation".to_string()));
    }

    #[test]
    fn test_should_parse_organization_name_with_quoted_embedded_comma() {
        let csv = "Registry,Assignment,Organization Name,Organization Address\n\
                   MA-L,286FB9,\"Nokia Shanghai Bell Co., Ltd.\",\"No.388 Ning Qiao Road\"";
        let map = parse_oui_csv(csv);
        assert_eq!(
            map.get("286FB9"),
            Some(&"Nokia Shanghai Bell Co., Ltd.".to_string())
        );
    }

    #[test]
    fn test_should_skip_header_row() {
        let csv = "Registry,Assignment,Organization Name,Organization Address\n\
                   MA-L,38E2CA,Katun Corporation,7760 France Ave S";
        let map = parse_oui_csv(csv);
        assert_eq!(map.len(), 1);
        assert!(!map.contains_key("Assignment"));
    }

    #[test]
    fn test_should_parse_multiple_data_rows_into_separate_keys() {
        let csv = "Registry,Assignment,Organization Name,Organization Address\n\
                   MA-L,286FB9,\"Nokia Shanghai Bell Co., Ltd.\",Address 1\n\
                   MA-L,38E2CA,Katun Corporation,Address 2\n\
                   MA-M,741AE09,Private,Address 3";
        let map = parse_oui_csv(csv);
        assert_eq!(map.len(), 3);
        assert_eq!(
            map.get("286FB9"),
            Some(&"Nokia Shanghai Bell Co., Ltd.".to_string())
        );
        assert_eq!(map.get("38E2CA"), Some(&"Katun Corporation".to_string()));
        assert_eq!(map.get("741AE09"), Some(&"Private".to_string()));
    }

    #[test]
    fn test_should_skip_row_with_empty_organization_name() {
        let csv = "Registry,Assignment,Organization Name,Organization Address\n\
                   MA-M,741AE09,,Address 3";
        let map = parse_oui_csv(csv);
        assert!(!map.contains_key("741AE09"));
        assert!(map.is_empty());
    }

    #[test]
    fn test_should_uppercase_the_prefix_key() {
        let csv = "Registry,Assignment,Organization Name,Organization Address\n\
                   ma-l,abc123,Some Vendor,Address";
        let map = parse_oui_csv(csv);
        assert_eq!(map.get("ABC123"), Some(&"Some Vendor".to_string()));
        assert!(!map.contains_key("abc123"));
    }

    #[test]
    fn test_should_return_empty_map_for_header_only_csv() {
        let csv = "Registry,Assignment,Organization Name,Organization Address";
        let map = parse_oui_csv(csv);
        assert!(map.is_empty());
    }

    #[test]
    fn test_should_resolve_real_ma_l_prefix_vmware() {
        let resolver = OUIResolver::default();
        assert_eq!(
            resolver.resolve("00:50:56:AB:CD:EF"),
            Some("VMware, Inc.".to_string())
        );
    }

    #[test]
    fn test_should_resolve_real_ma_l_prefix_with_corrected_proxmox_name() {
        let resolver = OUIResolver::default();
        assert_eq!(
            resolver.resolve("BC:24:11:00:01:07"),
            Some("Proxmox Server Solutions GmbH".to_string())
        );
    }

    #[test]
    fn test_should_resolve_real_ma_m_prefix() {
        let resolver = OUIResolver::default();
        assert_eq!(
            resolver.resolve("C8:5C:E2:70:00:01"),
            Some("SYNERGY SYSTEMS AND SOLUTIONS".to_string())
        );
    }

    #[test]
    fn test_should_resolve_real_ma_s_prefix() {
        let resolver = OUIResolver::default();
        assert_eq!(
            resolver.resolve("8C:1F:64:AF:A0:01"),
            Some("DATA ELECTRONIC DEVICES, INC".to_string())
        );
    }

    #[test]
    fn test_should_resolve_case_insensitively() {
        let resolver = OUIResolver::default();
        assert_eq!(
            resolver.resolve("00:50:56:ab:cd:ef"),
            Some("VMware, Inc.".to_string())
        );
    }

    #[test]
    fn test_should_return_none_for_unregistered_prefix() {
        let resolver = OUIResolver::default();
        assert_eq!(resolver.resolve("AA:AA:AA:AA:AA:AA"), None);
    }

    #[test]
    fn test_should_return_none_for_too_short_mac_string() {
        let resolver = OUIResolver::default();
        assert_eq!(resolver.resolve("AB:CD"), None);
        assert_eq!(resolver.resolve(""), None);
    }

    #[test]
    fn test_should_have_loaded_a_large_number_of_prefixes() {
        let resolver = OUIResolver::default();
        assert!(resolver.prefixes.len() >= 40000);
    }
}


