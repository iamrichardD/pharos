/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/tests/set_command_integration.rs
 * Author: Antigravity
 * License: AGPL-3.0 (See LICENSE file for details)
 * ======================================================================== */

use pharos_server::handle_connection;
use pharos_server::storage::{MemoryStorage, Storage};
use pharos_server::auth::{AuthManager, SecurityTier};
use pharos_server::middleware::{MiddlewareChain, RbacMiddleware, SecurityTierMiddleware};
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
    fn new(_alias: &str) -> Self {
        let mut rng = rand::rngs::OsRng;
        let priv_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
        let pub_key = priv_key.public_key().to_openssh().unwrap();
        Self {
            pub_key,
            priv_key,
        }
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

async fn setup_rbac_server(keys_dir: &std::path::Path) -> (std::net::SocketAddr, Arc<RwLock<dyn Storage>>) {
    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let auth_manager = Arc::new(AuthManager::new(keys_dir, SecurityTier::Open));
    
    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Open }));
    chain.add(Arc::new(RbacMiddleware));
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

async fn authenticate_user(reader: &mut BufReader<TcpStream>, user: &TestUser, name: &str) {
    let mut line = String::new();
    reader.get_mut().write_all(format!("login {}\n", name).as_bytes()).await.unwrap();
    reader.read_line(&mut line).await.unwrap();
    let challenge = line.trim().trim_start_matches("301:").to_string();
    let sig = user.sign(&challenge);
    let auth_cmd = format!("auth \"{}\" \"{}\"\n", user.pub_key, sig);
    reader.get_mut().write_all(auth_cmd.as_bytes()).await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));
}

#[tokio::test]
async fn test_set_session_options() {
    let _ = tracing_subscriber::fmt::try_init();
    let keys_dir = tempdir().unwrap();
    let test_user = TestUser::new("admin");
    std::fs::write(keys_dir.path().join("admin_id_ed25519.pub"), test_user.pub_key.as_bytes()).unwrap();
    
    let (addr, storage) = setup_rbac_server(keys_dir.path()).await;
    
    let stream = TcpStream::connect(addr).await.unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    
    reader.read_line(&mut line).await.unwrap(); // Database ready
    
    // Authenticate
    authenticate_user(&mut reader, &test_user, "admin").await;
    
    // 1. set with no arguments returns all 7 options with default values
    reader.get_mut().write_all(b"set\n").await.unwrap();
    let mut options = Vec::new();
    loop {
        line.clear();
        reader.read_line(&mut line).await.unwrap();
        if line.starts_with("200:Done.") {
            break;
        }
        options.push(line.trim().to_string());
    }
    assert_eq!(options, vec![
        "-200:echo:off",
        "-200:limit:off",
        "-200:charset:us-ascii",
        "-200:verbose:off",
        "-200:addonly:off",
        "-200:nolog:off",
        "-200:external:off"
    ]);

    // 2. set limit=1 then 200:Done.; then set again shows -200:limit:1
    reader.get_mut().write_all(b"set limit=1\n").await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Done."));
    
    reader.get_mut().write_all(b"set\n").await.unwrap();
    options.clear();
    loop {
        line.clear();
        reader.read_line(&mut line).await.unwrap();
        if line.starts_with("200:Done.") {
            break;
        }
        options.push(line.trim().to_string());
    }
    assert_eq!(options, vec![
        "-200:echo:off",
        "-200:limit:1",
        "-200:charset:us-ascii",
        "-200:verbose:off",
        "-200:addonly:off",
        "-200:nolog:off",
        "-200:external:off"
    ]);

    // Add two records
    reader.get_mut().write_all(b"add name=alice role=admin status=active\n").await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));
    
    reader.get_mut().write_all(b"add name=bob role=user status=active\n").await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));

    // Verify 2 records exist in storage
    {
        let lock = storage.read().unwrap();
        assert_eq!(lock.record_count(), 2);
    }

    // 3. With limit=1 active and 2+ records matching a broad delete selection: the delete is rejected with 518:
    reader.get_mut().write_all(b"delete status=active\n").await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.starts_with("518:"), "Expected 518: but got: {}", line);
    
    // Verify records are still present
    {
        let lock = storage.read().unwrap();
        assert_eq!(lock.record_count(), 2);
    }

    // 4. Same as #3 but for change: with limit=1 active and 2+ matching records, change is rejected with 518: and no record modified.
    reader.get_mut().write_all(b"change status=active make city=london\n").await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.starts_with("518:"), "Expected 518: but got: {}", line);
    
    // Verify no city fields were added/modified
    {
        let lock = storage.read().unwrap();
        let records = lock.query(&[], None).unwrap();
        for r in records {
            assert!(!r.fields.contains_key("city"));
        }
    }

    // Turn off limit, turn on addonly
    reader.get_mut().write_all(b"set limit=off addonly=on\n").await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Done."));

    // 5. set addonly=on, then a change targeting a field that already exists on the matched record is rejected with 521:
    // Targeting name="alice" to change name="alex" (overwriting existing field "name")
    reader.get_mut().write_all(b"change name=alice make name=alex\n").await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.starts_with("521:"), "Expected 521: but got: {}", line);
    
    // Verify "name" is still "alice"
    {
        let lock = storage.read().unwrap();
        let records = lock.query(&[(Some("name".to_string()), "alice".to_string())], None).unwrap();
        assert_eq!(records.len(), 1);
    }

    // 6. set addonly=on, then a change that only sets a field the record does NOT already have succeeds normally (200: and the field is now present)
    reader.get_mut().write_all(b"change name=alice make age=25\n").await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:"));
    
    {
        let lock = storage.read().unwrap();
        let records = lock.query(&[(Some("name".to_string()), "alice".to_string())], None).unwrap();
        assert_eq!(records[0].fields.get("age").unwrap(), "25");
    }

    // 7. An invalid token, e.g. set limit=notanumber, returns 512:Illegal value and limit remains unchanged
    reader.get_mut().write_all(b"set limit=notanumber\n").await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("512:Illegal value"));
    
    // Verify limit is still off/None
    reader.get_mut().write_all(b"set\n").await.unwrap();
    options.clear();
    loop {
        line.clear();
        reader.read_line(&mut line).await.unwrap();
        if line.starts_with("200:Done.") {
            break;
        }
        options.push(line.trim().to_string());
    }
    assert!(options.contains(&"-200:limit:off".to_string()));

    // 8. set nosuchoption=on returns 513:Unknown option
    reader.get_mut().write_all(b"set nosuchoption=on\n").await.unwrap();
    line.clear();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("513:Unknown option"));
}
