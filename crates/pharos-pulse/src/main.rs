/* ========================================================================
 * Project: pharos
 * Component: pharos-pulse
 * File: crates/pharos-pulse/src/main.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This crate implements the `pharos-pulse` Automated Inventory & Presence
 * System. It ensures a node's presence and identity are known to the 
 * Pharos server by sending an initial baseline inventory and periodic
 * heartbeats.
 * * Traceability:
 * Related to Task 14.11 (Issue #100), implements "Inventory-First" strategy.
 * Implements Task 105 (Issue #105): Filter out "unknown" inventory fields.
 * ======================================================================== */

use pharos_client::PharosClient;
use std::env;
use std::time::Duration;
use sysinfo::System;
use tokio::time::{sleep, interval};
use anyhow::Result;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting pharos-pulse agent v{}...", env!("CARGO_PKG_VERSION"));

    let server_addr = env::var("PHAROS_SERVER").unwrap_or_else(|_| "127.0.0.1:2378".to_string());
    let machine_name = env::var("PHAROS_MACHINE_NAME").unwrap_or_else(|_| {
        sysinfo::System::host_name().unwrap_or_else(|| "unknown-host".to_string())
    });

    // We initialize signals early and use a unified shutdown future.
    let shutdown = async {
        #[cfg(unix)]
        {
            let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Failed to register SIGTERM handler");
            let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                .expect("Failed to register SIGINT handler");
            tokio::select! {
                _ = sigterm.recv() => println!("SIGTERM received"),
                _ = sigint.recv() => println!("SIGINT received"),
                _ = tokio::signal::ctrl_c() => println!("CTRL+C received"),
            }
        }
        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl-c");
            println!("CTRL+C received");
        }
    };
    tokio::pin!(shutdown);

    println!("Pharos Server Address: {}", server_addr);
    println!("Machine Name: {}", machine_name);

    tokio::select! {
        _ = wait_for_server(&server_addr) => {
            println!("Connectivity to server established.");
        },
        _ = &mut shutdown => {
            println!("Shutdown signal received during startup, exiting gracefully...");
            return Ok(());
        }
    }

    // 1. Baseline (ONLINE) — retry with backoff until it succeeds or shutdown is requested, so a
    // transient failure (e.g. TLS/cert not yet trusted right after boot) doesn't leave the node
    // unregistered for up to an hour waiting on the next heartbeat tick.
    println!("Collecting baseline inventory...");
    let inventory = collect_inventory();

    tokio::select! {
        _ = send_baseline_until_success(&server_addr, &machine_name, inventory) => {},
        _ = &mut shutdown => {
            println!("Shutdown signal received during startup, exiting gracefully...");
            return Ok(());
        }
    }

    // 2. Heartbeat & Shutdown handling
    let mut heartbeat_interval = interval(Duration::from_secs(3600));
    // First tick finishes immediately, we already sent baseline, so skip first tick
    heartbeat_interval.tick().await; 

    println!("Entering heartbeat loop (60 minute intervals)...");

    tokio::select! {
        _ = async {
            loop {
                heartbeat_interval.tick().await;
                println!("Sending periodic heartbeat...");
                if let Err(e) = send_presence(&server_addr, &machine_name, "online", None).await {
                    eprintln!("Failed to send heartbeat: {:?}", e);
                }
            }
        } => {},
        _ = &mut shutdown => {
            println!("Shutdown signal received, initiating graceful exit...");
        },
    }

    // 3. Graceful Exit (OFFLINE)
    println!("Initiating graceful shutdown (sending OFFLINE signal)...");
    
    // We wrap the offline signal in a short timeout (5s) to ensure we don't block
    // the container shutdown for too long if the server is already going down.
    let shutdown_timeout = Duration::from_secs(5);
    match tokio::time::timeout(shutdown_timeout, send_presence(&server_addr, &machine_name, "offline", None)).await {
        Ok(Ok(_)) => println!("Offline signal sent successfully."),
        Ok(Err(e)) => eprintln!("Failed to send offline signal: {:?}", e),
        Err(_) => eprintln!("Timed out sending offline signal after {:?}", shutdown_timeout),
    }

    println!("pharos-pulse agent shutdown complete.");
    Ok(())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InventoryPayload {
    pub fields: HashMap<String, String>,
    pub multi_fields: HashMap<String, Vec<String>>,
}

fn collect_inventory() -> InventoryPayload {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut fields = HashMap::new();
    let mut multi_fields: HashMap<String, Vec<String>> = HashMap::new();
    fields.insert("type".to_string(), "machine".to_string());
    
    if let Some(brand) = sys.cpus().first().map(|c| c.brand().to_string()) {
        fields.insert("cpu_brand".to_string(), brand);
    }
    
    fields.insert("cpu_cores".to_string(), sys.cpus().len().to_string());
    // sysinfo 0.30's total_memory() returns bytes, not KB, despite this field's name and every
    // downstream consumer (e.g. mdb's format_human()) treating it as genuinely in KB - confirmed
    // live in production (a reported value was found to be the real KB total times exactly 1024).
    fields.insert("mem_total_kb".to_string(), bytes_to_kb(sys.total_memory()).to_string());
    fields.insert("os_name".to_string(), System::name().unwrap_or_else(|| "unknown".to_string()));
    fields.insert("os_version".to_string(), System::os_version().unwrap_or_else(|| "unknown".to_string()));
    fields.insert("kernel_version".to_string(), System::kernel_version().unwrap_or_else(|| "unknown".to_string()));
    fields.insert("serial_number".to_string(), get_serial_number());

    // Enumerate real network interfaces for MAC and IP address auto-population.
    // Skip any interface that has no assigned IP address at all (e.g. bond slaves, unassigned bonds, veth pairs).
    if let Ok(if_addrs) = if_addrs::get_if_addrs() {
        let mut ip_map: HashMap<String, (Option<String>, Option<String>)> = HashMap::new();
        for iface in if_addrs {
            if iface.is_loopback() || iface.name == "lo" || iface.name.starts_with("lo") {
                continue;
            }
            let ip = iface.addr.ip();
            if ip.is_loopback() {
                continue;
            }
            let entry = ip_map.entry(iface.name.clone()).or_insert((None, None));
            match ip {
                std::net::IpAddr::V4(v4) => entry.0 = Some(v4.to_string()),
                std::net::IpAddr::V6(v6) => entry.1 = Some(v6.to_string()),
            }
        }

        let networks = sysinfo::Networks::new_with_refreshed_list();

        for (iface_name, (v4_opt, v6_opt)) in ip_map {
            if v4_opt.is_none() && v6_opt.is_none() {
                continue;
            }
            let ip_vec = multi_fields.entry("ip_addr".to_string()).or_default();
            if let Some(v4) = v4_opt {
                if !ip_vec.contains(&v4) {
                    ip_vec.push(v4);
                }
            }
            if let Some(v6) = v6_opt {
                if !ip_vec.contains(&v6) {
                    ip_vec.push(v6);
                }
            }

            if let Some(net_data) = networks.get(&iface_name) {
                let mac = net_data.mac_address();
                if mac != sysinfo::MacAddr::UNSPECIFIED {
                    let mac_str = mac.to_string();
                    if mac_str != "00:00:00:00:00:00" && !mac_str.is_empty() {
                        let mac_vec = multi_fields.entry("mac_addr".to_string()).or_default();
                        if !mac_vec.contains(&mac_str) {
                            mac_vec.push(mac_str);
                        }
                    }
                }
            }
        }
    }

    // Filter out fields with value "unknown" to minimize record size and noise
    fields.retain(|_, v| v != "unknown");

    InventoryPayload {
        fields,
        multi_fields,
    }
}

/// sysinfo 0.30's total_memory() returns bytes; every consumer of the mem_total_kb field
/// (including mdb's format_human()) expects genuine KB, matching the field's own name.
fn bytes_to_kb(bytes: u64) -> u64 {
    bytes / 1024
}

fn build_presence_command(machine_name: &str, status: &str, inventory: Option<InventoryPayload>) -> String {
    let mut cmd = format!("add hostname=\"{}\" status=\"{}\"", 
                          machine_name.replace("\"", "\\\""), 
                          status.replace("\"", "\\\""));
    
    if let Some(inv) = inventory {
        // Sort keys for deterministic testing
        let mut keys: Vec<&String> = inv.fields.keys().collect();
        keys.sort();
        for k in keys {
            let v = inv.fields.get(k).unwrap();
            cmd.push_str(&format!(" {}=\"{}\"", k, v.replace("\"", "\\\"")));
        }

        let mut multi_keys: Vec<&String> = inv.multi_fields.keys().collect();
        multi_keys.sort();
        for mk in multi_keys {
            let values = inv.multi_fields.get(mk).unwrap();
            for val in values {
                cmd.push_str(&format!(" {}=\"{}\"", mk, val.replace("\"", "\\\"")));
            }
        }
    }
    cmd
}

async fn send_presence(server_addr: &str, machine_name: &str, status: &str, inventory: Option<InventoryPayload>) -> Result<()> {
    let mut client = PharosClient::connect(server_addr, &format!("pulse-{}", machine_name)).await?;

    let cmd = build_presence_command(machine_name, status, inventory);
    
    client.execute_authenticated(&cmd).await?;
    client.quit().await?;
    Ok(())
}

async fn wait_for_server(server_addr: &str) {
    let mut delay = Duration::from_secs(1);
    loop {
        match tokio::net::TcpStream::connect(server_addr).await {
            Ok(_) => {
                println!("Connectivity verified to pharos-server at {}", server_addr);
                break;
            }
            Err(e) => {
                eprintln!("Waiting for pharos-server at {}: {} (Retrying in {:?})", server_addr, e, delay);
                sleep(delay).await;
                delay = std::cmp::min(delay * 2, Duration::from_secs(60));
            }
        }
    }
}

async fn send_baseline_until_success(
    server_addr: &str,
    machine_name: &str,
    inventory: InventoryPayload,
) {
    let mut delay = Duration::from_secs(1);
    loop {
        match send_presence(server_addr, machine_name, "online", Some(inventory.clone())).await {
            Ok(_) => {
                println!("Baseline inventory registered successfully with pharos-server at {}", server_addr);
                break;
            }
            Err(e) => {
                eprintln!("Failed to register baseline inventory with {}: {} (Retrying in {:?})", server_addr, e, delay);
                sleep(delay).await;
                delay = std::cmp::min(delay * 2, Duration::from_secs(60));
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn get_serial_number() -> String {
    std::fs::read_to_string("/sys/class/dmi/id/product_serial")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| {
            // Fallback for environments where the file is missing or inaccessible
            "unknown".to_string()
        })
}

#[cfg(target_os = "macos")]
fn get_serial_number() -> String {
    let output = std::process::Command::new("ioreg")
        .args(&["-rd1", "-c", "IOPlatformExpertDevice"])
        .output();
    if let Ok(out) = output {
        let s = String::from_utf8_lossy(&out.stdout);
        for line in s.lines() {
            if line.contains("IOPlatformSerialNumber") {
                return line.split('=').last().unwrap_or("unknown").trim().replace("\"", "");
            }
        }
    }
    "unknown".to_string()
}

#[cfg(target_os = "windows")]
fn get_serial_number() -> String {
    let output = std::process::Command::new("powershell")
        .args(&["-Command", "Get-CimInstance Win32_Bios | Select-Object -ExpandProperty SerialNumber"])
        .output();
    if let Ok(out) = output {
        return String::from_utf8_lossy(&out.stdout).trim().to_string();
    }
    "unknown".to_string()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn get_serial_number() -> String {
    "unsupported-os".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_convert_bytes_to_kb_using_real_world_reported_value() {
        // proxmox-01's actual reported (buggy) mem_total_kb value, confirmed live in production
        // to be exactly the real KB total (per `free -k`) times 1024 - i.e. genuinely bytes.
        assert_eq!(bytes_to_kb(13493395456), 13177144);
    }

    #[test]
    fn test_should_collect_inventory_fields_when_invoked() {
        let inv = collect_inventory();
        // Core fields should always be present
        assert!(inv.fields.contains_key("type"));
        assert!(inv.fields.contains_key("cpu_cores"));
        assert!(inv.fields.contains_key("mem_total_kb"));
        
        // Ensure no "unknown" values exist in the inventory
        for (k, v) in &inv.fields {
            assert_ne!(v, "unknown", "Field '{}' should not have value 'unknown'", k);
        }
    }

    #[test]
    fn test_inspect_sysinfo_networks() {
        let inv = collect_inventory();
        for (k, values) in &inv.multi_fields {
            for v in values {
                println!("Collected network multi_field: {} = {}", k, v);
            }
        }
    }

    #[test]
    fn test_should_format_presence_command_correctly_when_inventory_provided() {
        let mut fields = HashMap::new();
        fields.insert("type".to_string(), "machine".to_string());
        fields.insert("cpu_cores".to_string(), "8".to_string());
        let mut multi_fields = HashMap::new();
        multi_fields.insert("ip_addr".to_string(), vec!["192.168.1.100".to_string(), "fe80::1".to_string()]);
        
        let inv = InventoryPayload { fields, multi_fields };
        let cmd = build_presence_command("test-host", "online", Some(inv));
        assert!(cmd.contains("add hostname=\"test-host\" status=\"online\""));
        assert!(cmd.contains("type=\"machine\""));
        assert!(cmd.contains("cpu_cores=\"8\""));
        assert!(cmd.contains("ip_addr=\"192.168.1.100\""));
        assert!(cmd.contains("ip_addr=\"fe80::1\""));
    }

    #[test]
    fn test_should_increase_delay_exponentially_up_to_limit_when_calculating_backoff() {
        // Verify the exact backoff logic used in send_baseline_until_success & wait_for_server
        let mut delay = Duration::from_secs(1);
        let cap = Duration::from_secs(60);

        // Step 1: 1s * 2 = 2s
        delay = std::cmp::min(delay * 2, cap);
        assert_eq!(delay, Duration::from_secs(2));

        // Step 2: 2s * 2 = 4s
        delay = std::cmp::min(delay * 2, cap);
        assert_eq!(delay, Duration::from_secs(4));

        // Step 3: 4s * 2 = 8s
        delay = std::cmp::min(delay * 2, cap);
        assert_eq!(delay, Duration::from_secs(8));

        // Fast forward to near cap
        delay = Duration::from_secs(32);
        delay = std::cmp::min(delay * 2, cap);
        assert_eq!(delay, Duration::from_secs(60)); // Capped at 60s
    }
}
