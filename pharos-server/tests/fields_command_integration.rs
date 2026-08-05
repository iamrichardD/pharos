/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/tests/fields_command_integration.rs
 * Author: Antigravity
 * License: AGPL-3.0 (See LICENSE file for details)
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

async fn setup_test_server(keys_dir: &std::path::Path) -> (std::net::SocketAddr, Arc<RwLock<dyn Storage>>) {
    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let auth_manager = Arc::new(AuthManager::new(keys_dir, SecurityTier::Open));

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

    (addr, storage)
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

async fn read_fields_response(reader: &mut BufReader<TcpStream>) -> Vec<String> {
    let mut response_lines = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        reader.read_line(&mut line).await.unwrap();
        let trimmed = line.trim().to_string();
        let is_terminal = trimmed == "200:Ok." || trimmed.starts_with("507:");
        response_lines.push(trimmed);
        if is_terminal {
            break;
        }
    }
    response_lines
}

#[tokio::test]
async fn test_fields_empty_server() {
    let keys_dir = tempdir().unwrap();
    let (addr, _) = setup_test_server(keys_dir.path()).await;
    let stream = TcpStream::connect(addr).await.unwrap();
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap(); // Consume welcome

    reader.get_mut().write_all(b"fields\n").await.unwrap();
    let response_lines = read_fields_response(&mut reader).await;

    // Expected baseline fields in alphabetical order:
    // alias (1), created_at (2), hostname (3), last_seen_at (4), status (5), type (6)
    assert_eq!(response_lines.len(), 13); // 6 fields * 2 lines each + 1 termination line = 13 lines

    assert_eq!(response_lines[0], "-200:1:alias:max 32 Public");
    assert_eq!(response_lines[1], "-200:1:alias:Unique short identifier for a person entry; used to detect an existing record on add/upsert.");

    assert_eq!(response_lines[2], "-200:2:created_at:max 32 Public");
    assert_eq!(response_lines[3], "-200:2:created_at:ISO-8601 timestamp of when this entry was first created (server-injected).");

    assert_eq!(response_lines[4], "-200:3:hostname:max 256 Public");
    assert_eq!(response_lines[5], "-200:3:hostname:Unique identifier for a machine entry; used to detect an existing record on add/upsert.");

    assert_eq!(response_lines[6], "-200:4:last_seen_at:max 32 Public");
    assert_eq!(response_lines[7], "-200:4:last_seen_at:ISO-8601 timestamp of the most recent update to this entry (server-injected).");

    assert_eq!(response_lines[8], "-200:5:status:max 64 Public");
    assert_eq!(response_lines[9], "-200:5:status:Free-form status/presence value (e.g. \"active\", \"online\", \"offline\").");

    assert_eq!(response_lines[10], "-200:6:type:max 64 Public");
    assert_eq!(response_lines[11], "-200:6:type:Record type discriminator (e.g. \"person\" or \"machine\").");

    assert_eq!(response_lines[12], "200:Ok.");
}

#[tokio::test]
async fn test_fields_with_dynamic_field() {
    let keys_dir = tempdir().unwrap();
    let test_user = TestUser::new();
    std::fs::write(keys_dir.path().join("tester_id_ed25519.pub"), test_user.pub_key.as_bytes()).unwrap();

    let (addr, _) = setup_test_server(keys_dir.path()).await;
    let stream = TcpStream::connect(addr).await.unwrap();
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap(); // Consume welcome
    authenticate(&mut reader, &test_user).await;

    // Add a record with an ad-hoc field: email
    reader.get_mut().write_all(b"add hostname=x type=machine email=foo@example.com\n").await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));

    reader.get_mut().write_all(b"fields\n").await.unwrap();
    let response_lines = read_fields_response(&mut reader).await;

    // Sorted list of fields: alias, created_at, email, hostname, last_seen_at, status, type
    // email should be at id 3 (0-based index 2 -> two lines each -> lines[4], lines[5])
    assert_eq!(response_lines.len(), 15); // 7 fields * 2 + 1 = 15 lines

    assert_eq!(response_lines[4], "-200:3:email:max 256 Public");
    assert_eq!(response_lines[5], "-200:3:email:User-defined field; no additional metadata available.");
}

#[tokio::test]
async fn test_fields_filtering_single_existing() {
    let keys_dir = tempdir().unwrap();
    let (addr, _) = setup_test_server(keys_dir.path()).await;
    let stream = TcpStream::connect(addr).await.unwrap();
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap(); // Consume welcome

    reader.get_mut().write_all(b"fields hostname\n").await.unwrap();
    let response_lines = read_fields_response(&mut reader).await;

    assert_eq!(response_lines.len(), 3);
    assert_eq!(response_lines[0], "-200:3:hostname:max 256 Public");
    assert_eq!(response_lines[1], "-200:3:hostname:Unique identifier for a machine entry; used to detect an existing record on add/upsert.");
    assert_eq!(response_lines[2], "200:Ok.");
}

#[tokio::test]
async fn test_fields_filtering_non_existent() {
    let keys_dir = tempdir().unwrap();
    let (addr, _) = setup_test_server(keys_dir.path()).await;
    let stream = TcpStream::connect(addr).await.unwrap();
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap(); // Consume welcome

    reader.get_mut().write_all(b"fields nosuchfield\n").await.unwrap();
    let response_lines = read_fields_response(&mut reader).await;

    assert_eq!(response_lines, vec!["507:Field does not exist".to_string()]);
}

#[tokio::test]
async fn test_fields_filtering_partial() {
    let keys_dir = tempdir().unwrap();
    let (addr, _) = setup_test_server(keys_dir.path()).await;
    let stream = TcpStream::connect(addr).await.unwrap();
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap(); // Consume welcome

    reader.get_mut().write_all(b"fields hostname nosuchfield\n").await.unwrap();
    let response_lines = read_fields_response(&mut reader).await;

    assert_eq!(response_lines.len(), 3);
    assert_eq!(response_lines[0], "-200:3:hostname:max 256 Public");
    assert_eq!(response_lines[2], "200:Ok.");
}

#[tokio::test]
async fn test_fields_id_stability() {
    let keys_dir = tempdir().unwrap();
    let test_user = TestUser::new();
    std::fs::write(keys_dir.path().join("tester_id_ed25519.pub"), test_user.pub_key.as_bytes()).unwrap();

    let (addr, _) = setup_test_server(keys_dir.path()).await;
    let stream = TcpStream::connect(addr).await.unwrap();
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap(); // Consume welcome
    authenticate(&mut reader, &test_user).await;

    // Get initial fields list.
    reader.get_mut().write_all(b"fields\n").await.unwrap();
    let response_lines = read_fields_response(&mut reader).await;
    assert_eq!(response_lines[4], "-200:3:hostname:max 256 Public");

    // Add a field starting with 'z' (e.g. zipcode) - sorts after every baseline field.
    reader.get_mut().write_all(b"add hostname=x type=machine zipcode=12345\n").await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));

    reader.get_mut().write_all(b"fields\n").await.unwrap();
    let response_lines = read_fields_response(&mut reader).await;

    // hostname must still be id 3 - zipcode sorts to the end, not before it.
    assert_eq!(response_lines[4], "-200:3:hostname:max 256 Public");
    assert_eq!(response_lines[12], "-200:7:zipcode:max 256 Public");
}
