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
    let auth_manager = Arc::new(AuthManager::new(temp_dir.path()));
    
    let middleware_chain = Arc::new(middleware_chain);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_storage = Arc::clone(&storage);
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_connection(socket, server_storage, auth_manager, middleware_chain).await.unwrap();
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
    stream.read(&mut buf).await.unwrap(); // consume welcome

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
    stream.read(&mut buf).await.unwrap(); // consume welcome

    stream.write_all(b"query return name\n").await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("401:Authentication required"));
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
    stream.read(&mut buf).await.unwrap(); // consume welcome

    stream.write_all(b"add name=Test\n").await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(response.contains("401:Authentication required for Scoped tier"));
}

#[tokio::test]
async fn test_should_block_write_when_guest_id_provided() {
    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let temp_dir = tempdir().unwrap();
    let auth_manager = Arc::new(AuthManager::new(temp_dir.path()));
    
    let mut middleware_chain = MiddlewareChain::new();
    middleware_chain.add(Arc::new(ReadOnlyMiddleware {
        read_only_ids: vec!["guest".to_string()],
    }));
    let middleware_chain = Arc::new(middleware_chain);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_connection(socket, storage, auth_manager, middleware_chain).await.unwrap();
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
    assert!(response.contains("500:Read-only access"));
}

#[tokio::test]
async fn test_should_allow_write_when_other_id_provided() {
    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(MemoryStorage::new()));
    let temp_dir = tempdir().unwrap();
    let auth_manager = Arc::new(AuthManager::new(temp_dir.path()));
    
    let mut middleware_chain = MiddlewareChain::new();
    middleware_chain.add(Arc::new(ReadOnlyMiddleware {
        read_only_ids: vec!["guest".to_string()],
    }));
    let middleware_chain = Arc::new(middleware_chain);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        handle_connection(socket, storage, auth_manager, middleware_chain).await.unwrap();
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
    // It should reach the Auth check and return 401 (not 500)
    assert!(response.contains("401:Authentication required"));
}
