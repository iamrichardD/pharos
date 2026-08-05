/* ========================================================================
 * Project: pharos
 * Component: Server Core Tests
 * File: pharos-server/tests/console_version_hardening_live.rs
 * Author: Antigravity
 * License: AGPL-3.0 (See LICENSE file for details)
 * ======================================================================== */

use pharos_server::alerting::{self, AlertState};
use pharos_server::handle_connection;
use pharos_server::storage::{FileStorage, Storage};
use pharos_server::auth::{AuthManager, SecurityTier};
use pharos_server::middleware::{MiddlewareChain, SecurityTierMiddleware};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use tempfile::tempdir;
use warp::Filter;
use std::sync::atomic::{AtomicBool, Ordering};
use ssh_key::PrivateKey;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer, SigningKey};

struct TestUser {
    pub_key: String,
    priv_key: PrivateKey,
}

impl TestUser {
    fn new() -> Self {
        let mut rng = rand::rngs::OsRng;
        let priv_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
        let pub_key = priv_key.public_key().to_openssh().unwrap();
        Self { pub_key, priv_key }
    }

    fn sign(&self, challenge: &str) -> String {
        let sig_bytes = match self.priv_key.key_data() {
            ssh_key::private::KeypairData::Ed25519(kp) => {
                let signing_key = SigningKey::from_bytes(&kp.private.to_bytes());
                signing_key.sign(challenge.as_bytes()).to_vec()
            }
            _ => panic!("Unsupported key type"),
        };
        STANDARD.encode(&sig_bytes)
    }
}

async fn authenticate(reader: &mut BufReader<TcpStream>, user: &TestUser) {
    let mut line = String::new();
    reader.get_mut().write_all(b"login tester\n").await.unwrap();
    reader.read_line(&mut line).await.unwrap();
    let challenge = line.trim().trim_start_matches("301:").to_string();
    let sig = user.sign(&challenge);
    let auth_cmd = format!("auth \"{}\" \"{}\"\n", user.pub_key, sig);
    reader.get_mut().write_all(auth_cmd.as_bytes()).await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));
}

async fn exec_cmd(reader: &mut BufReader<TcpStream>, cmd: &str) -> String {
    reader.get_mut().write_all(format!("{}\n", cmd).as_bytes()).await.unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    line.trim().to_string()
}

#[tokio::test]
async fn test_live_verification_step_5_hostname_collision_avoidance() {
    let temp_dir = tempdir().unwrap();
    let keys_dir = temp_dir.path().join("keys");
    std::fs::create_dir_all(&keys_dir).unwrap();

    let user = TestUser::new();
    std::fs::write(keys_dir.join("tester_id_ed25519.pub"), user.pub_key.as_bytes()).unwrap();

    let storage_path = temp_dir.path().join("data.json");
    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(FileStorage::new(storage_path.clone())));
    let auth_manager = Arc::new(AuthManager::new(&keys_dir, SecurityTier::Open));

    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Open }));
    let middleware_chain = Arc::new(chain);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_storage = Arc::clone(&storage);
    tokio::spawn(async move {
        loop {
            if let Ok((socket, _)) = listener.accept().await {
                let s = Arc::clone(&server_storage);
                let a = Arc::clone(&auth_manager);
                let m = Arc::clone(&middleware_chain);
                tokio::spawn(async move {
                    let _ = handle_connection(socket, "127.0.0.1:1234".to_string(), s, a, m).await;
                });
            }
        }
    });

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut reader = BufReader::new(stream);
    let mut welcome = String::new();
    reader.read_line(&mut welcome).await.unwrap();

    authenticate(&mut reader, &user).await;

    // 1. Simulate pharos-pulse registering host 'test-host'
    let resp_pulse = exec_cmd(&mut reader, "add type=\"machine\" hostname=\"test-host\" role=\"pharos-pulse\"").await;
    assert_eq!(resp_pulse, "200:Ok");

    // 2. Simulate console starting on container hostname 'test-host' with PHAROS_CONSOLE_HOSTNAME unset
    // Self-report logic falls back to test-host-console
    let console_hostname = format!("{}-console", "test-host");
    let resp_console = exec_cmd(
        &mut reader,
        &format!("add type=\"machine\" hostname=\"{}\" version=\"v1.10.15\" role=\"pharos-console-web\"", console_hostname)
    ).await;

    // Must succeed as test-host-console without 511:Collision
    assert_eq!(resp_console, "200:Ok");

    // Verify both records exist independently
    let lock = storage.read().unwrap();
    let pulse_records = lock.query(&[(Some("hostname".to_string()), "test-host".to_string())], None).unwrap();
    assert_eq!(pulse_records.len(), 1);
    let console_records = lock.query(&[(Some("hostname".to_string()), "test-host-console".to_string())], None).unwrap();
    assert_eq!(console_records.len(), 1);
}

#[tokio::test]
async fn test_live_verification_step_6_version_mismatch_normalization() {
    let temp_dir = tempdir().unwrap();
    let storage_path = temp_dir.path().join("data.json");
    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(FileStorage::new(storage_path)));

    // Setup local mock webhook endpoint
    let webhook_called = Arc::new(AtomicBool::new(false));
    let webhook_called_clone = Arc::clone(&webhook_called);
    let webhook_route = warp::post()
        .and(warp::path("webhook"))
        .and(warp::body::json())
        .map(move |body: serde_json::Value| {
            webhook_called_clone.store(true, Ordering::SeqCst);
            println!("Mock Webhook Received Event: {:?}", body);
            warp::reply::json(&"ok")
        });

    let (addr, server) = warp::serve(webhook_route).bind_ephemeral(([127, 0, 0, 1], 0));
    let webhook_url = format!("http://{}/webhook", addr);
    tokio::spawn(server);

    // 1. Add record with self-reported version="v1.10.15" and expected_version="1.10.15" (no 'v' prefix)
    {
        let mut lock = storage.write().unwrap();
        let mut fields = HashMap::new();
        fields.insert("type".to_string(), "machine".to_string());
        fields.insert("hostname".to_string(), "test-host-console".to_string());
        fields.insert("version".to_string(), "v1.10.15".to_string());
        fields.insert("expected_version".to_string(), "1.10.15".to_string());
        lock.upsert_record(fields, None, None).unwrap();
    }

    let mut alert_state = AlertState::default();

    // Check version mismatches -> Should NOT trigger webhook because normalized versions match!
    alerting::check_version_mismatches(
        &storage,
        &mut alert_state,
        Some(&webhook_url),
        None,
    ).await;

    // Small delay to allow any async task if it were spawned
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(!webhook_called.load(Ordering::SeqCst), "Webhook should NOT have fired for v1.10.15 vs 1.10.15");

    // 2. Now update expected_version to "v2.0.0-different"
    {
        let mut lock = storage.write().unwrap();
        let mut fields = HashMap::new();
        fields.insert("type".to_string(), "machine".to_string());
        fields.insert("hostname".to_string(), "test-host-console".to_string());
        fields.insert("version".to_string(), "v1.10.15".to_string());
        fields.insert("expected_version".to_string(), "v2.0.0-different".to_string());
        lock.upsert_record(fields, None, None).unwrap();
    }

    // Check version mismatches -> SHOULD trigger webhook for genuine mismatch!
    alerting::check_version_mismatches(
        &storage,
        &mut alert_state,
        Some(&webhook_url),
        None,
    ).await;

    // Allow time for tokio::spawn webhook task
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(webhook_called.load(Ordering::SeqCst), "Webhook SHOULD have fired for v1.10.15 vs v2.0.0-different");
}
