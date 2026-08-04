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
}
