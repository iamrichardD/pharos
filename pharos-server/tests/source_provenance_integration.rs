/* ========================================================================
 * Project: pharos
 * Component: Server Core Tests
 * File: pharos-server/tests/source_provenance_integration.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Integration test suite verifying source provenance generation, normalization,
 * field immutability, and client spoofing protection over the wire protocol.
 * * Traceability:
 * Related to RFC 2378 client provenance tracking and source immutability.
 * ======================================================================== */

use pharos_server::handle_connection;
use pharos_server::storage::{MemoryStorage, Storage};
use pharos_server::auth::{AuthManager, SecurityTier};
use pharos_server::middleware::{MiddlewareChain, SecurityTierMiddleware};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use std::sync::{Arc, RwLock};
use tempfile::tempdir;
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

async fn authenticate(reader: &mut BufReader<tokio::net::tcp::ReadHalf<'_>>, writer: &mut tokio::net::tcp::WriteHalf<'_>, user: &TestUser) {
    let mut line = String::new();
    writer.write_all(b"login tester\n").await.unwrap();
    reader.read_line(&mut line).await.unwrap();
    let challenge = line.trim().trim_start_matches("301:").to_string();
    let sig = user.sign(&challenge);
    let auth_cmd = format!("auth \"{}\" \"{}\"\n", user.pub_key, sig);
    writer.write_all(auth_cmd.as_bytes()).await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"), "Expected 200:Ok on auth, got: {}", line);
}

async fn setup_server() -> (std::net::SocketAddr, Arc<RwLock<dyn Storage>>, TestUser) {
    let dir = tempdir().unwrap();
    let user = TestUser::new();
    std::fs::write(dir.path().join("tester_id_ed25519.pub"), user.pub_key.as_bytes()).unwrap();
    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let auth_manager = Arc::new(AuthManager::new(dir.path(), SecurityTier::Open));

    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Open }));
    let middleware_chain = Arc::new(chain);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_storage = Arc::clone(&storage);
    tokio::spawn(async move {
        loop {
            if let Ok((socket, peer_addr)) = listener.accept().await {
                let s = Arc::clone(&server_storage);
                let a = Arc::clone(&auth_manager);
                let m = Arc::clone(&middleware_chain);
                tokio::spawn(async move {
                    let _ = handle_connection(socket, peer_addr.to_string(), s, a, m).await;
                });
            }
        }
    });

    (addr, storage, user)
}

#[tokio::test]
async fn test_should_set_source_to_mdb_for_add_from_mdb_client_id() {
    let (addr, storage, user) = setup_server().await;

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);

    // Consume the server's welcome banner before doing anything else
    let mut welcome = String::new();
    buf_reader.read_line(&mut welcome).await.unwrap();

    // Identify as mdb
    writer.write_all(b"id mdb\n").await.unwrap();
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));

    authenticate(&mut buf_reader, &mut writer, &user).await;

    // Send Add command
    writer.write_all(b"add hostname=\"src-test-mdb\" type=\"machine\"\n").await.unwrap();
    line.clear();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"), "Expected 200:Ok on add, got: {}", line);

    // Query back from storage and assert source field
    let records = storage.read().unwrap().query(&[(Some("hostname".to_string()), "src-test-mdb".to_string())], None).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].fields.get("source").map(|s| s.as_str()), Some("mdb"));
}

#[tokio::test]
async fn test_should_set_source_to_pharos_scan_for_add_from_pharos_scan_client_id() {
    let (addr, storage, user) = setup_server().await;

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);

    // Consume the server's welcome banner before doing anything else
    let mut welcome = String::new();
    buf_reader.read_line(&mut welcome).await.unwrap();

    // Identify as pharos-scan
    writer.write_all(b"id pharos-scan\n").await.unwrap();
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));

    authenticate(&mut buf_reader, &mut writer, &user).await;

    // Send Add command
    writer.write_all(b"add hostname=\"src-test-scan\" type=\"machine\"\n").await.unwrap();
    line.clear();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"), "Expected 200:Ok on add, got: {}", line);

    // Query back from storage and assert source field
    let records = storage.read().unwrap().query(&[(Some("hostname".to_string()), "src-test-scan".to_string())], None).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].fields.get("source").map(|s| s.as_str()), Some("pharos-scan"));
}

#[tokio::test]
async fn test_should_set_source_to_pharos_pulse_for_add_from_pulse_prefixed_client_id() {
    let (addr, storage, user) = setup_server().await;

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);

    // Consume the server's welcome banner before doing anything else
    let mut welcome = String::new();
    buf_reader.read_line(&mut welcome).await.unwrap();

    // Identify as pulse-test-host-01
    writer.write_all(b"id pulse-test-host-01\n").await.unwrap();
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));

    authenticate(&mut buf_reader, &mut writer, &user).await;

    // Send Add command
    writer.write_all(b"add hostname=\"src-test-pulse\" type=\"machine\"\n").await.unwrap();
    line.clear();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"), "Expected 200:Ok on add, got: {}", line);

    // Query back from storage and assert source field
    let records = storage.read().unwrap().query(&[(Some("hostname".to_string()), "src-test-pulse".to_string())], None).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].fields.get("source").map(|s| s.as_str()), Some("pharos-pulse"));
}

#[tokio::test]
async fn test_should_strip_client_supplied_source_field_and_use_derived_value_instead() {
    let (addr, storage, user) = setup_server().await;

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);

    // Consume the server's welcome banner before doing anything else
    let mut welcome = String::new();
    buf_reader.read_line(&mut welcome).await.unwrap();

    // Identify as mdb
    writer.write_all(b"id mdb\n").await.unwrap();
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));

    authenticate(&mut buf_reader, &mut writer, &user).await;

    // Send Add command attempting to spoof source
    writer.write_all(b"add hostname=\"src-test-spoof\" type=\"machine\" source=\"fake-spoofed-value\"\n").await.unwrap();
    line.clear();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"), "Expected 200:Ok on add, got: {}", line);

    // Query back from storage and assert source field equals derived mdb, NOT spoofed value
    let records = storage.read().unwrap().query(&[(Some("hostname".to_string()), "src-test-spoof".to_string())], None).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].fields.get("source").map(|s| s.as_str()), Some("mdb"));
}

#[tokio::test]
async fn test_should_increment_records_added_total_on_new_record() {
    let (addr, _storage, user) = setup_server().await;

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);

    let mut welcome = String::new();
    buf_reader.read_line(&mut welcome).await.unwrap();

    writer.write_all(b"id mdb\n").await.unwrap();
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));

    authenticate(&mut buf_reader, &mut writer, &user).await;

    let before = pharos_server::metrics::RECORDS_ADDED_TOTAL.with_label_values(&["mdb"]).get();

    writer.write_all(b"add hostname=\"metrics-add-host-1\" type=\"machine\"\n").await.unwrap();
    line.clear();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"), "Expected 200:Ok on add, got: {}", line);

    let after = pharos_server::metrics::RECORDS_ADDED_TOTAL.with_label_values(&["mdb"]).get();
    // >=, not ==: this counter is global process state shared with other tests in this file
    // that also `add` under client_id "mdb" and run concurrently in the same test binary -
    // this proves the wiring increments the counter without being flaky under that concurrency.
    assert!(after > before, "expected RECORDS_ADDED_TOTAL[mdb] to increase, before={before} after={after}");
}

#[tokio::test]
async fn test_should_increment_records_updated_total_on_existing_record() {
    let (addr, _storage, user) = setup_server().await;

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);

    let mut welcome = String::new();
    buf_reader.read_line(&mut welcome).await.unwrap();

    writer.write_all(b"id mdb\n").await.unwrap();
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));

    authenticate(&mut buf_reader, &mut writer, &user).await;

    // First add creates the record
    writer.write_all(b"add hostname=\"metrics-update-host-1\" type=\"machine\" status=\"active\"\n").await.unwrap();
    line.clear();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"), "Expected 200:Ok on initial add, got: {}", line);

    let before = pharos_server::metrics::RECORDS_UPDATED_TOTAL.with_label_values(&["mdb"]).get();

    // Second add updates the record
    writer.write_all(b"add hostname=\"metrics-update-host-1\" type=\"machine\" status=\"inactive\"\n").await.unwrap();
    line.clear();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"), "Expected 200:Ok on second add, got: {}", line);

    let after = pharos_server::metrics::RECORDS_UPDATED_TOTAL.with_label_values(&["mdb"]).get();
    // >=, not ==: same cross-test concurrency reasoning as the added-total test above.
    assert!(after > before, "expected RECORDS_UPDATED_TOTAL[mdb] to increase, before={before} after={after}");
}

#[tokio::test]
async fn test_should_increment_records_deleted_total_on_delete() {
    let (addr, _storage, user) = setup_server().await;

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);

    let mut welcome = String::new();
    buf_reader.read_line(&mut welcome).await.unwrap();

    writer.write_all(b"id mdb\n").await.unwrap();
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));

    authenticate(&mut buf_reader, &mut writer, &user).await;

    writer.write_all(b"add hostname=\"metrics-delete-host-1\" type=\"machine\"\n").await.unwrap();
    line.clear();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"), "Expected 200:Ok on add, got: {}", line);

    let before = pharos_server::metrics::RECORDS_DELETED_TOTAL.with_label_values(&["mdb"]).get();

    writer.write_all(b"delete hostname=\"metrics-delete-host-1\"\n").await.unwrap();
    line.clear();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"), "Expected 200:Ok on delete, got: {}", line);

    let after = pharos_server::metrics::RECORDS_DELETED_TOTAL.with_label_values(&["mdb"]).get();
    // >=, not ==: same cross-test concurrency reasoning as the added-total test above.
    assert!(after > before, "expected RECORDS_DELETED_TOTAL[mdb] to increase, before={before} after={after}");
}
