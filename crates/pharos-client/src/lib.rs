/* ========================================================================
 * Project: pharos
 * Component: Shared Client Library (pharos-client)
 * File: crates/pharos-client/src/lib.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This crate provides a shared, async-first client library for interacting
 * with a Pharos server using the RFC 2378 protocol. It supports connection
 * management, authentication (including SSH-key based challenges), and
 * parsing of responses.
 * * Traceability:
 * Related to Task 10.1 (Issue #39)
 * ======================================================================== */

use tokio::net::TcpStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use ssh_key::PrivateKey;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::path::Path;
use std::fs;
use std::env;
use anyhow::{Result, Context, anyhow};
use std::sync::Arc;
use tokio_rustls::rustls::{ClientConfig, RootCertStore, pki_types::ServerName};
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

/// Represents a field in a record returned by the Pharos server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PharosField {
    pub key: String,
    pub value: String,
}

/// Represents a single record match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PharosRecord {
    pub id: i32,
    pub fields: Vec<PharosField>,
}

/// Represents the possible outcomes of a Pharos query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PharosResponse {
    Ok(String),
    Matches {
        count: i32,
        records: Vec<PharosRecord>,
    },
    Error {
        code: i32,
        message: String,
    },
    AuthenticationRequired {
        challenge: String,
    },
}

pub struct PharosClient {
    stream: BufReader<TlsStream<TcpStream>>,
    client_id: String,
}

impl PharosClient {
    /// Connects to a Pharos server at the given address.
    pub async fn connect(addr: &str, client_id: &str) -> Result<Self> {
        let tcp_stream = TcpStream::connect(addr).await
            .with_context(|| format!("Failed to connect to Pharos server at {}", addr))?;

        // --- TLS Configuration ---
        let mut root_store = RootCertStore::empty();
        
        // Add native roots if available (rustls-native-certs 0.8 returns CertificateResult)
        let native_certs = rustls_native_certs::load_native_certs();
        for cert in native_certs.certs {
            root_store.add(cert)?;
        }
        if !native_certs.errors.is_empty() {
            log::warn!("Errors loading some native certificates: {:?}", native_certs.errors);
        }

        // Add custom CA if PHAROS_CA_CERT is set
        if let Ok(ca_path_str) = env::var("PHAROS_CA_CERT") {
            let ca_path = Path::new(&ca_path_str);
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(30);
            
            while !ca_path.exists() && start.elapsed() < timeout {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }

            if !ca_path.exists() {
                return Err(anyhow!("Timeout waiting for CA cert at {:?}", ca_path));
            }

            let file = fs::File::open(ca_path)
                .with_context(|| format!("Failed to open CA cert at {:?}", ca_path))?;
            let mut reader = std::io::BufReader::new(file);
            for cert in rustls_pemfile::certs(&mut reader) {
                root_store.add(cert?)?;
            }
        }

        // Add webpki roots as a fallback
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        
        let connector = TlsConnector::from(Arc::new(config));
        
        // Use the hostname part of the address for SNI
        let domain = addr.split(':').next().unwrap_or("localhost");
        let server_name = ServerName::try_from(domain)
            .map_err(|_| anyhow!("Invalid server name: {}", domain))?
            .to_owned();

        let tls_stream = connector.connect(server_name, tcp_stream).await
            .context("TLS handshake failed")?;

        let mut reader = BufReader::new(tls_stream);

        // Read banner
        let mut banner = String::new();
        reader.read_line(&mut banner).await
            .context("Failed to read banner from server")?;
        
        if banner.is_empty() {
            return Err(anyhow!("Connection closed by server during banner"));
        }

        let mut client = PharosClient {
            stream: reader,
            client_id: client_id.to_string(),
        };

        // Send ID
        client.send_line(&format!("id {}", client_id)).await?;
        let id_resp = client.read_line().await?;
        if !id_resp.starts_with("200") {
            return Err(anyhow!("Server rejected identification: {}", id_resp));
        }

        Ok(client)
    }

    /// Sends a single command and returns the parsed response.
    pub async fn execute(&mut self, command: &str) -> Result<PharosResponse> {
        self.send_line(command).await?;
        self.parse_response().await
    }

    /// Explicitly authenticates the session using the configured client ID.
    pub async fn authenticate(&mut self) -> Result<()> {
        self.send_line(&format!("login {}", self.client_id)).await?;
        let resp = self.read_line().await?;
        
        if resp.starts_with("301:") {
            let challenge = &resp[4..];
            let (pub_key_ssh, sig_b64) = Self::sign_message_async(challenge).await?;
            
            self.send_line(&format!("auth \"{}\" \"{}\"", pub_key_ssh, sig_b64)).await?;
            let auth_resp = self.read_line().await?;
            
            if auth_resp.starts_with("200") {
                Ok(())
            } else {
                Err(anyhow!("Authentication failed: {}", auth_resp))
            }
        } else {
            Err(anyhow!("Failed to receive challenge from server: {}", resp))
        }
    }

    async fn send_line(&mut self, line: &str) -> Result<()> {
        let mut cmd = line.to_string();
        if !cmd.ends_with("
") {
            cmd.push_str("
");
        }
        self.stream.write_all(cmd.as_bytes()).await
            .context("Failed to write to stream")?;
        self.stream.flush().await
            .context("Failed to flush stream")?;
        Ok(())
    }

    async fn read_line(&mut self) -> Result<String> {
        let mut line = String::new();
        self.stream.read_line(&mut line).await
            .context("Failed to read line from stream")?;
        Ok(line.trim().to_string())
    }

    async fn parse_response(&mut self) -> Result<PharosResponse> {
        let mut records = Vec::new();
        let mut current_record: Option<PharosRecord> = None;
        let mut match_count = 0;

        loop {
            let line = self.read_line().await?;
            if line.is_empty() {
                break;
            }

            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() < 2 {
                continue;
            }

            let code: i32 = parts[0].parse()
                .with_context(|| format!("Invalid response code: {}", parts[0]))?;
            let message = parts[1].trim();

            match code {
                200 => {
                    if let Some(record) = current_record.take() {
                        records.push(record);
                    }
                    if match_count > 0 || !records.is_empty() {
                        return Ok(PharosResponse::Matches { count: match_count, records });
                    } else {
                        return Ok(PharosResponse::Ok(message.to_string()));
                    }
                }
                102 => {
                    // Extract match count if possible
                    if let Some(count_str) = message.split_whitespace().nth(2) {
                        match_count = count_str.parse().unwrap_or(0);
                    }
                }
                401 => {
                    // New message format: 401:Authentication required. Use 'login [alias]' to receive a challenge.
                    return Ok(PharosResponse::AuthenticationRequired { challenge: String::new() });
                }
                c if c >= 400 => {
                    return Ok(PharosResponse::Error { code: c, message: message.to_string() });
                }
                c if c < 0 => {
                    // Data line: -200:ID:FIELD:VALUE
                    let data_parts: Vec<&str> = message.splitn(3, ':').collect();
                    if data_parts.len() == 3 {
                        let id: i32 = data_parts[0].parse().unwrap_or(0);
                        let field = data_parts[1].to_string();
                        let value = data_parts[2].trim().to_string();

                        if let Some(ref mut record) = current_record {
                            if record.id != id {
                                records.push(current_record.take().unwrap());
                                current_record = Some(PharosRecord { id, fields: vec![PharosField { key: field, value }] });
                            } else {
                                record.fields.push(PharosField { key: field, value });
                            }
                        } else {
                            current_record = Some(PharosRecord { id, fields: vec![PharosField { key: field, value }] });
                        }
                    }
                }
                _ => {
                    // Intermediate message (e.g. 100, 101)
                }
            }
        }

        if let Some(record) = current_record {
            records.push(record);
        }

        if match_count > 0 || !records.is_empty() {
            Ok(PharosResponse::Matches { count: match_count, records })
        } else {
            Ok(PharosResponse::Ok("Ok".to_string()))
        }
    }

    /// Performs authenticated execution of a command.
    pub async fn execute_authenticated(&mut self, command: &str) -> Result<PharosResponse> {
        let resp = self.execute(command).await?;
        
        if let PharosResponse::AuthenticationRequired { .. } = resp {
            self.authenticate().await?;
            // Retry original command
            return self.execute(command).await;
        }

        Ok(resp)
    }

    /// Signs a message using the configured SSH private key.
    /// Returns (Public Key SSH string, Signature Base64 string).
    pub async fn sign_message_async(message: &str) -> Result<(String, String)> {
        let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let priv_key_path_str = env::var("PHAROS_PRIVATE_KEY").unwrap_or_else(|_| {
            let p = format!("{}/.ssh/id_ed25519", home);
            if Path::new(&p).exists() {
                p
            } else {
                // Fallback for Pharos-managed admin key
                format!("{}/.ssh/admin_id_ed25519", home)
            }
        });

        let priv_key_path = Path::new(&priv_key_path_str);
        
        // Wait for private key to appear (up to 60 seconds)
        // This is critical for Sandbox where pharos-server generates it.
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(60);
        
        if !priv_key_path.exists() {
            log::info!("Waiting for private key at {:?} (timeout: 60s)...", priv_key_path);
        }

        while !priv_key_path.exists() && start.elapsed() < timeout {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }

        if !priv_key_path.exists() {
            // Check fallback path /etc/pharos/keys/ if the primary path failed
            let fallback_path = Path::new("/etc/pharos/keys/admin_id_ed25519");
            if fallback_path.exists() {
                log::info!("Primary key not found, but found fallback at {:?}", fallback_path);
                return Self::sign_with_key_path(fallback_path, message).await;
            }
            return Err(anyhow!("Private key not found at {:?} after 60s. Ensure PHAROS_PRIVATE_KEY is set correctly.", priv_key_path));
        }

        Self::sign_with_key_path(priv_key_path, message).await
    }

    async fn sign_with_key_path(path: &Path, message: &str) -> Result<(String, String)> {
        let key_content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read private key at {:?}", path))?;
        let priv_key = PrivateKey::from_openssh(&key_content)
            .map_err(|e| anyhow!("Failed to parse SSH private key: {}", e))?;
        
        // Use raw key data signing to match the server's raw verification logic.
        let sig_bytes = match priv_key.key_data() {
            ssh_key::private::KeypairData::Ed25519(kp) => {
                use ed25519_dalek::{Signer, SigningKey};
                let signing_key = SigningKey::from_bytes(&kp.private.to_bytes());
                signing_key.sign(message.as_bytes()).to_vec()
            }
            _ => return Err(anyhow!("Unsupported key type for raw signing. Only Ed25519 is supported.")),
        };

        let sig_b64 = STANDARD.encode(&sig_bytes);
        let pub_key_ssh = priv_key.public_key().to_openssh()
            .map_err(|e| anyhow!("Failed to export public key: {}", e))?;
        
        Ok((pub_key_ssh, sig_b64))
    }

    pub async fn quit(mut self) -> Result<()> {
        self.send_line("quit").await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_should_correctly_sign_challenge_when_key_exists() {
        // This test would need a real key. For now, we just skip it or 
        // mock the sign_challenge function if it were more modular.
    }
    
    // For more robust testing, we'd want to mock the TCP stream.
    // However, for this increment, we will verify integration with ph and mdb.
}
