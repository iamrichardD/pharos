/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/tests/middleware_integration.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Integration test to verify that the middleware system correctly intercepts
 * and processes commands in the pharos-server.
 * * Traceability:
 * Related to GitHub Issue #33.
 * ======================================================================== */

use pharos_server::handle_connection;
use pharos_server::storage::{MemoryStorage, Storage};
use pharos_server::auth::{AuthManager, SecurityTier};
use pharos_server::middleware::{MiddlewareChain, ReadOnlyMiddleware, SecurityTierMiddleware};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::{Arc, RwLock};
use tempfile::tempdir;

async fn setup_test_server(middleware_chain: MiddlewareChain) -> (std::net::SocketAddr, Arc<RwLock<dyn Storage>>) {
    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let temp_dir = tempdir().unwrap();
    let auth_manager = Arc::new(AuthManager::new(temp_dir.path(), SecurityTier::Open));
    
    let middleware_chain = Arc::new(middleware_chain);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_storage = Arc::clone(&storage);
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_connection(socket, "127.0.0.1:1234".to_string(), server_storage, auth_manager, middleware_chain).await.unwrap();
    });

    (addr, storage)
}

#[tokio::test]
async fn test_should_allow_query_in_open_tier() {
    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Open }));
    
    let (addr, _) = setup_test_server(chain).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    
    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf).await.unwrap(); // consume welcome

    stream.write_all(b"query return name\n").await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("501:No matches") || response.contains("102:There were"));
}

#[tokio::test]
async fn test_should_block_query_in_protected_tier_without_auth() {
    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Protected }));
    
    let (addr, _) = setup_test_server(chain).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    
    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf).await.unwrap(); // consume welcome

    stream.write_all(b"query return name\n").await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("506:Authentication required"));
}

#[tokio::test]
async fn test_should_block_write_in_scoped_tier_without_admin_role() {
    // Note: We test the middleware logic directly since simulating SSH auth in full integration 
    // requires setting up keys. We'll test the middleware's response to an unauthenticated write
    // in Scoped, which should fail due to no auth first.
    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Scoped }));
    
    let (addr, _) = setup_test_server(chain).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    
    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf).await.unwrap(); // consume welcome

    stream.write_all(b"add name=Test\n").await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("506:Authentication required for Scoped tier"));
}

#[tokio::test]
async fn test_should_block_write_when_guest_id_provided() {
    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let temp_dir = tempdir().unwrap();
    let auth_manager = Arc::new(AuthManager::new(temp_dir.path(), SecurityTier::Open));
    
    let mut middleware_chain = MiddlewareChain::new();
    middleware_chain.add(Arc::new(ReadOnlyMiddleware {
        read_only_ids: vec!["guest".to_string()],
    }));
    middleware_chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Open }));
    let middleware_chain = Arc::new(middleware_chain);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_connection(socket, "127.0.0.1:1234".to_string(), storage, auth_manager, middleware_chain).await.unwrap();
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    
    // Read welcome message
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).await.unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).contains("200:Database ready"));

    // Set ID to guest
    stream.write_all(b"id guest
").await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).contains("200:Ok"));

    // Attempt to Add (should be blocked by ReadOnlyMiddleware even if authenticated)
    // Note: Authenticated check happens AFTER middleware in our current main.rs logic
    // but middleware can short-circuit before it.
    stream.write_all(b"add name=Test
").await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("517:Operation failed because database is read-only"));
}

#[tokio::test]
async fn test_should_allow_write_when_other_id_provided() {
    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let temp_dir = tempdir().unwrap();
    let auth_manager = Arc::new(AuthManager::new(temp_dir.path(), SecurityTier::Open));
    
    let mut middleware_chain = MiddlewareChain::new();
    middleware_chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Open }));
    middleware_chain.add(Arc::new(ReadOnlyMiddleware {
        read_only_ids: vec!["guest".to_string()],
    }));
    let middleware_chain = Arc::new(middleware_chain);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_connection(socket, "127.0.0.1:1234".to_string(), storage, auth_manager, middleware_chain).await.unwrap();
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    
    // Read welcome message
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).await.unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).contains("200:Database ready"));

    // Set ID to admin
    stream.write_all(b"id admin
").await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).contains("200:Ok"));

    // Attempt to Add (should NOT be blocked by ReadOnlyMiddleware, but might be blocked by Auth if we didn't mock it)
    stream.write_all(b"add name=Test
").await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    // It should reach the Auth check and return 506 (not 517)
    assert!(response.contains("506:Authentication required"));
}

#[tokio::test]
async fn test_should_verify_auth_check_command() {
    // 1. Generate a keypair
    use ssh_key::PrivateKey;
    let mut rng = rand::rngs::OsRng;
    let priv_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
    let pub_key_openssh = priv_key.public_key().to_openssh().unwrap();
    
    // 2. Setup server with this authorized key
    let temp_dir = tempdir().unwrap();
    let key_path = temp_dir.path().join("test.pub");
    std::fs::write(&key_path, pub_key_openssh.as_bytes()).unwrap();
    
    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let auth_manager = Arc::new(AuthManager::new(temp_dir.path(), SecurityTier::Open));
    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Open }));
    let middleware_chain = Arc::new(chain);
    
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let server_storage = Arc::clone(&storage);
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_connection(socket, "127.0.0.1:1234".to_string(), server_storage, auth_manager, middleware_chain).await.unwrap();
    });
    
    // 3. Connect and send auth-check
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf).await.unwrap(); // welcome
    
    let challenge = "test-challenge";
    // Sign the challenge
    let sig_bytes = match priv_key.key_data() {
        ssh_key::private::KeypairData::Ed25519(kp) => {
            use ed25519_dalek::{Signer, SigningKey};
            let signing_key = SigningKey::from_bytes(&kp.private.to_bytes());
            signing_key.sign(challenge.as_bytes()).to_vec()
        }
        _ => panic!("Unsupported key type"),
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let sig_b64 = STANDARD.encode(&sig_bytes);
    
    let cmd = format!("auth-check \"{}\" \"{}\" \"{}\"\n", pub_key_openssh, sig_b64, challenge);
    stream.write_all(cmd.as_bytes()).await.unwrap();

    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("200:Ok"));
}

#[tokio::test]
async fn test_should_reject_auth_check_with_invalid_signature() {
    use ssh_key::PrivateKey;
    let mut rng = rand::rngs::OsRng;
    let priv_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
    let pub_key_openssh = priv_key.public_key().to_openssh().unwrap();

    let temp_dir = tempdir().unwrap();
    let key_path = temp_dir.path().join("test.pub");
    std::fs::write(&key_path, pub_key_openssh.as_bytes()).unwrap();

    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let auth_manager = Arc::new(AuthManager::new(temp_dir.path(), SecurityTier::Open));
    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Open }));
    let middleware_chain = Arc::new(chain);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_storage = Arc::clone(&storage);
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_connection(socket, "127.0.0.1:1234".to_string(), server_storage, auth_manager, middleware_chain).await.unwrap();
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf).await.unwrap(); // welcome

    // Sign a DIFFERENT string than the challenge we present - the signature
    // will not verify.
    let sig_bytes = match priv_key.key_data() {
        ssh_key::private::KeypairData::Ed25519(kp) => {
            use ed25519_dalek::{Signer, SigningKey};
            let signing_key = SigningKey::from_bytes(&kp.private.to_bytes());
            signing_key.sign(b"not-the-challenge").to_vec()
        }
        _ => panic!("Unsupported key type"),
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let sig_b64 = STANDARD.encode(&sig_bytes);

    let cmd = format!("auth-check \"{}\" \"{}\" \"test-challenge\"\n", pub_key_openssh, sig_b64);
    stream.write_all(cmd.as_bytes()).await.unwrap();

    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("516:No authorization for request"), "Expected 516: but got: {}", response);
}

#[tokio::test]
async fn test_should_require_admin_role_for_authenticated_non_admin_write_in_scoped_tier() {
    use ssh_key::PrivateKey;
    let mut rng = rand::rngs::OsRng;
    let priv_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
    let pub_key_openssh = priv_key.public_key().to_openssh().unwrap();

    let temp_dir = tempdir().unwrap();
    // Deliberately no "admin" in the filename, so this key gets no admin role.
    let key_path = temp_dir.path().join("regular_id_ed25519.pub");
    std::fs::write(&key_path, pub_key_openssh.as_bytes()).unwrap();

    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let auth_manager = Arc::new(AuthManager::new(temp_dir.path(), SecurityTier::Scoped));
    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Scoped }));
    let middleware_chain = Arc::new(chain);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_storage = Arc::clone(&storage);
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_connection(socket, "127.0.0.1:1234".to_string(), server_storage, auth_manager, middleware_chain).await.unwrap();
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf).await.unwrap(); // welcome

    stream.write_all(b"login regular\n").await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    let challenge = resp.trim().trim_start_matches("301:").to_string();

    let sig_bytes = match priv_key.key_data() {
        ssh_key::private::KeypairData::Ed25519(kp) => {
            use ed25519_dalek::{Signer, SigningKey};
            let signing_key = SigningKey::from_bytes(&kp.private.to_bytes());
            signing_key.sign(challenge.as_bytes()).to_vec()
        }
        _ => panic!("Unsupported key type"),
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let sig_b64 = STANDARD.encode(&sig_bytes);

    let auth_cmd = format!("auth \"{}\" \"{}\"\n", pub_key_openssh, sig_b64);
    stream.write_all(auth_cmd.as_bytes()).await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    assert!(String::from_utf8_lossy(&buf[..n]).contains("200:Ok"));

    // Authenticated, but no admin role - the write must still be blocked.
    stream.write_all(b"add name=Test\n").await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("516:Forbidden: Admin role required"), "Expected 516: but got: {}", response);
}

#[tokio::test]
async fn test_should_return_extension_code_for_recognized_but_undispatched_command() {
    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Open }));

    let (addr, _) = setup_test_server(chain).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();

    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf).await.unwrap(); // consume welcome

    // "siteinfo" is parsed by the protocol layer but has no dispatch arm yet.
    stream.write_all(b"siteinfo\n").await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("597:Command recognized, but not yet implemented"), "Expected 597: but got: {}", response);
}

#[tokio::test]
async fn test_should_allow_valid_auth_check_in_protected_tier() {
    use ssh_key::PrivateKey;
    let mut rng = rand::rngs::OsRng;
    let priv_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
    let pub_key_openssh = priv_key.public_key().to_openssh().unwrap();

    let temp_dir = tempdir().unwrap();
    let key_path = temp_dir.path().join("test.pub");
    std::fs::write(&key_path, pub_key_openssh.as_bytes()).unwrap();

    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let auth_manager = Arc::new(AuthManager::new(temp_dir.path(), SecurityTier::Protected));
    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Protected }));
    let middleware_chain = Arc::new(chain);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_storage = Arc::clone(&storage);
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_connection(socket, "127.0.0.1:1234".to_string(), server_storage, auth_manager, middleware_chain).await.unwrap();
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf).await.unwrap(); // welcome

    let challenge = "test-protected-challenge";
    let sig_bytes = match priv_key.key_data() {
        ssh_key::private::KeypairData::Ed25519(kp) => {
            use ed25519_dalek::{Signer, SigningKey};
            let signing_key = SigningKey::from_bytes(&kp.private.to_bytes());
            signing_key.sign(challenge.as_bytes()).to_vec()
        }
        _ => panic!("Unsupported key type"),
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let sig_b64 = STANDARD.encode(&sig_bytes);

    let cmd = format!("auth-check \"{}\" \"{}\" \"{}\"\n", pub_key_openssh, sig_b64, challenge);
    stream.write_all(cmd.as_bytes()).await.unwrap();

    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("200:Ok"), "Expected 200:Ok for auth-check under Protected tier, got: {}", response);
}

#[tokio::test]
async fn test_should_allow_valid_auth_check_in_scoped_tier() {
    use ssh_key::PrivateKey;
    let mut rng = rand::rngs::OsRng;
    let priv_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
    let pub_key_openssh = priv_key.public_key().to_openssh().unwrap();

    let temp_dir = tempdir().unwrap();
    let key_path = temp_dir.path().join("test.pub");
    std::fs::write(&key_path, pub_key_openssh.as_bytes()).unwrap();

    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let auth_manager = Arc::new(AuthManager::new(temp_dir.path(), SecurityTier::Scoped));
    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Scoped }));
    let middleware_chain = Arc::new(chain);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_storage = Arc::clone(&storage);
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_connection(socket, "127.0.0.1:1234".to_string(), server_storage, auth_manager, middleware_chain).await.unwrap();
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf).await.unwrap(); // welcome

    let challenge = "test-scoped-challenge";
    let sig_bytes = match priv_key.key_data() {
        ssh_key::private::KeypairData::Ed25519(kp) => {
            use ed25519_dalek::{Signer, SigningKey};
            let signing_key = SigningKey::from_bytes(&kp.private.to_bytes());
            signing_key.sign(challenge.as_bytes()).to_vec()
        }
        _ => panic!("Unsupported key type"),
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let sig_b64 = STANDARD.encode(&sig_bytes);

    let cmd = format!("auth-check \"{}\" \"{}\" \"{}\"\n", pub_key_openssh, sig_b64, challenge);
    stream.write_all(cmd.as_bytes()).await.unwrap();

    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("200:Ok"), "Expected 200:Ok for auth-check under Scoped tier, got: {}", response);
}

#[tokio::test]
async fn test_should_reject_invalid_auth_check_in_protected_tier() {
    use ssh_key::PrivateKey;
    let mut rng = rand::rngs::OsRng;
    let priv_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
    let pub_key_openssh = priv_key.public_key().to_openssh().unwrap();

    let temp_dir = tempdir().unwrap();
    let key_path = temp_dir.path().join("test.pub");
    std::fs::write(&key_path, pub_key_openssh.as_bytes()).unwrap();

    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let auth_manager = Arc::new(AuthManager::new(temp_dir.path(), SecurityTier::Protected));
    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Protected }));
    let middleware_chain = Arc::new(chain);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_storage = Arc::clone(&storage);
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_connection(socket, "127.0.0.1:1234".to_string(), server_storage, auth_manager, middleware_chain).await.unwrap();
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf).await.unwrap(); // welcome

    // Sign a DIFFERENT string than the challenge presented
    let sig_bytes = match priv_key.key_data() {
        ssh_key::private::KeypairData::Ed25519(kp) => {
            use ed25519_dalek::{Signer, SigningKey};
            let signing_key = SigningKey::from_bytes(&kp.private.to_bytes());
            signing_key.sign(b"wrong-challenge").to_vec()
        }
        _ => panic!("Unsupported key type"),
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let sig_b64 = STANDARD.encode(&sig_bytes);

    let cmd = format!("auth-check \"{}\" \"{}\" \"test-challenge\"\n", pub_key_openssh, sig_b64);
    stream.write_all(cmd.as_bytes()).await.unwrap();

    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("516:No authorization for request"), "Expected 516: but got: {}", response);
}
