/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/tests/rbac_integration.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Integration test to verify refined RBAC (Role-Based Access Control)
 * as documented in artifacts/pharos-auth-decision-tree.md.
 * * Traceability:
 * Related to Refined RBAC Task.
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
    let auth_manager = Arc::new(AuthManager::new(keys_dir));
    
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
                    let _ = handle_connection(socket, s, a, m).await;
                });
            }
        }
    });

    (addr, storage)
}

#[tokio::test]
async fn test_should_allow_guest_search_but_deny_delete() {
    let _ = tracing_subscriber::fmt::try_init();
    let keys_dir = tempdir().unwrap();
    let (addr, _) = setup_rbac_server(keys_dir.path()).await;
    
    let stream = TcpStream::connect(addr).await.unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    
    reader.read_line(&mut line).await.unwrap(); // welcome
    
    // 1. Guest Search (should be allowed)
    line.clear();
    reader.get_mut().write_all(b"query return name\n").await.unwrap();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("501:No matches") || line.contains("102:There were"));

    // 2. Guest Delete (should be denied)
    line.clear();
    reader.get_mut().write_all(b"delete name=any\n").await.unwrap();
    reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("401:Authentication required"));
}

#[tokio::test]
async fn test_should_enforce_team_authorization() {
    let _ = tracing_subscriber::fmt::try_init();
    let keys_dir = tempdir().unwrap();
    
    // Create users belonging to different teams
    let devops_user = TestUser::new("devops-user");
    let security_user = TestUser::new("security-user");
    
    std::fs::write(keys_dir.path().join("devops_id_ed25519.pub"), devops_user.pub_key.as_bytes()).unwrap();
    std::fs::write(keys_dir.path().join("security_id_ed25519.pub"), security_user.pub_key.as_bytes()).unwrap();
    
    let (addr, storage) = setup_rbac_server(keys_dir.path()).await;

    // --- SCENARIO: DevOps user creates a record ---
    let devops_stream = TcpStream::connect(addr).await.unwrap();
    let mut devops_reader = BufReader::new(devops_stream);
    let mut line = String::new();
    devops_reader.read_line(&mut line).await.unwrap(); // welcome

    // Login as devops
    devops_reader.get_mut().write_all(b"login devops-user\n").await.unwrap();
    line.clear();
    devops_reader.read_line(&mut line).await.unwrap();
    let challenge = line.trim().trim_start_matches("301:").to_string();
    let sig = devops_user.sign(&challenge);
    
    let auth_cmd = format!("auth \"{}\" \"{}\"\n", devops_user.pub_key, sig);
    devops_reader.get_mut().write_all(auth_cmd.as_bytes()).await.unwrap();
    line.clear();
    devops_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));

    // Add record owned by devops team
    devops_reader.get_mut().write_all(b"add hostname=prod-web-01 type=machine\n").await.unwrap();
    line.clear();
    devops_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));

    // Verify record ownership in storage
    {
        let lock = storage.read().unwrap();
        let records = lock.query(&[(Some("hostname".to_string()), "prod-web-01".to_string())], None);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].owner_team, Some("devops".to_string()));
    }

    // --- SCENARIO: Security user attempts to update DevOps record ---
    let sec_stream = TcpStream::connect(addr).await.unwrap();
    let mut sec_reader = BufReader::new(sec_stream);
    sec_reader.read_line(&mut line).await.unwrap(); // welcome

    // Login as security
    sec_reader.get_mut().write_all(b"login security-user\n").await.unwrap();
    line.clear();
    sec_reader.read_line(&mut line).await.unwrap();
    let challenge = line.trim().trim_start_matches("301:").to_string();
    let sig = security_user.sign(&challenge);
    
    let auth_cmd = format!("auth \"{}\" \"{}\"\n", security_user.pub_key, sig);
    sec_reader.get_mut().write_all(auth_cmd.as_bytes()).await.unwrap();
    line.clear();
    sec_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));

    // Attempt update (should be denied)
    sec_reader.get_mut().write_all(b"add hostname=prod-web-01 status=compromised\n").await.unwrap();
    line.clear();
    sec_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("403:Forbidden: Unauthorized record modification"));

    // --- SCENARIO: DevOps user updates own record ---
    devops_reader.get_mut().write_all(b"add hostname=prod-web-01 status=healthy\n").await.unwrap();
    line.clear();
    devops_reader.read_line(&mut line).await.unwrap();
    assert!(line.contains("200:Ok"));
    
    {
        let lock = storage.read().unwrap();
        let records = lock.query(&[(Some("hostname".to_string()), "prod-web-01".to_string())], None);
        assert_eq!(records[0].fields.get("status").unwrap(), "healthy");
    }
}
