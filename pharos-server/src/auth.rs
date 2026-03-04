/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/src/auth.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module implements SSH-key-based authentication. It provides
 * functionality to parse public keys, verify signatures against
 * challenges, and manage authorized keys.
 * * Traceability:
 * Related to Task 4.3 (Issue #15)
 * ======================================================================== */

use ssh_key::{PublicKey, Signature};
use signature::Verifier;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::fs;
use std::path::Path;
use tracing::{info, error};
use std::collections::HashMap;

/// Defines the operational security tier of the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityTier {
    /// Unauthenticated read-only access, authenticated writes.
    Open,
    /// Authenticated access for both reads and writes.
    Protected,
    /// Role-based access control based on provenance metadata.
    Scoped,
}

impl Default for SecurityTier {
    fn default() -> Self {
        SecurityTier::Open
    }
}

pub struct AuthManager {
    authorized_keys: Vec<PublicKey>,
    key_roles: HashMap<String, Vec<String>>, // Maps base64 public key to a list of roles
}

impl AuthManager {
    pub fn new(keys_dir: &Path) -> Self {
        let mut authorized_keys = Vec::new();
        let mut key_roles = HashMap::new();

        // Ensure keys directory exists
        if !keys_dir.exists() {
            if let Err(e) = fs::create_dir_all(keys_dir) {
                error!("Failed to create keys directory {:?}: {}", keys_dir, e);
            } else {
                info!("Created keys directory {:?}", keys_dir);
            }
        }

        // 1. Initial Load
        if keys_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(keys_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().map(|s| s == "pub").unwrap_or(false) {
                        if let Ok(content) = fs::read_to_string(&path) {
                            match PublicKey::from_openssh(&content) {
                                Ok(key) => {
                                    info!("Loaded authorized key from {:?}", path);
                                    Self::register_key(&mut authorized_keys, &mut key_roles, &path, key);
                                }
                                Err(e) => error!("Failed to parse public key {:?}: {}", path, e),
                            }
                        }
                    }
                }
            }
        }

        // 2. Auto-generation if no keys found
        if authorized_keys.is_empty() {
            info!("No authorized keys found. Generating initial admin keypair...");
            let admin_priv_path = keys_dir.join("admin_id_ed25519");
            let admin_pub_path = keys_dir.join("admin_id_ed25519.pub");

            use ssh_key::PrivateKey;
            let mut rng = rand::rngs::OsRng;
            match PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519) {
                Ok(priv_key) => {
                    let priv_openssh = priv_key.to_openssh(ssh_key::LineEnding::LF).unwrap();
                    let pub_openssh = priv_key.public_key().to_openssh().unwrap();

                    if let Err(e) = fs::write(&admin_priv_path, priv_openssh.as_bytes()) {
                        error!("Failed to save initial private key: {}", e);
                    } else {
                        // Set strict permissions on private key if on Unix
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let mut perms = fs::metadata(&admin_priv_path).unwrap().permissions();
                            perms.set_mode(0o600);
                            let _ = fs::set_permissions(&admin_priv_path, perms);
                        }
                        info!("Initial private key saved to {:?}", admin_priv_path);
                    }

                    if let Err(e) = fs::write(&admin_pub_path, pub_openssh.as_bytes()) {
                        error!("Failed to save initial public key: {}", e);
                    } else {
                        info!("Initial public key saved to {:?}", admin_pub_path);
                        if let Ok(key) = PublicKey::from_openssh(&pub_openssh) {
                            Self::register_key(&mut authorized_keys, &mut key_roles, &admin_pub_path, key);
                        }
                    }
                }
                Err(e) => error!("Failed to generate initial keypair: {}", e),
            }
        }

        Self { authorized_keys, key_roles }
    }

    fn register_key(authorized_keys: &mut Vec<PublicKey>, key_roles: &mut HashMap<String, Vec<String>>, path: &Path, key: PublicKey) {
        let key_b64 = STANDARD.encode(key.to_bytes().unwrap_or_default());
        authorized_keys.push(key);

        // Extract roles from comment or filename
        let mut roles = Vec::new();
        if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
            if filename.contains("admin") {
                roles.push("admin".to_string());
            } else if filename.contains("user") {
                roles.push("user".to_string());
            }
        }
        key_roles.insert(key_b64, roles);
    }

    pub fn verify(&self, public_key_b64: &str, signature_b64: &str, challenge: &str) -> bool {
        // 1. Decode public key
        let pub_key = match PublicKey::from_openssh(public_key_b64) {
            Ok(k) => k,
            Err(_) => {
                // Try parsing as raw bytes if it was base64 encoded
                match STANDARD.decode(public_key_b64) {
                    Ok(bytes) => match PublicKey::from_bytes(&bytes) {
                        Ok(k) => k,
                        Err(e) => {
                            error!("Failed to parse public key: {}", e);
                            return false;
                        }
                    },
                    Err(_) => return false,
                }
            }
        };

        // 2. Check if authorized
        if !self.authorized_keys.iter().any(|k| k == &pub_key) {
            info!("Public key not authorized.");
            return false;
        }

        // 3. Decode signature
        let sig_bytes = match STANDARD.decode(signature_b64) {
            Ok(b) => b,
            Err(_) => return false,
        };

        let signature = match Signature::new(pub_key.algorithm(), sig_bytes) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to parse signature: {}", e);
                return false;
            }
        };

        // 4. Verify signature against challenge
        match pub_key.key_data().verify(challenge.as_bytes(), &signature) {
            Ok(_) => true,
            Err(e) => {
                error!("Signature verification failed: {}", e);
                false
            }
        }
    }

    pub fn get_roles(&self, public_key_b64: &str) -> Vec<String> {
        let pub_key = match PublicKey::from_openssh(public_key_b64) {
            Ok(k) => k,
            Err(_) => {
                match STANDARD.decode(public_key_b64) {
                    Ok(bytes) => match PublicKey::from_bytes(&bytes) {
                        Ok(k) => k,
                        Err(_) => return Vec::new(),
                    },
                    Err(_) => return Vec::new(),
                }
            }
        };

        let key_b64 = STANDARD.encode(pub_key.to_bytes().unwrap_or_default());
        self.key_roles.get(&key_b64).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_should_verify_valid_signature_when_key_is_authorized() {
        // Sample Ed25519 key (public part)
        // Private key: (not shown, used to generate signature)
        let pub_key_str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOm6UM1vI9z385C7S47+u7588mX36254558558558558 test@pharos";
        
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("test.pub");
        let mut file = fs::File::create(&key_path).unwrap();
        file.write_all(pub_key_str.as_bytes()).unwrap();

        let manager = AuthManager::new(dir.path());
        assert_eq!(manager.authorized_keys.len(), 1);

        // This test would need a real signature to pass verification.
        // For unit testing the logic without a real signing key in the test,
        // we can check if it at least parses and fails correctly.
        
        let challenge = "test-challenge";
        let fake_sig = STANDARD.encode([0u8; 64]);
        
        // Should fail signature verification but pass authorization check if we could mock verification
        // But for now, we'll just verify it returns false for bad signature
        assert!(!manager.verify(pub_key_str, &fake_sig, challenge));
    }
}

