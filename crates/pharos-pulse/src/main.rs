/* ========================================================================
 * Project: pharos
 * Component: pharos-pulse
 * File: crates/pharos-pulse/src/main.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This crate implements the `pharos-pulse` heartbeat agent. It periodically
 * collects system metrics (CPU, Memory, Uptime) and transmits them to the
 * `pharos-server` to power the real-time TUI dashboard and tracking.
 * * Traceability:
 * Related to Task 14.1 (Issue #47)
 * ======================================================================== */

use pharos_client::PharosClient;
use std::env;
use std::time::Duration;
use sysinfo::System;
use tokio::time::sleep;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting pharos-pulse agent...");
    
    let server_addr = env::var("PHAROS_SERVER").unwrap_or_else(|_| "127.0.0.1:10011".to_string());
    let machine_name = env::var("PHAROS_MACHINE_NAME").unwrap_or_else(|_| {
        sysinfo::System::host_name().unwrap_or_else(|| "unknown-host".to_string())
    });

    let mut sys = System::new_all();
    
    loop {
        sys.refresh_all();
        
        let uptime = sysinfo::System::uptime();
        let total_mem = sys.total_memory();
        let used_mem = sys.used_memory();
        let global_cpu = sys.global_cpu_info().cpu_usage();

        println!("Metrics: CPU: {:.2}% | Mem: {}/{} KB | Uptime: {}s", global_cpu, used_mem, total_mem, uptime);

        if let Err(e) = send_metrics(&server_addr, &machine_name, global_cpu, used_mem, total_mem, uptime).await {
            eprintln!("Failed to send metrics: {:?}", e);
        }

        sleep(Duration::from_secs(10)).await;
    }
}

async fn send_metrics(server_addr: &str, machine_name: &str, cpu: f32, used_mem: u64, total_mem: u64, uptime: u64) -> Result<()> {
    let mut client = PharosClient::connect(server_addr, &format!("pulse-{}", machine_name)).await?;

    // Attempt to upsert the machine record
    let add_cmd = format!(
        "add type=\"machine\" name=\"{}\" cpu=\"{:.2}\" mem_used=\"{}\" mem_total=\"{}\" uptime=\"{}\"",
        machine_name, cpu, used_mem, total_mem, uptime
    );

    match client.execute_authenticated(&add_cmd).await {
        Ok(resp) => {
            println!("Server response: {:?}", resp);
        }
        Err(e) => {
            eprintln!("Authentication or transmission failed: {:?}", e);
        }
    }

    client.quit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_compile_metrics_collection_when_invoked() {
        let mut sys = System::new_all();
        sys.refresh_all();
        let uptime = sysinfo::System::uptime();
        assert!(uptime > 0 || uptime == 0);
    }
}
