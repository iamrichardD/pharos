/* ========================================================================
 * Project: pharos
 * Component: Server Core Tests
 * File: pharos-server/tests/flush_on_large_query_response.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Regression test for a real production hang: `mdb '*'` and `mdb type=machine`
 * (multi-record, full/unprojected-field query output) would hang forever,
 * while single-record full output and multi-record single-field-projected
 * output both worked. Root cause: handle_connection's TLS write-half never
 * called .flush() anywhere - write_all() only queues plaintext into rustls'
 * internal buffer, and for a response large enough to hit backpressure
 * partway through, the tail silently never reaches the socket once the loop
 * moves on to await the next read. Confirmed live against real production
 * data (pharos-01.iamrichardd.com's data.json) via the compose harness in
 * scripts/live-verify/: server-side tracing showed the full response loop
 * completing and writing "200:Ok" every time, while the client consistently
 * stalled reading at the same byte offset regardless of timeout length.
 *
 * This only reproduces over an actual TLS-wrapped stream - a plain TcpStream
 * (as most other integration tests in this file use) doesn't have rustls'
 * internal write-buffering behavior, so it can't exercise or guard against
 * this bug. Hence the local TLS acceptor/connector setup below, using rcgen
 * for a self-signed cert instead of shelling out to openssl (unlike
 * scripts/gen-sandbox-certs.sh) so this test doesn't depend on the host or
 * CI container having an openssl CLI binary installed.
 * ======================================================================== */

use pharos_server::handle_connection;
use pharos_server::storage::{MemoryStorage, Storage};
use pharos_server::auth::{AuthManager, SecurityTier};
use pharos_server::middleware::{MiddlewareChain, SecurityTierMiddleware};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tempfile::tempdir;
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tokio_rustls::rustls::{ServerConfig, ClientConfig, RootCertStore};
use tokio_rustls::rustls::pki_types::{ServerName, PrivatePkcs8KeyDer, PrivateKeyDer};

fn build_test_tls() -> (TlsAcceptor, TlsConnector) {
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();

    let rcgen::CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_der = cert.der().clone();
    let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pair.serialize_der()));

    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der.clone()], key_der)
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(server_config));

    let mut roots = RootCertStore::empty();
    roots.add(cert_der).unwrap();
    let client_config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(client_config));

    (acceptor, connector)
}

/// Seeds `count` synthetic machine records shaped like real production
/// inventory (multiple scalar fields plus multi-valued ip_addr/mac_addr),
/// matching what pharos-01.iamrichardd.com's real data.json actually looks
/// like - the shape that triggered the original hang.
fn seed_machine_records(storage: &mut MemoryStorage, count: usize) {
    for i in 0..count {
        let fields = vec![
            ("type".to_string(), "machine".to_string()),
            ("hostname".to_string(), format!("host-{i:03}")),
            ("cpu_brand".to_string(), "AMD Ryzen 5 5500U with Radeon Graphics".to_string()),
            ("cpu_cores".to_string(), "8".to_string()),
            ("mem_total_kb".to_string(), "524288".to_string()),
            ("os_name".to_string(), "Debian GNU/Linux".to_string()),
            ("os_version".to_string(), "13".to_string()),
            ("kernel_version".to_string(), "7.0.14-5-pve".to_string()),
            ("manufacturer".to_string(), "AZW".to_string()),
            ("product_name".to_string(), "SER".to_string()),
            ("status".to_string(), "online".to_string()),
            ("created_at".to_string(), "2026-08-04T23:13:06.622776083+00:00".to_string()),
            ("last_seen_at".to_string(), "2026-08-07T15:31:27.625731049+00:00".to_string()),
            ("ip_addr".to_string(), format!("192.168.86.{i}")),
            ("ip_addr".to_string(), format!("fe80::be24:11ff:fe00:{i:x}")),
            ("mac_addr".to_string(), format!("bc:24:11:00:{i:02x}:07")),
            ("mac_addr".to_string(), format!("bc:24:11:00:{i:02x}:08")),
        ];
        storage.add_record(fields, None, None).unwrap();
    }
}

async fn setup_tls_server(record_count: usize, acceptor: TlsAcceptor) -> std::net::SocketAddr {
    let dir = tempdir().unwrap();
    let mut storage = MemoryStorage::new();
    seed_machine_records(&mut storage, record_count);
    let storage: Arc<RwLock<dyn Storage>> = Arc::new(RwLock::new(storage));
    let auth_manager = Arc::new(AuthManager::new(dir.path(), SecurityTier::Open));

    let mut chain = MiddlewareChain::new();
    chain.add(Arc::new(SecurityTierMiddleware { default_tier: SecurityTier::Open }));
    let middleware_chain = Arc::new(chain);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            if let Ok((socket, peer_addr)) = listener.accept().await {
                let s = Arc::clone(&storage);
                let a = Arc::clone(&auth_manager);
                let m = Arc::clone(&middleware_chain);
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    match acceptor.accept(socket).await {
                        Ok(tls_stream) => {
                            let _ = handle_connection(tls_stream, peer_addr.to_string(), s, a, m).await;
                        }
                        Err(e) => eprintln!("TLS accept failed in test server: {e}"),
                    }
                });
            }
        }
    });

    addr
}

#[tokio::test]
async fn test_should_not_hang_on_multi_record_full_field_query_over_tls() {
    // 30 records x ~17 fields each comfortably exceeds the ~10-record/~150-line
    // payload that hung against real production data - a solid margin so this
    // test doesn't become a borderline/flaky proxy for an exact buffer size.
    let (acceptor, connector) = build_test_tls();
    let addr = setup_tls_server(30, acceptor).await;

    let tcp = TcpStream::connect(addr).await.unwrap();
    let server_name = ServerName::try_from("localhost").unwrap();
    let tls_stream = connector.connect(server_name, tcp).await.unwrap();
    let (reader, mut writer) = tokio::io::split(tls_stream);
    let mut reader = BufReader::new(reader);

    let mut welcome = String::new();
    reader.read_line(&mut welcome).await.unwrap();
    assert!(welcome.starts_with("200:Database ready"), "Unexpected banner: {welcome}");

    // No `return <field>` clause - full/unprojected output, the case that hung.
    writer.write_all(b"query *\n").await.unwrap();

    let read_all_lines = async {
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).await.unwrap();
            if bytes_read == 0 {
                break;
            }
            let trimmed = line.trim_end_matches(['\n', '\r']).to_string();
            let is_terminal = trimmed == "200:Ok" || trimmed.starts_with("500:") || trimmed.starts_with("501:");
            lines.push(trimmed);
            if is_terminal {
                break;
            }
        }
        lines
    };

    let lines = tokio::time::timeout(Duration::from_secs(10), read_all_lines)
        .await
        .expect(
            "Query response was not fully received within 10s - the server likely wrote a \
             response without flushing it (see handle_connection's TLS write-half), leaving \
             the client blocked reading bytes that never left the server's write buffer.",
        );

    assert_eq!(lines.last().map(String::as_str), Some("200:Ok"), "Response did not terminate with 200:Ok: {lines:?}");
    assert!(lines[0].starts_with("102:There were 30 matches"), "Unexpected match-count line: {}", lines[0]);

    for i in 0..30 {
        let expected = format!("hostname: host-{i:03}");
        assert!(lines.iter().any(|l| l.contains(&expected)), "Missing record for host-{i:03} in response");
    }
    // Continuation lines for the second ip_addr/mac_addr value use blank
    // padding instead of repeating the field name - confirm at least one
    // survived intact through the TLS write path too, not just the "200:Ok"
    // terminator.
    assert!(lines.iter().any(|l| l.contains(":       : fe80::be24:11ff:fe00:0")), "Missing ip_addr continuation line");
}

#[tokio::test]
async fn test_should_not_hang_on_type_machine_query_over_tls() {
    let (acceptor, connector) = build_test_tls();
    let addr = setup_tls_server(20, acceptor).await;

    let tcp = TcpStream::connect(addr).await.unwrap();
    let server_name = ServerName::try_from("localhost").unwrap();
    let tls_stream = connector.connect(server_name, tcp).await.unwrap();
    let (reader, mut writer) = tokio::io::split(tls_stream);
    let mut reader = BufReader::new(reader);

    let mut welcome = String::new();
    reader.read_line(&mut welcome).await.unwrap();

    writer.write_all(b"query type=machine\n").await.unwrap();

    let read_all_lines = async {
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).await.unwrap();
            if bytes_read == 0 {
                break;
            }
            let trimmed = line.trim_end_matches(['\n', '\r']).to_string();
            let is_terminal = trimmed == "200:Ok" || trimmed.starts_with("500:") || trimmed.starts_with("501:");
            lines.push(trimmed);
            if is_terminal {
                break;
            }
        }
        lines
    };

    let lines = tokio::time::timeout(Duration::from_secs(10), read_all_lines)
        .await
        .expect("query type=machine hung - full-field multi-record responses must be flushed");

    assert_eq!(lines.last().map(String::as_str), Some("200:Ok"));
    assert!(lines[0].starts_with("102:There were 20 matches"), "Unexpected match-count line: {}", lines[0]);
}
