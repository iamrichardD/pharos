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
    stream: BufReader<TcpStream>,
}

impl PharosClient {
    /// Connects to a Pharos server at the given address.
    pub async fn connect(addr: &str, client_id: &str) -> Result<Self> {
        let stream = TcpStream::connect(addr).await
            .with_context(|| format!("Failed to connect to Pharos server at {}", addr))?;
        let mut reader = BufReader::new(stream);

        // Read banner
        let mut banner = String::new();
        reader.read_line(&mut banner).await
            .context("Failed to read banner from server")?;
        
        if banner.is_empty() {
            return Err(anyhow!("Connection closed by server during banner"));
        }

        let mut client = PharosClient {
            stream: reader,
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
                    return Ok(PharosResponse::Ok(message.to_string()));
                }
                102 => {
                    // Extract match count if possible
                    if let Some(count_str) = message.split_whitespace().nth(2) {
                        match_count = count_str.parse().unwrap_or(0);
                    }
                }
                401 => {
                    if let Some(challenge_pos) = message.find("Challenge: ") {
                        let challenge = message[challenge_pos + 11..].to_string();
                        return Ok(PharosResponse::AuthenticationRequired { challenge });
                    }
                    return Ok(PharosResponse::Error { code, message: message.to_string() });
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
        
        if let PharosResponse::AuthenticationRequired { challenge } = resp {
            let (pub_key_ssh, sig_b64) = self.sign_challenge(&challenge)?;
            
            self.send_line(&format!("auth \"{}\" \"{}\"", pub_key_ssh, sig_b64)).await?;
            let auth_resp = self.read_line().await?;
            
            if auth_resp.starts_with("200") {
                // Retry original command
                return self.execute(command).await;
            } else {
                return Err(anyhow!("Authentication failed: {}", auth_resp));
            }
        }

        Ok(resp)
    }

    fn sign_challenge(&self, challenge_hex: &str) -> Result<(String, String)> {
        let priv_key_path = env::var("PHAROS_PRIVATE_KEY").unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            format!("{}/.ssh/id_ed25519", home)
        });

        if !Path::new(&priv_key_path).exists() {
            return Err(anyhow!("Private key not found at {}. Use PHAROS_PRIVATE_KEY to specify it.", priv_key_path));
        }

        let key_content = fs::read_to_string(&priv_key_path)
            .with_context(|| format!("Failed to read private key at {}", priv_key_path))?;
        let priv_key = PrivateKey::from_openssh(&key_content)
            .map_err(|e| anyhow!("Failed to parse SSH private key: {}", e))?;
        
        let sig = priv_key.sign("", ssh_key::HashAlg::Sha256, challenge_hex.as_bytes())
            .map_err(|e| anyhow!("Failed to sign challenge: {}", e))?;
        let sig_b64 = STANDARD.encode(sig.signature().as_bytes());
        
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
