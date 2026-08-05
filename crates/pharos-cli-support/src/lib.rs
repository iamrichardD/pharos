/* ========================================================================
 * Project: pharos
 * Component: Shared CLI Support
 * File: crates/pharos-cli-support/src/lib.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Shared interactive key-setup helper functions, SSH enrollment, and error
 * handling logic used by CLI applications (mdb and ph).
 * * Traceability:
 * Issue #185 refactoring and deduplication plan.
 * ======================================================================== */

use anyhow::{Context, Result};
use pharos_client::{PharosClient, PharosResponse};
use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn is_missing_key_error(err: &anyhow::Error) -> bool {
    format!("{:#}", err).contains("No private key found for signing")
}

pub fn is_auth_failure_error(err: &anyhow::Error) -> bool {
    let msg = format!("{:#}", err);
    msg.contains("Authentication failed") || msg.contains("401") || msg.contains("403") || msg.contains("AuthenticationRequired")
}

pub fn default_personal_key_path() -> Result<PathBuf> {
    let home = env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home).join(".ssh").join("id_ed25519"))
}

pub fn generate_local_ed25519_key(key_path: &Path) -> Result<()> {
    if key_path.exists() {
        anyhow::bail!("Key file already exists at {:?}", key_path);
    }
    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {:?}", parent))?;
    }
    let output = Command::new("ssh-keygen")
        .args([
            "-t", "ed25519",
            "-N", "",
            "-f", key_path.to_str().context("Invalid key path")?,
        ])
        .output()
        .context("Failed to execute ssh-keygen")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ssh-keygen failed: {}", stderr.trim());
    }
    Ok(())
}

pub fn offer_to_generate_key(key_path: &Path) -> Result<bool> {
    if key_path.exists() {
        return Ok(false);
    }
    eprintln!("No signing key found for this identity.");
    eprint!("Generate a new personal key now at ~/.ssh/id_ed25519? [Y/n] ");
    io::stderr().flush().ok();
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if answer.trim().eq_ignore_ascii_case("n") {
        return Ok(false);
    }
    generate_local_ed25519_key(key_path)?;
    eprintln!("Generated. You'll still need to enroll the .pub file on your hub before this works - see below.");
    Ok(true)
}

pub fn is_valid_ssh_target(target: &str) -> bool {
    if target.is_empty() || target.starts_with('-') {
        return false;
    }
    let (user, host) = match target.split_once('@') {
        Some((u, h)) => (Some(u), h),
        None => (None, target),
    };
    if let Some(u) = user {
        if u.is_empty() {
            return false;
        }
        let first = match u.chars().next() {
            Some(c) => c,
            None => return false,
        };
        if !first.is_ascii_alphanumeric() {
            return false;
        }
        if !u.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-') {
            return false;
        }
    }
    if host.is_empty() {
        return false;
    }
    let first = match host.chars().next() {
        Some(c) => c,
        None => return false,
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    if !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
        return false;
    }
    true
}

pub fn enroll_key_via_ssh(target: &str, pub_key_path: &Path) -> Result<()> {
    if !is_valid_ssh_target(target) {
        eprintln!("Invalid SSH target: '{}'. Expected [user@]host, e.g. admin@192.168.1.5.", target);
        print_manual_enrollment_instructions(pub_key_path);
        return Ok(());
    }

    if !pub_key_path.exists() {
        eprintln!("Public key file not found at {:?}", pub_key_path);
        print_manual_enrollment_instructions(pub_key_path);
        return Ok(());
    }

    let pub_key_content = match std::fs::read_to_string(pub_key_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read public key at {:?}: {}", pub_key_path, e);
            print_manual_enrollment_instructions(pub_key_path);
            return Ok(());
        }
    };

    let user: String = env::var("USER")
        .unwrap_or_else(|_| "cli".to_string())
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let user = if user.is_empty() { "cli".to_string() } else { user };
    let remote_filename = format!("{}-admin_id_ed25519.pub", user);
    let remote_path = format!("/etc/pharos/keys/{}", remote_filename);

    let mut child = match Command::new("ssh")
        .args([
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=10",
            target,
            &format!("sudo tee {} >/dev/null && (sudo systemctl reload pharos-server || true)", remote_path),
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to execute ssh: {}", e);
            print_manual_enrollment_instructions(pub_key_path);
            return Ok(());
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(pub_key_content.as_bytes());
    }

    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Failed to wait for ssh: {}", e);
            print_manual_enrollment_instructions(pub_key_path);
            return Ok(());
        }
    };

    if output.status.success() {
        let fp = match Command::new("ssh-keygen")
            .args(["-lf", pub_key_path.to_str().unwrap_or("")])
            .output()
        {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            _ => pub_key_content.trim().to_string(),
        };
        eprintln!("Enrolled key on {} as {}.", target, remote_path);
        eprintln!("Key fingerprint: {}", fp);
        eprintln!("Note: Remote filename contains 'admin' token, granting admin role on the hub.");
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Could not auto-enroll key via SSH on {}: {}", target, if stderr.trim().is_empty() { "connection failed or command rejected" } else { stderr.trim() });
        print_manual_enrollment_instructions(pub_key_path);
    }

    Ok(())
}

pub fn print_manual_enrollment_instructions(pub_key_path: &Path) {
    let pub_name = pub_key_path.file_name().unwrap_or_default().to_string_lossy();
    eprintln!("\nTo manually enroll this key on your Pharos hub:");
    eprintln!("  1. Copy public key to hub:");
    eprintln!("     scp {} <user>@<hub>:/tmp/", pub_key_path.display());
    eprintln!("  2. Move it to /etc/pharos/keys/ with an 'admin' token in the filename:");
    eprintln!("     sudo mv /tmp/{} /etc/pharos/keys/user-admin_id_ed25519.pub", pub_name);
    eprintln!("  3. Reload pharos-server:");
    eprintln!("     sudo systemctl reload pharos-server");
}

async fn offer_ssh_enrollment_and_retry(
    client: &mut PharosClient,
    cmd_to_send: &str,
    pub_key_path: &Path,
    fallback_err: anyhow::Error,
) -> Result<PharosResponse> {
    eprint!("Enroll this key's public half on a hub now via SSH? Enter [user@]host, or leave blank to skip: ");
    io::stderr().flush().ok();
    let mut target = String::new();
    io::stdin().read_line(&mut target)?;
    let target = target.trim();
    if !target.is_empty() {
        enroll_key_via_ssh(target, pub_key_path)?;
        client.execute_authenticated(cmd_to_send).await
    } else {
        print_manual_enrollment_instructions(pub_key_path);
        Err(fallback_err)
    }
}

pub async fn execute_with_interactive_setup(
    client: &mut PharosClient,
    cmd_to_send: &str,
) -> Result<PharosResponse> {
    let err = match client.execute_authenticated(cmd_to_send).await {
        Ok(resp) => return Ok(resp),
        Err(e) => e,
    };

    if !io::stdin().is_terminal() {
        return Err(err);
    }

    if is_missing_key_error(&err) {
        let key_path = default_personal_key_path()?;
        if offer_to_generate_key(&key_path)? {
            let retry_res = client.execute_authenticated(cmd_to_send).await;
            match retry_res {
                Ok(resp) => return Ok(resp),
                Err(retry_err) => {
                    if is_auth_failure_error(&retry_err) {
                        let pub_key_path = key_path.with_extension("pub");
                        return offer_ssh_enrollment_and_retry(client, cmd_to_send, &pub_key_path, retry_err).await;
                    } else {
                        return Err(retry_err);
                    }
                }
            }
        } else {
            return Err(err);
        }
    } else if is_auth_failure_error(&err) {
        if let Ok(key_path) = default_personal_key_path() {
            let pub_key_path = key_path.with_extension("pub");
            if pub_key_path.exists() {
                return offer_ssh_enrollment_and_retry(client, cmd_to_send, &pub_key_path, err).await;
            }
        }
    }

    Err(err)
}

pub const CLIENT_CONF_PATH: &str = "/etc/pharos/client.conf";

pub fn read_configured_server() -> Option<String> {
    read_configured_server_from_path(Path::new(CLIENT_CONF_PATH))
}

pub fn read_configured_server_from_path(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(val) = line.strip_prefix("PHAROS_SERVER=") {
            let val = val.trim().trim_matches(|c| c == '\'' || c == '"');
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Resolves the hub address the same way for every CLI client (mdb, ph), in one place, so a
/// future change to this precedence chain (or to how the source is described in error messages)
/// only needs to happen once. Returns the address alongside a human-readable description of
/// where it came from, so a connection-failure message can tell the operator what to check
/// instead of just repeating the address back at them.
pub fn resolve_server_address() -> (String, &'static str) {
    if let Ok(server) = env::var("PHAROS_SERVER") {
        (server, "PHAROS_SERVER environment variable")
    } else if let Ok(host) = env::var("PHAROS_HOST") {
        let port = env::var("PHAROS_PORT").unwrap_or_else(|_| "2378".to_string());
        (format!("{}:{}", host, port), "PHAROS_HOST/PHAROS_PORT environment variables")
    } else if let Some(server) = read_configured_server() {
        (server, "/etc/pharos/client.conf")
    } else {
        (
            "127.0.0.1:2378".to_string(),
            "built-in default — no PHAROS_SERVER/PHAROS_HOST env var set and no /etc/pharos/client.conf found",
        )
    }
}

/// Heuristic only, never blocks execution: if a query looks like it might be an
/// unquoted shell glob that expanded into a directory listing (many bare tokens,
/// none in field=value form), warn to stderr so the operator notices before
/// assuming a clean "no matches" is a genuine empty result. A single correctly-
/// quoted `*` is exactly one token and never trips this.
pub fn looks_like_glob_expansion(tokens: &[String]) -> bool {
    const THRESHOLD: usize = 15;
    if tokens.len() < THRESHOLD {
        return false;
    }
    tokens.iter().all(|t| !t.contains('='))
}

pub fn warn_if_looks_like_glob_expansion(tokens: &[String]) {
    if looks_like_glob_expansion(tokens) {
        eprintln!(
            "Warning: this command included {} arguments with no field=value pairs. If you meant to search with a wildcard (e.g. mdb '*'), make sure to quote it — otherwise your shell may have expanded it into a list of filenames from the current directory before mdb/ph ever saw it. Proceeding anyway.",
            tokens.len()
        );
    }
}

pub fn enforce_add_record_type(cmd_str: &str, expected_type: &str, cli_name: &str) -> String {
    let mut tokens = tokenize_cmd(cmd_str);
    if tokens.is_empty() {
        return cmd_str.to_string();
    }
    if !tokens[0].eq_ignore_ascii_case("add") {
        return cmd_str.to_string();
    }

    let mut type_found = false;
    for token in tokens.iter_mut().skip(1) {
        if let Some((k, v)) = token.split_once('=') {
            if k.eq_ignore_ascii_case("type") {
                let unquoted = v.trim_matches('"');
                if !unquoted.eq_ignore_ascii_case(expected_type) {
                    eprintln!(
                        "Note: {} always registers {}-type records; overriding type={} to type={}.",
                        cli_name, expected_type, unquoted, expected_type
                    );
                }
                *token = format!("type={}", expected_type);
                type_found = true;
            }
        }
    }

    if !type_found {
        tokens.push(format!("type={}", expected_type));
    }

    tokens.join(" ")
}

pub fn tokenize_cmd(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escaped = false;
    for c in line.chars() {
        if escaped {
            current.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            in_quotes = !in_quotes;
            current.push(c);
        } else if c.is_whitespace() && !in_quotes {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_identify_missing_key_error() {
        let err = anyhow::anyhow!("No private key found for signing. Checked, in order:\n  - /root/.ssh/id_ed25519");
        assert!(is_missing_key_error(&err));
    }

    #[test]
    fn test_should_not_identify_other_errors_as_missing_key() {
        let err = anyhow::anyhow!("Failed to connect to Pharos server at 127.0.0.1:2378");
        assert!(!is_missing_key_error(&err));
    }

    #[test]
    fn test_should_validate_valid_ssh_targets() {
        assert!(is_valid_ssh_target("user@host"));
        assert!(is_valid_ssh_target("admin@192.168.1.5"));
        assert!(is_valid_ssh_target("hostname.domain.local"));
        assert!(is_valid_ssh_target("10.0.0.1"));
    }

    #[test]
    fn test_should_reject_invalid_ssh_targets_and_argument_injection() {
        assert!(!is_valid_ssh_target("-oProxyCommand=touch /tmp/pwned"));
        assert!(!is_valid_ssh_target("-h"));
        assert!(!is_valid_ssh_target(""));
        assert!(!is_valid_ssh_target("@host"));
        assert!(!is_valid_ssh_target("user@"));
        assert!(!is_valid_ssh_target("user@host;rm -rf /"));
    }

    #[test]
    fn test_should_read_configured_server_when_valid_file_exists() {
        let path = std::env::temp_dir().join(format!("test_client_conf_valid_{}.conf", std::process::id()));
        std::fs::write(&path, "PHAROS_SERVER=192.168.1.100:2378\n").unwrap();
        let res = read_configured_server_from_path(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(res, Some("192.168.1.100:2378".to_string()));
    }

    #[test]
    fn test_should_return_none_when_config_file_missing() {
        let path = Path::new("/nonexistent/path/client.conf");
        assert_eq!(read_configured_server_from_path(path), None);
    }

    #[test]
    fn test_should_return_none_when_config_file_empty() {
        let path = std::env::temp_dir().join(format!("test_client_conf_empty_{}.conf", std::process::id()));
        std::fs::write(&path, "").unwrap();
        let res = read_configured_server_from_path(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(res, None);
    }

    #[test]
    fn test_should_return_none_when_config_file_has_no_matching_line() {
        let path = std::env::temp_dir().join(format!("test_client_conf_nomatch_{}.conf", std::process::id()));
        std::fs::write(&path, "# Comment\nOTHER_VAR=value\nPHAROS_SERVER=\n").unwrap();
        let res = read_configured_server_from_path(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(res, None);
    }

    #[test]
    fn test_should_not_detect_glob_expansion_when_single_quoted_wildcard() {
        let tokens = vec!["*".to_string()];
        assert!(!looks_like_glob_expansion(&tokens));
    }

    #[test]
    fn test_should_not_detect_glob_expansion_when_below_threshold() {
        let tokens: Vec<String> = (0..14).map(|i| format!("file{}", i)).collect();
        assert!(!looks_like_glob_expansion(&tokens));
    }

    #[test]
    fn test_should_not_detect_glob_expansion_when_has_field_value_pairs() {
        let mut tokens: Vec<String> = (0..15).map(|i| format!("token{}", i)).collect();
        tokens[5] = "hostname=server1".to_string();
        assert!(!looks_like_glob_expansion(&tokens));
    }

    #[test]
    fn test_should_detect_glob_expansion_when_at_or_above_threshold_with_no_fields() {
        let tokens: Vec<String> = (0..15).map(|i| format!("file{}.txt", i)).collect();
        assert!(looks_like_glob_expansion(&tokens));
    }

    #[test]
    fn test_should_force_type_machine_on_add_when_absent() {
        let result = enforce_add_record_type("add hostname=srv-1", "machine", "mdb");
        assert_eq!(result, "add hostname=srv-1 type=machine");
    }

    #[test]
    fn test_should_override_conflicting_type_and_force_machine_on_add() {
        let result = enforce_add_record_type("add hostname=srv-2 type=person", "machine", "mdb");
        assert_eq!(result, "add hostname=srv-2 type=machine");
    }

    #[test]
    fn test_should_force_type_person_on_add_when_absent() {
        let result = enforce_add_record_type("add name=\"Jane Doe\"", "person", "ph");
        assert_eq!(result, "add name=\"Jane Doe\" type=person");
    }

    #[test]
    fn test_should_override_conflicting_type_and_force_person_on_add() {
        let result = enforce_add_record_type("add name=\"Jane Doe\" type=machine", "person", "ph");
        assert_eq!(result, "add name=\"Jane Doe\" type=person");
    }
}
