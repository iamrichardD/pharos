/* ========================================================================
 * Project: pharos
 * Component: Network Scanner (pharos-scan)
 * File: pharos-scan/src/sweep.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Implements an unprivileged ICMP ping sweep using the system `ping` binary
 * to freshen the kernel ARP/neighbor cache prior to reading /proc/net/arp.
 * * Traceability:
 * Related to unattended pharos-scan --auto device discovery mode.
 * ======================================================================== */

use std::net::IpAddr;
use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use ipnet::IpNet;
use tracing::debug;

/// Perform an unprivileged ICMP ping sweep across every host address in `cidr` (e.g. "192.168.1.0/24")
/// by shelling out to the system `ping` binary (`ping -c 1 -W 1 <ip>`).
///
/// This is a "fire and forget" operation: individual ping results (success, timeout, or failure
/// to spawn) are ignored. The primary purpose of this sweep is to trigger outbound ICMP echo
/// requests that force the host OS kernel to populate and freshen its ARP/neighbor cache for
/// active devices on the subnet. Reachability and device discovery are determined subsequently
/// by reading the OS ARP table, not from ping response status.
pub async fn ping_sweep(cidr: &str) -> Result<usize> {
    let net: IpNet = cidr
        .parse()
        .with_context(|| format!("Invalid subnet '{}' (expected CIDR notation, e.g. 192.168.1.0/24)", cidr))?;

    let host_ips: Vec<IpAddr> = net.hosts().collect();
    let total_hosts = host_ips.len();

    stream::iter(host_ips)
        .map(|ip| async move {
            let mut cmd = tokio::process::Command::new("ping");
            cmd.arg("-c").arg("1").arg("-W").arg("1").arg(ip.to_string());
            // Results are ignored (see the function doc comment) - suppress ping's own
            // stdout/stderr so an unattended sweep of a /24 doesn't spam hundreds of lines
            // into the systemd journal every 10 minutes.
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());
            if let Err(e) = cmd.status().await {
                debug!("Failed to execute ping command for {}: {}", ip, e);
            }
        })
        .buffer_unordered(64)
        .collect::<Vec<()>>()
        .await;

    Ok(total_hosts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_should_return_error_for_invalid_cidr() {
        let result = ping_sweep("not-a-valid-cidr").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_should_return_correct_host_count_for_small_subnet_without_panicking() {
        let result = ping_sweep("127.0.0.0/30").await;
        assert_eq!(result.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_should_complete_within_reasonable_time_for_small_subnet() {
        let res = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            ping_sweep("127.0.0.0/30"),
        )
        .await;
        assert!(res.is_ok(), "ping_sweep timed out after 10 seconds");
        assert_eq!(res.unwrap().unwrap(), 2);
    }
}
