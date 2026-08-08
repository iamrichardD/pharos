/* ========================================================================
 * Project: pharos
 * Component: Network Scanner (pharos-scan)
 * File: pharos-scan/src/subnet.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module enumerates the host system's network interfaces to discover
 * local IPv4 subnets for unattended device discovery sweeps without requiring
 * manual subnet configuration. Includes safety caps against overly wide netmasks.
 * * Traceability:
 * Related to Task 10.2 (Issue #40)
 * ======================================================================== */

/// Returns true if a netmask prefix length is strictly less than 20 (i.e. wider than `/20`).
pub fn is_subnet_too_wide(prefixlen: u8) -> bool {
    prefixlen < 20
}

/// Enumerates the host's non-loopback network interfaces to discover local IPv4 subnets for unattended scanning.
///
/// Returns a deduplicated list of CIDR strings (e.g. `["192.168.1.0/24"]`).
///
/// Subnets are filtered out under the following safety conditions:
/// - Loopback interfaces (`is_loopback()`) or loopback IPv4 addresses (`127.0.0.0/8`).
/// - Subnets wider than `/20` (`prefixlen < 20`). This deliberate safety cap prevents a misconfigured or unusual
///   host netmask (such as `/16` or `/8`) from causing an unattended sweep to attempt probing millions of addresses.
///   Skipped subnets log a `tracing::warn!` message containing the skipped CIDR and its prefix length.
///
/// If interface enumeration fails (e.g. OS or permission errors), an empty `Vec` is returned adhering to the
/// best-effort failure policy established in `read_arp_cache`.
pub fn local_ipv4_subnets() -> Vec<String> {
    let addrs = match if_addrs::get_if_addrs() {
        Ok(addrs) => addrs,
        Err(e) => {
            tracing::warn!("Failed to retrieve network interface addresses: {}", e);
            return Vec::new();
        }
    };

    let mut subnets = Vec::new();

    for iface in addrs {
        if iface.is_loopback() {
            continue;
        }

        if let if_addrs::IfAddr::V4(ref ifv4) = iface.addr {
            if ifv4.ip.is_loopback() {
                continue;
            }

            if let Ok(net) = ipnet::Ipv4Net::with_netmask(ifv4.ip, ifv4.netmask) {
                let net_trunc = net.trunc();
                let prefixlen = net_trunc.prefix_len();
                let cidr_str = net_trunc.to_string();

                if is_subnet_too_wide(prefixlen) {
                    tracing::warn!(
                        subnet = %cidr_str,
                        prefixlen = prefixlen,
                        "Skipping subnet wider than /20 during local subnet discovery"
                    );
                } else if !subnets.contains(&cidr_str) {
                    subnets.push(cidr_str);
                }
            }
        }
    }

    subnets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_return_a_vec_without_panicking() {
        let subnets = local_ipv4_subnets();
        // Smoke test to ensure function executes cleanly without panic.
        let _ = subnets.len();
    }

    #[test]
    fn test_should_not_include_loopback_subnet() {
        let subnets = local_ipv4_subnets();
        for cidr in subnets {
            assert!(
                !cidr.starts_with("127."),
                "Local IPv4 subnets must not contain loopback address range 127.0.0.0/8, got: {}",
                cidr
            );
        }
    }

    #[test]
    fn test_should_exclude_subnets_wider_than_slash_20() {
        assert!(
            !is_subnet_too_wide(24),
            "/24 subnet should NOT be considered too wide"
        );
        assert!(
            !is_subnet_too_wide(20),
            "/20 subnet should NOT be considered too wide"
        );
        assert!(
            is_subnet_too_wide(16),
            "/16 subnet SHOULD be considered too wide"
        );
        assert!(
            is_subnet_too_wide(8),
            "/8 subnet SHOULD be considered too wide"
        );
    }
}
