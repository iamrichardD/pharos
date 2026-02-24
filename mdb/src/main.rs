/* ========================================================================
 * Project: pharos
 * Component: CLI-mdb
 * File: mdb/src/main.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This is the entry point for the 'mdb' CLI client, used for machine/infrastructure
 * assets using the RFC 2378 protocol. It separates IO from logic for testability.
 * * Traceability:
 * Related to Task 3.2 (Issue #12)
 * ======================================================================== */

use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process;

/// trait to abstract IO operations for the client to enable mocking
pub trait MdbIo {
    fn connect(&mut self, addr: &str) -> Result<(), String>;
    fn write_line(&mut self, line: &str) -> Result<(), String>;
    fn read_line(&mut self) -> Result<String, String>;
}

struct RealIo {
    stream: Option<TcpStream>,
    reader: Option<BufReader<TcpStream>>,
}

impl RealIo {
    fn new() -> Self {
        RealIo {
            stream: None,
            reader: None,
        }
    }
}

impl MdbIo for RealIo {
    fn connect(&mut self, addr: &str) -> Result<(), String> {
        match TcpStream::connect(addr) {
            Ok(s) => {
                self.stream = Some(s.try_clone().map_err(|e| e.to_string())?);
                self.reader = Some(BufReader::new(s));
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    fn write_line(&mut self, line: &str) -> Result<(), String> {
        if let Some(stream) = &mut self.stream {
            write!(stream, "{}\r\n", line).map_err(|e| e.to_string())
        } else {
            Err("Not connected".to_string())
        }
    }

    fn read_line(&mut self) -> Result<String, String> {
        if let Some(reader) = &mut self.reader {
            let mut buf = String::new();
            match reader.read_line(&mut buf) {
                Ok(0) => Ok("".to_string()), // EOF
                Ok(_) => Ok(buf),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Not connected".to_string())
        }
    }
}

use ssh_key::PrivateKey;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::path::Path;
use std::fs;

pub fn run_client<T: MdbIo>(io: &mut T, addr: &str, command_line: &str) -> Result<Vec<String>, String> {
    io.connect(addr)?;
    
    // Read banner
    let banner = io.read_line()?;
    if banner.is_empty() {
        return Err("Connection closed by server".to_string());
    }

    // Send ID
    io.write_line("id mdb")?;
    let _id_resp = io.read_line()?;

    // If it's not a query/ph command, it might be a write command
    let lower_cmd = command_line.to_lowercase();
    let is_query = lower_cmd.starts_with("query ") || lower_cmd.starts_with("ph ");
    
    let cmd_to_send = if is_query {
        command_line.to_string()
    } else {
        // If it's a known write command, use as is, else default to query
        let first_word = lower_cmd.split_whitespace().next().unwrap_or("");
        match first_word {
            "add" | "change" | "delete" | "status" | "siteinfo" | "quit" => command_line.to_string(),
            _ => format!("query {}", command_line),
        }
    };

    io.write_line(&cmd_to_send)?;

    let mut output = Vec::new();
    loop {
        let line = io.read_line()?;
        if line.is_empty() {
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
        if parts.len() < 2 {
            output.push(trimmed.to_string());
            continue;
        }

        if let Ok(code) = parts[0].parse::<i32>() {
            if code == 401 {
                // Authentication required
                // Expected format: 401:Authentication required. Challenge: <hex>
                if let Some(challenge_pos) = trimmed.find("Challenge: ") {
                    let challenge_hex = &trimmed[challenge_pos + 11..];
                    
                    // Sign challenge
                    let (pub_key_ssh, sig_b64) = sign_challenge(challenge_hex)?;
                    
                    // Send Auth command
                    io.write_line(&format!("auth \"{}\" \"{}\"", pub_key_ssh, sig_b64))?;
                    let auth_resp = io.read_line()?;
                    if auth_resp.starts_with("200") {
                        // Retry original command
                        io.write_line(&cmd_to_send)?;
                        continue;
                    } else {
                        output.push(auth_resp);
                        break;
                    }
                }
            }

            if code >= 200 {
                if code != 200 {
                    output.push(trimmed.to_string());
                }
                break;
            } else if code < 0 {
                let data_parts: Vec<&str> = parts[1].splitn(3, ':').collect();
                if data_parts.len() == 3 {
                    let field = data_parts[1];
                    let val = data_parts[2];
                    output.push(format!("{:>15}: {}", field, val.trim()));
                } else {
                    output.push(trimmed.to_string());
                }
            } else {
                output.push(parts[1].to_string());
            }
        } else {
            output.push(trimmed.to_string());
        }
    }

    let _ = io.write_line("quit");

    Ok(output)
}

fn sign_challenge(challenge_hex: &str) -> Result<(String, String), String> {
    let priv_key_path = env::var("PHAROS_PRIVATE_KEY").unwrap_or_else(|_| {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{}/.ssh/id_ed25519", home)
    });

    if !Path::new(&priv_key_path).exists() {
        return Err(format!("Private key not found at {}. Use PHAROS_PRIVATE_KEY to specify it.", priv_key_path));
    }

    let key_content = fs::read_to_string(&priv_key_path).map_err(|e| e.to_string())?;
    let priv_key = PrivateKey::from_openssh(&key_content).map_err(|e| e.to_string())?;
    
    let sig = priv_key.sign("", ssh_key::HashAlg::Sha256, challenge_hex.as_bytes()).map_err(|e| e.to_string())?;
    let sig_b64 = STANDARD.encode(sig.signature().as_bytes());
    
    let pub_key_ssh = priv_key.public_key().to_openssh().map_err(|e| e.to_string())?;
    
    Ok((pub_key_ssh, sig_b64))
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: mdb <query>");
        process::exit(1);
    }

    let query_string = args.join(" ");
    
    let host = env::var("PHAROS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PHAROS_PORT").unwrap_or_else(|_| "1050".to_string());
    let addr = format!("{}:{}", host, port);

    let mut io = RealIo::new();
    match run_client(&mut io, &addr, &query_string) {
        Ok(output) => {
            for line in output {
                println!("{}", line);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockIo {
        connect_result: Result<(), String>,
        read_responses: Vec<String>,
        written_lines: Vec<String>,
    }

    impl MockIo {
        fn new(connect_result: Result<(), String>, read_responses: Vec<&str>) -> Self {
            let mut read_responses: Vec<String> = read_responses.iter().map(|s| s.to_string()).collect();
            read_responses.reverse(); // so we can pop
            MockIo {
                connect_result,
                read_responses,
                written_lines: Vec::new(),
            }
        }
    }

    impl MdbIo for MockIo {
        fn connect(&mut self, _addr: &str) -> Result<(), String> {
            self.connect_result.clone()
        }

        fn write_line(&mut self, line: &str) -> Result<(), String> {
            self.written_lines.push(line.to_string());
            Ok(())
        }

        fn read_line(&mut self) -> Result<String, String> {
            if let Some(resp) = self.read_responses.pop() {
                Ok(resp)
            } else {
                Ok("".to_string())
            }
        }
    }

    #[test]
    fn test_should_return_formatted_output_when_query_successful() {
        let mut mock = MockIo::new(
            Ok(()),
            vec![
                "200:Database ready\r\n",
                "200:Ok\r\n", // id mdb response
                "102:There were 1 matches to your request.\r\n",
                "-200:1:hostname: srv01\r\n",
                "-200:1:ip: 192.168.1.10\r\n",
                "200:Ok\r\n"
            ]
        );

        let result = run_client(&mut mock, "127.0.0.1:1050", "hostname=srv01").unwrap();
        
        assert_eq!(mock.written_lines[0], "id mdb");
        assert_eq!(mock.written_lines[1], "query hostname=srv01");
        assert_eq!(mock.written_lines[2], "quit");
        
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "There were 1 matches to your request.");
        assert_eq!(result[1], "       hostname: srv01");
        assert_eq!(result[2], "             ip: 192.168.1.10");
    }

    #[test]
    fn test_should_return_error_when_connection_fails() {
        let mut mock = MockIo::new(
            Err("Connection refused".to_string()),
            vec![]
        );

        let result = run_client(&mut mock, "127.0.0.1:1050", "hostname=srv01");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Connection refused");
    }

    #[test]
    fn test_should_return_empty_when_no_matches() {
        let mut mock = MockIo::new(
            Ok(()),
            vec![
                "200:Database ready\r\n",
                "200:Ok\r\n", // id mdb response
                "501:No matches to query\r\n",
            ]
        );

        let result = run_client(&mut mock, "127.0.0.1:1050", "hostname=unknown").unwrap();
        
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "501:No matches to query");
    }
}