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
    println!("Starting pharos-pulse agent...");

    #[cfg(unix)]
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    
    let server_addr = env::var("PHAROS_SERVER").unwrap_or_else(|_| "127.0.0.1:2378".to_string());
    let machine_name = env::var("PHAROS_MACHINE_NAME").unwrap_or_else(|_| {
        sysinfo::System::host_name().unwrap_or_else(|| "unknown-host".to_string())
    });

    tokio::select! {
        _ = wait_for_server(&server_addr) => {},
        _ = tokio::signal::ctrl_c() => {
            println!("SIGINT received during startup, shutting down...");
            return Ok(());
        },
        _ = async {
            #[cfg(unix)]
            {
                sigterm.recv().await;
            }
            #[cfg(not(unix))]
            {
                std::future::pending::<()>().await;
            }
        } => {
            println!("SIGTERM received during startup, shutting down...");
            return Ok(());
        }
    }

    // 1. Baseline (ONLINE)
    println!("Collecting baseline inventory...");
    let inventory = collect_inventory();
    if let Err(e) = send_presence(&server_addr, &machine_name, "online", Some(inventory)).await {
        eprintln!("Failed to send baseline presence: {:?}", e);
    } else {
        println!("Baseline inventory sent successfully (Status: online).");
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
        _ = tokio::signal::ctrl_c() => {
            println!("SIGINT received, shutting down...");
        },
        _ = async {
            #[cfg(unix)]
            {
                sigterm.recv().await;
                println!("SIGTERM received, shutting down...");
            }
            #[cfg(not(unix))]
            {
                // On non-unix, we just wait forever for other signals
                std::future::pending::<()>().await;
            }
        } => {},
    }

    // 3. Graceful Exit (OFFLINE)
    println!("Sending offline signal...");
    if let Err(e) = send_presence(&server_addr, &machine_name, "offline", None).await {
        eprintln!("Failed to send offline signal: {:?}", e);
    }

    println!("pharos-pulse agent shutdown complete.");
    Ok(())
}

fn collect_inventory() -> HashMap<String, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut inv = HashMap::new();
    inv.insert("type".to_string(), "machine".to_string());
    
    if let Some(brand) = sys.cpus().first().map(|c| c.brand().to_string()) {
        inv.insert("cpu_brand".to_string(), brand);
    }
    
    inv.insert("cpu_cores".to_string(), sys.cpus().len().to_string());
    inv.insert("mem_total_kb".to_string(), sys.total_memory().to_string());
    inv.insert("os_name".to_string(), System::name().unwrap_or_else(|| "unknown".to_string()));
    inv.insert("os_version".to_string(), System::os_version().unwrap_or_else(|| "unknown".to_string()));
    inv.insert("kernel_version".to_string(), System::kernel_version().unwrap_or_else(|| "unknown".to_string()));
    inv.insert("serial_number".to_string(), get_serial_number());

    // Filter out fields with value "unknown" to minimize record size and noise
    inv.retain(|_, v| v != "unknown");

    inv
}

fn build_presence_command(machine_name: &str, status: &str, inventory: Option<HashMap<String, String>>) -> String {
    let mut cmd = format!("add hostname=\"{}\" status=\"{}\"", 
                          machine_name.replace("\"", "\\\""), 
                          status.replace("\"", "\\\""));
    
    if let Some(inv) = inventory {
        // Sort keys for deterministic testing
        let mut keys: Vec<&String> = inv.keys().collect();
        keys.sort();
        for k in keys {
            let v = inv.get(k).unwrap();
            cmd.push_str(&format!(" {}=\"{}\"", k, v.replace("\"", "\\\"")));
        }
    }
    cmd
}

async fn send_presence(server_addr: &str, machine_name: &str, status: &str, inventory: Option<HashMap<String, String>>) -> Result<()> {
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
    fn test_should_collect_inventory_fields_when_invoked() {
        let inv = collect_inventory();
        // Core fields should always be present
        assert!(inv.contains_key("type"));
        assert!(inv.contains_key("cpu_cores"));
        assert!(inv.contains_key("mem_total_kb"));
        
        // Ensure no "unknown" values exist in the inventory
        for (k, v) in &inv {
            assert_ne!(v, "unknown", "Field '{}' should not have value 'unknown'", k);
        }
    }

    #[test]
    fn test_should_format_presence_command_correctly_when_inventory_provided() {
        let mut inv = HashMap::new();
        inv.insert("type".to_string(), "machine".to_string());
        inv.insert("cpu_cores".to_string(), "8".to_string());
        
        let cmd = build_presence_command("test-host", "online", Some(inv));
        assert!(cmd.contains("add hostname=\"test-host\" status=\"online\""));
        assert!(cmd.contains("type=\"machine\""));
        assert!(cmd.contains("cpu_cores=\"8\""));
    }
}
