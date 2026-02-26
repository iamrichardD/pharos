/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/src/main.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This is the binary entry point for the pharos backend server. It initializes
 * the environment, storage, and middleware before starting the TCP listener.
 * * Traceability:
 * Implements RFC 2378 Section 2.
 * ======================================================================== */

use pharos_server::storage::{Storage, MemoryStorage, FileStorage, LdapStorage};
use pharos_server::metrics::{CPU_USAGE, MEMORY_USAGE_BYTES, TOTAL_RECORDS, gather_metrics, check_health_thresholds};
use pharos_server::auth::{AuthManager, SecurityTier};
use pharos_server::middleware::{MiddlewareChain, LoggingMiddleware, ReadOnlyMiddleware, SecurityTierMiddleware};
use pharos_server::handle_connection;
use tokio::net::TcpListener;
use tracing::{info, error};
use tracing_subscriber;
use std::sync::{Arc, RwLock};
use sysinfo::System;
use warp::Filter;
use std::time::Duration;
use std::path::PathBuf;
use std::env;
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let use_tui = args.contains(&"--tui".to_string());

    // Initialize tracing for observability only if TUI is not taking over stdout
    if !use_tui {
        tracing_subscriber::fmt::init();
    }

    // Determine storage backend based on environment variables
    let storage: Arc<RwLock<dyn Storage>> = if let Ok(url) = env::var("PHAROS_LDAP_URL") {
        info!("Initializing LdapStorage at {}", url);
        let bind_dn = env::var("PHAROS_LDAP_BIND_DN").unwrap_or_default();
        let bind_pw = env::var("PHAROS_LDAP_BIND_PW").unwrap_or_default();
        let base_dn = env::var("PHAROS_LDAP_BASE_DN").unwrap_or_default();
        Arc::new(RwLock::new(LdapStorage::new(url, bind_dn, bind_pw, base_dn)))
    } else if let Ok(path) = env::var("PHAROS_STORAGE_PATH") {
        info!("Initializing FileStorage at {:?}", path);
        Arc::new(RwLock::new(FileStorage::new(PathBuf::from(path))))
    } else {
        info!("Initializing in-memory storage (Development Tier)");
        Arc::new(RwLock::new(MemoryStorage::new()))
    };

    // Initialize AuthManager
    let keys_dir = env::var("PHAROS_KEYS_DIR").unwrap_or_else(|_| "/home/rdelgado/.ssh/keys".to_string());
    let auth_manager = Arc::new(AuthManager::new(Path::new(&keys_dir)));

    // Initialize Middleware Chain
    let mut middleware_chain = MiddlewareChain::new();
    middleware_chain.add(Arc::new(LoggingMiddleware));

    let security_tier = match env::var("PHAROS_SECURITY_TIER").unwrap_or_else(|_| "open".to_string()).to_lowercase().as_str() {
        "protected" => SecurityTier::Protected,
        "scoped" => SecurityTier::Scoped,
        _ => SecurityTier::Open,
    };
    info!("Running with Security Tier: {:?}", security_tier);

    middleware_chain.add(Arc::new(SecurityTierMiddleware {
        default_tier: security_tier,
    }));

    middleware_chain.add(Arc::new(ReadOnlyMiddleware {
        read_only_ids: vec!["guest".to_string()],
    }));
    let middleware_chain = Arc::new(middleware_chain);

    // --- Metrics Scrape Server (Pull Method) ---
    let storage_for_metrics: Arc<RwLock<dyn Storage>> = Arc::clone(&storage);
    let metrics_route = warp::path("metrics").map(move || {
        // Update storage count on scrape
        if let Ok(lock) = storage_for_metrics.read() {
            TOTAL_RECORDS.set(lock.record_count() as i64);
        }
        gather_metrics()
    });
    
    tokio::spawn(async move {
        info!("Prometheus metrics server starting on 0.0.0.0:9090/metrics");
        warp::serve(metrics_route).run(([0, 0, 0, 0], 9090)).await;
    });

    // --- Background Metrics Collection & Health Monitoring ---
    let storage_for_monitor: Arc<RwLock<dyn Storage>> = Arc::clone(&storage);
    tokio::spawn(async move {
        let mut sys = System::new_all();
        loop {
            // Update system info
            sys.refresh_all();
            
            // Record CPU Usage (average over all CPUs)
            let cpu_load: f32 = sys.cpus().iter().map(|cpu: &sysinfo::Cpu| cpu.cpu_usage()).sum::<f32>() / sys.cpus().len() as f32;
            CPU_USAGE.set(cpu_load as f64);

            // Record Memory Usage
            let used_mem = sys.used_memory();
            MEMORY_USAGE_BYTES.set(used_mem as i64);

            // Record Storage Count
            if let Ok(lock) = storage_for_monitor.read() {
                TOTAL_RECORDS.set(lock.record_count() as i64);
            }

            // Health Monitor Threshold Warnings
            // CPU > 90% or Memory > 1GB (Arbitrary for MVP demonstration)
            check_health_thresholds(90.0, 1024 * 1024 * 1024);

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });

    let addr = "0.0.0.0:1050"; // Using 1050 for dev to avoid privileged port 105
    let listener = TcpListener::bind(addr).await?;
    info!("Pharos Server listening on {}", addr);

    if use_tui {
        tokio::spawn(async move {
            loop {
                if let Ok((socket, _)) = listener.accept().await {
                    let storage_ref: Arc<RwLock<dyn Storage>> = Arc::clone(&storage);
                    let auth_ref = Arc::clone(&auth_manager);
                    let middleware_ref = Arc::clone(&middleware_chain);
                    tokio::spawn(async move {
                        if let Err(_e) = handle_connection(socket, storage_ref, auth_ref, middleware_ref).await {
                            // Suppress error log since TUI uses stdout
                        }
                    });
                }
            }
        });
        
        if let Err(e) = pharos_server::tui::run_tui().await {
            // Restore terminal state is handled inside run_tui, just print error
            eprintln!("TUI Error: {}", e);
        }
    } else {
        loop {
            let (socket, _) = listener.accept().await?;
            let storage_ref: Arc<RwLock<dyn Storage>> = Arc::clone(&storage);
            let auth_ref = Arc::clone(&auth_manager);
            let middleware_ref = Arc::clone(&middleware_chain);
            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, storage_ref, auth_ref, middleware_ref).await {
                    error!("Error handling connection: {:?}", e);
                }
            });
        }
    }

    Ok(())
}
