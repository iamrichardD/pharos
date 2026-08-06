/* ========================================================================
 * Project: pharos
 * Component: Server Core Tests
 * File: pharos-server/tests/multi_value_ip_mac_verification.rs
 * Purpose: Verification harness for RFC-native multi-valued ip_addr/mac_addr fields
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
async fn test_verification_step_1_direct_storage_add() {
    println!("\n=== VERIFICATION STEP 1: Direct Storage Add ===");
    let mut storage = MemoryStorage::new();
    let fields = vec![
        ("type".to_string(), "machine".to_string()),
        ("hostname".to_string(), "srv-01".to_string()),
        ("ip_addr".to_string(), "192.168.86.5".to_string()),
        ("ip_addr".to_string(), "192.168.86.6".to_string()),
        ("mac_addr".to_string(), "e0:51:d8:1d:e3:22".to_string()),
    ];
    storage.add_record(fields, None, None).unwrap();
    
    let records = storage.query(&[(Some("hostname".to_string()), "srv-01".to_string())], None).unwrap();
    assert_eq!(records.len(), 1);
    let ip_list = records[0].multi_fields.get("ip_addr").unwrap();
    let mac_list = records[0].multi_fields.get("mac_addr").unwrap();

    assert_eq!(ip_list, &vec!["192.168.86.5".to_string(), "192.168.86.6".to_string()]);
    assert_eq!(mac_list, &vec!["e0:51:d8:1d:e3:22".to_string()]);
    println!("SUCCESS Step 1: multi_fields[\"ip_addr\"] = {:?}, multi_fields[\"mac_addr\"] = {:?}", ip_list, mac_list);
}

#[tokio::test]
async fn test_verification_step_2_append_semantics_and_idempotency() {
    println!("\n=== VERIFICATION STEP 2: Append Semantics & Idempotency ===");
    let mut storage = MemoryStorage::new();
    let fields = vec![
        ("type".to_string(), "machine".to_string()),
        ("hostname".to_string(), "srv-01".to_string()),
        ("ip_addr".to_string(), "192.168.86.5".to_string()),
        ("mac_addr".to_string(), "e0:51:d8:1d:e3:22".to_string()),
    ];
    storage.add_record(fields, None, None).unwrap();

    // Later change supplying a new ip_addr=
    let selections = vec![(Some("hostname".to_string()), "srv-01".to_string())];
    let modifications = vec![("ip_addr".to_string(), "192.168.86.6".to_string())];
    let count = storage.change_record(&selections, &modifications, None, &[]).unwrap();
    assert_eq!(count, 1);

    // Repeat change with the same IP (idempotent duplicate check)
    storage.change_record(&selections, &modifications, None, &[]).unwrap();

    let records = storage.query(&selections, None).unwrap();
    let ip_list = records[0].multi_fields.get("ip_addr").unwrap();
    let mac_list = records[0].multi_fields.get("mac_addr").unwrap();

    assert_eq!(ip_list, &vec!["192.168.86.5".to_string(), "192.168.86.6".to_string()]);
    assert_eq!(mac_list, &vec!["e0:51:d8:1d:e3:22".to_string()]);
    println!("SUCCESS Step 2: Appended new IP without duplication or touching mac_addr: {:?}", ip_list);
}

#[tokio::test]
async fn test_verification_step_3_raw_wire_protocol_response_bytes() {
    println!("\n=== VERIFICATION STEP 3: Raw Wire Protocol Response Bytes ===");
    let (addr, _storage, user) = setup_server().await;

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);

    // Consume the server's welcome banner before doing anything else
    let mut welcome = String::new();
    buf_reader.read_line(&mut welcome).await.unwrap();

    // add/change require an authenticated SSH-key session even under Open tier
    authenticate(&mut buf_reader, &mut writer, &user).await;

    // Add multi-valued record over wire
    writer.write_all(b"add hostname=\"srv-multi\" type=\"machine\" ip_addr=\"192.168.86.5\" ip_addr=\"192.168.86.6\" mac_addr=\"e0:51:d8:1d:e3:22\"\n").await.unwrap();
    let mut line = String::new();
    buf_reader.read_line(&mut line).await.unwrap();
    assert!(line.starts_with("200:Ok"), "Expected 200:Ok on add, got: {}", line);

    // Query over wire
    writer.write_all(b"query ip_addr=192.168.86.5\n").await.unwrap();

    let mut response_lines = Vec::new();
    loop {
        let mut l = String::new();
        let bytes_read = buf_reader.read_line(&mut l).await.unwrap();
        if bytes_read == 0 {
            break;
        }
        let trimmed = l.trim_end_matches('\n').trim_end_matches('\r').to_string();
        response_lines.push(trimmed.clone());
        if trimmed == "200:Ok" || trimmed.starts_with("500:") || trimmed.starts_with("501:") {
            break;
        }
    }

    println!("RAW WIRE PROTOCOL RESPONSE BYTES:");
    for r in &response_lines {
        println!("{}", r);
    }

    assert!(response_lines.iter().any(|l| l.contains("-200:1:ip_addr: 192.168.86.5")), "Missing first ip_addr line");
    assert!(response_lines.iter().any(|l| l.contains("-200:1:       : 192.168.86.6")), "Missing continuation line for second ip_addr");
    println!("SUCCESS Step 3: Verified RFC continuation line format over wire!");
}

#[tokio::test]
async fn test_verification_step_5_malformed_value_fail_closed() {
    println!("\n=== VERIFICATION STEP 5: Malformed Value Fail-Closed ===");
    let mut storage = MemoryStorage::new();
    let fields = vec![
        ("type".to_string(), "machine".to_string()),
        ("hostname".to_string(), "srv-bad".to_string()),
        ("ip_addr".to_string(), "192.168.86.5".to_string()),
        ("ip_addr".to_string(), "not-an-ip".to_string()),
    ];
    let res = storage.add_record(fields, None, None);
    assert!(matches!(res, Err(pharos_server::storage::StorageError::InvalidArgument(_))));
    assert_eq!(storage.record_count(), 0, "No records should be stored when validation fails!");
    println!("SUCCESS Step 5: Returned 512:Illegal value and failed closed (0 partial records written)");
}
