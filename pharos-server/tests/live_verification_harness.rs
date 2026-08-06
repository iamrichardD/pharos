/* ========================================================================
 * Project: pharos
 * Component: Server Core Tests
 * File: pharos-server/tests/live_verification_harness.rs
 * Author: Antigravity
 * License: AGPL-3.0 (See LICENSE file for details)
 * ======================================================================== */

use pharos_server::handle_connection;
use pharos_server::storage::{FileStorage, Storage, RecordType};
use pharos_server::auth::{AuthManager, SecurityTier};
use pharos_server::middleware::{MiddlewareChain, SecurityTierMiddleware};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
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
async fn test_live_verification_steps_3_4_5_6_7() {
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
    reader.read_line(&mut welcome).await.unwrap(); // consume welcome

    authenticate(&mut reader, &user).await;

    println!("\n=== STEP 3: Raw add command missing type field ===");
    let resp3 = exec_cmd(&mut reader, "add hostname=notype-host status=active").await;
    println!("Step 3 Response: {}", resp3);
    assert_eq!(resp3, "512:Illegal value: a 'type' field is required (e.g. type=machine)");

    println!("\n=== STEP 4: Upsert record with conflicting type=person ===");
    let resp4_init = exec_cmd(&mut reader, "add hostname=srv-upsert type=machine status=initial").await;
    println!("Step 4 Init Response: {}", resp4_init);
    assert_eq!(resp4_init, "200:Ok");

    let resp4_mismatch = exec_cmd(&mut reader, "add hostname=srv-upsert type=person status=should_not_apply").await;
    println!("Step 4 Mismatch Response: {}", resp4_mismatch);
    assert_eq!(resp4_mismatch, "512:Illegal value: type is immutable after creation and cannot be changed");

    // Verify record fields on server are unchanged
    {
        let lock = storage.read().unwrap();
        let records = lock.query(&[(Some("hostname".to_string()), "srv-upsert".to_string())], None).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].fields.get("type").unwrap(), "machine");
        assert_eq!(records[0].fields.get("status").unwrap(), "initial");
        println!("Step 4 Verification: srv-upsert fields strictly unchanged: type=machine, status=initial");
    }

    println!("\n=== STEP 5: Attempt change hostname=srv-upsert type=anything ===");
    let resp5_other = exec_cmd(&mut reader, "change hostname=srv-upsert make type=other status=changed").await;
    println!("Step 5 Change type=other Response: {}", resp5_other);
    assert_eq!(resp5_other, "512:Illegal value: type cannot be modified via change - it is set once at record creation");

    let resp5_same = exec_cmd(&mut reader, "change hostname=srv-upsert make type=machine status=changed").await;
    println!("Step 5 Change type=machine Response: {}", resp5_same);
    assert_eq!(resp5_same, "512:Illegal value: type cannot be modified via change - it is set once at record creation");

    // Verify record fields on server are unchanged
    {
        let lock = storage.read().unwrap();
        let records = lock.query(&[(Some("hostname".to_string()), "srv-upsert".to_string())], None).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].fields.get("type").unwrap(), "machine");
        assert_eq!(records[0].fields.get("status").unwrap(), "initial");
        println!("Step 5 Verification: srv-upsert fields strictly unchanged after failed changes");
    }

    println!("\n=== STEP 6: Self-heal record_type: null with fields['type'] == 'machine' ===");
    let heal_path = temp_dir.path().join("heal_data.json");
    let broken_json = r#"[
        {
            "id": 1,
            "record_type": null,
            "fields": {
                "hostname": "srv-heal",
                "type": "machine",
                "status": "active"
            },
            "owner_fingerprint": null,
            "owner_team": null
        }
    ]"#;
    std::fs::write(&heal_path, broken_json).unwrap();

    let mut heal_storage = FileStorage::new(heal_path.clone());
    let records = heal_storage.query(&[(Some("hostname".to_string()), "srv-heal".to_string())], None).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].record_type, Some(RecordType::Machine));
    println!("Step 6 Self-heal Result: record_type correctly healed to Machine: {:?}", records[0].record_type);

    // Subsequent heartbeat-style upsert asserting type=machine succeeds normally
    let mut heartbeat_fields = HashMap::new();
    heartbeat_fields.insert("hostname".to_string(), "srv-heal".to_string());
    heartbeat_fields.insert("type".to_string(), "machine".to_string());
    heartbeat_fields.insert("status".to_string(), "heartbeat_ok".to_string());
    let upsert_res = heal_storage.upsert_record(heartbeat_fields.into_iter().collect(), None, None);
    assert!(upsert_res.is_ok());
    let updated_heal = heal_storage.query(&[(Some("hostname".to_string()), "srv-heal".to_string())], None).unwrap();
    assert_eq!(updated_heal[0].fields.get("status").unwrap(), "heartbeat_ok");
    println!("Step 6 Heartbeat Upsert Result: heartbeat upsert succeeded normally after self-heal!");

    println!("\n=== STEP 7: Self-heal record with missing fields['type'] ===");
    let missing_type_path = temp_dir.path().join("missing_type.json");
    let missing_type_json = r#"[
        {
            "id": 1,
            "record_type": null,
            "fields": {
                "hostname": "srv-no-type"
            },
            "owner_fingerprint": null,
            "owner_team": null
        }
    ]"#;
    std::fs::write(&missing_type_path, missing_type_json).unwrap();
    let missing_storage = FileStorage::new(missing_type_path.clone());
    let missing_records = missing_storage.query(&[(Some("hostname".to_string()), "srv-no-type".to_string())], None).unwrap();
    assert_eq!(missing_records.len(), 1);
    assert_eq!(missing_records[0].record_type, None);
    println!("Step 7 Missing type field Result: left record_type as None as expected: {:?}", missing_records[0].record_type);
}
