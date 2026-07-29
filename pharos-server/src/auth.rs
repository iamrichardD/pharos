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
use std::path::{Path, PathBuf};
use tracing::{info, warn, error};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Instant, Duration};
use rand::rngs::OsRng;
use rand::RngCore;

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

struct Challenge {
    value: String,
    created_at: Instant,
}

/// The full set of authorized keys and derived metadata for one load pass.
/// Held behind a single lock so a reload can swap all three maps atomically.
struct KeyStore {
    authorized_keys: Vec<PublicKey>,
    key_roles: HashMap<String, Vec<String>>, // Maps base64 public key to a list of roles
    key_teams: HashMap<String, Vec<String>>, // Maps base64 public key to a list of teams
}

impl KeyStore {
    fn empty() -> Self {
        Self {
            authorized_keys: Vec::new(),
            key_roles: HashMap::new(),
            key_teams: HashMap::new(),
        }
    }
}

/// Outcome of scanning `keys_dir` once: the resulting key store plus every
/// file that was skipped (and why), so callers can log an aggregate summary.
struct LoadResult {
    store: KeyStore,
    skipped: Vec<(PathBuf, String)>,
}

/// Scans `keys_dir` for `.pub` files and loads any valid Ed25519 keys.
/// Does not mutate any shared state and does not trigger the auto-generate
/// fallback — that decision is made by the caller based on context (initial
/// boot vs. reload).
fn load_keys_from_dir(keys_dir: &Path) -> LoadResult {
    let mut store = KeyStore::empty();
    let mut skipped = Vec::new();

    if keys_dir.is_dir() {
        match fs::read_dir(keys_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().map(|s| s == "pub").unwrap_or(false) {
                        match fs::read_to_string(&path) {
                            Ok(content) => {
                                match PublicKey::from_openssh(&content) {
                                    Ok(key) => {
                                        if key.algorithm() == ssh_key::Algorithm::Ed25519 {
                                            info!("Loaded authorized key from {:?}", path);
                                            AuthManager::register_key(&mut store.authorized_keys, &mut store.key_roles, &mut store.key_teams, &path, key);
                                        } else {
                                            let algo = key.algorithm().to_string();
                                            error!("Skipping non-Ed25519 key in {:?}: {}", path, algo);
                                            skipped.push((path, format!("unsupported algorithm: {}", algo)));
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to parse public key {:?}: {}", path, e);
                                        skipped.push((path, format!("parse error: {}", e)));
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to read public key file {:?}: {}", path, e);
                                skipped.push((path, format!("read error: {}", e)));
                            }
                        }
                    }
                }
            }
            Err(e) => error!("Failed to read keys directory {:?}: {}", keys_dir, e),
        }
    }

    LoadResult { store, skipped }
}

/// Emits a single aggregate log line summarizing a load/reload pass, at a
/// level that's visible without tailing debug logs: `info!` when everything
/// loaded cleanly, `warn!` when anything was skipped.
fn log_load_summary(keys_dir: &Path, result: &LoadResult) {
    let loaded = result.store.authorized_keys.len();
    if result.skipped.is_empty() {
        info!("Loaded {} authorized key(s) from {:?}", loaded, keys_dir);
    } else {
        let skipped_list: Vec<String> = result.skipped.iter()
            .map(|(p, reason)| format!("{:?} ({})", p, reason))
            .collect();
        warn!(
            "Loaded {} authorized key(s) from {:?}; skipped {} unsupported/invalid file(s): [{}]",
            loaded, keys_dir, result.skipped.len(), skipped_list.join(", ")
        );
    }
}

pub struct AuthManager {
    keys_dir: PathBuf,
    store: RwLock<KeyStore>,
    challenges: RwLock<HashMap<String, Challenge>>,
}

impl AuthManager {
    pub fn new(keys_dir: &Path, security_tier: SecurityTier) -> Self {
        // Ensure keys directory exists
        if !keys_dir.exists() {
            if let Err(e) = fs::create_dir_all(keys_dir) {
                error!("Failed to create keys directory {:?}: {}", keys_dir, e);
            } else {
                info!("Created keys directory {:?}", keys_dir);
            }
        }

        // 1. Initial Load
        let mut result = load_keys_from_dir(keys_dir);
        log_load_summary(keys_dir, &result);

        // 2. Auto-generation if no keys found — only on initial boot, and only
        // for the Open tier. Protected/Scoped tiers are meant to require an
        // operator-provisioned credential, so silently minting one there would
        // undermine the whole point of the tier.
        if result.store.authorized_keys.is_empty() {
            match security_tier {
                SecurityTier::Open => {
                    Self::auto_generate_admin_key(keys_dir, &mut result.store);
                }
                SecurityTier::Protected | SecurityTier::Scoped => {
                    error!(
                        "SECURITY: {:?} tier requires an operator-provisioned key, but {:?} contains none. \
                        Refusing to self-issue a credential. The server will start, but ALL authenticated \
                        operations will be rejected until you add an Ed25519 .pub key to {:?} and reload \
                        (systemctl reload pharos-server, or SIGHUP).",
                        security_tier, keys_dir, keys_dir
                    );
                }
            }
        }

        Self {
            keys_dir: keys_dir.to_path_buf(),
            store: RwLock::new(result.store),
            challenges: RwLock::new(HashMap::new()),
        }
    }

    /// Re-scans `keys_dir` and atomically swaps in the newly loaded key set.
    ///
    /// This is what makes key enrollment/rotation restart-free: call it from
    /// a SIGHUP handler, an admin endpoint, or a directory watcher. The swap
    /// only happens after the new set is fully built, so there is never a
    /// window where the server has zero authorized keys because of a reload.
    /// If the directory comes back empty (e.g. caught mid-write during a key
    /// rotation), the previous key set is kept as-is and the auto-generate
    /// fallback is never triggered — that only ever runs once, at construction.
    pub fn reload(&self) {
        let result = load_keys_from_dir(&self.keys_dir);
        log_load_summary(&self.keys_dir, &result);

        if result.store.authorized_keys.is_empty() {
            warn!(
                "Reload of {:?} found zero authorized keys; retaining the previously loaded key set \
                instead of clearing it or generating a new credential.",
                self.keys_dir
            );
            return;
        }

        match self.store.write() {
            Ok(mut guard) => {
                *guard = result.store;
                info!("Authorized key set for {:?} reloaded successfully.", self.keys_dir);
            }
            Err(e) => error!("Failed to acquire write lock while reloading keys: {}", e),
        }
    }

    fn auto_generate_admin_key(keys_dir: &Path, store: &mut KeyStore) {
        warn!(
            "No authorized keys found in {:?}. Generating a brand-new admin Ed25519 credential \
            (Open tier quick-start convenience) — this creates a LIVE, unmanaged credential.",
            keys_dir
        );
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
                    warn!(
                        "Generated admin private key written to {:?} — if you didn't expect this, stop the \
                        server, delete that file, and enroll your own key in {:?} instead.",
                        admin_priv_path, keys_dir
                    );
                }

                if let Err(e) = fs::write(&admin_pub_path, pub_openssh.as_bytes()) {
                    error!("Failed to save initial public key: {}", e);
                } else {
                    info!("Initial public key saved to {:?}", admin_pub_path);
                    if let Ok(key) = PublicKey::from_openssh(&pub_openssh) {
                        Self::register_key(&mut store.authorized_keys, &mut store.key_roles, &mut store.key_teams, &admin_pub_path, key);
                    }
                }
            }
            Err(e) => error!("Failed to generate initial keypair: {}", e),
        }
    }

    fn register_key(
        authorized_keys: &mut Vec<PublicKey>,
        key_roles: &mut HashMap<String, Vec<String>>,
        key_teams: &mut HashMap<String, Vec<String>>,
        path: &Path,
        key: PublicKey
    ) {
        let key_b64 = STANDARD.encode(key.to_bytes().unwrap_or_default());
        authorized_keys.push(key);

        // Extract roles and teams from filename
        let mut roles = Vec::new();
        let mut teams = Vec::new();
        if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
            if filename.contains("admin") {
                roles.push("admin".to_string());
            } else if filename.contains("user") {
                roles.push("user".to_string());
            }

            // Simple team detection: e.g. "devops_id_ed25519.pub"
            if filename.contains("devops") {
                teams.push("devops".to_string());
            }
            if filename.contains("security") {
                teams.push("security".to_string());
            }
        }
        key_roles.insert(key_b64.clone(), roles);
        key_teams.insert(key_b64, teams);
    }

    pub fn generate_challenge(&self, alias: &str) -> String {
        let mut bytes = [0u8; 16];
        OsRng.fill_bytes(&mut bytes);
        let challenge_hex = hex::encode(bytes);

        if let Ok(mut lock) = self.challenges.write() {
            lock.insert(alias.to_string(), Challenge {
                value: challenge_hex.clone(),
                created_at: Instant::now(),
            });
        }
        challenge_hex
    }

    pub fn get_challenge(&self, alias: &str) -> Option<String> {
        let mut lock = self.challenges.write().ok()?;

        if let Some(stored) = lock.get(alias) {
            if stored.created_at.elapsed() > Duration::from_secs(300) {
                lock.remove(alias);
                return None;
            }
            return Some(stored.value.clone());
        }
        None
    }

    pub fn consume_challenge(&self, alias: &str) {
        if let Ok(mut lock) = self.challenges.write() {
            lock.remove(alias);
        }
    }

    pub fn verify(&self, public_key_b64: &str, signature_b64: &str, challenge: &str) -> bool {
        self.verify_with_fingerprint(public_key_b64, signature_b64, challenge).is_some()
    }

    pub fn verify_with_fingerprint(&self, public_key_b64: &str, signature_b64: &str, challenge: &str) -> Option<String> {
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
                            return None;
                        }
                    },
                    Err(_) => return None,
                }
            }
        };

        // 2. Check if authorized and ensure Ed25519 algorithm
        if pub_key.algorithm() != ssh_key::Algorithm::Ed25519 {
            error!("Unsupported key algorithm: {}. Only Ed25519 is allowed.", pub_key.algorithm());
            return None;
        }

        {
            let store = self.store.read().ok()?;
            if !store.authorized_keys.iter().any(|k| k == &pub_key) {
                info!("Public key not authorized.");
                return None;
            }
        }

        // 3. Decode signature
        let sig_bytes = match STANDARD.decode(signature_b64) {
            Ok(b) => b,
            Err(_) => return None,
        };

        let signature = match Signature::new(pub_key.algorithm(), sig_bytes) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to parse signature: {}", e);
                return None;
            }
        };

        // 4. Verify signature against challenge
        match pub_key.key_data().verify(challenge.as_bytes(), &signature) {
            Ok(_) => {
                // Return fingerprint (SHA256)
                Some(pub_key.fingerprint(ssh_key::HashAlg::Sha256).to_string())
            },
            Err(e) => {
                error!("Signature verification failed: {}", e);
                None
            }
        }
    }

    pub fn get_roles(&self, public_key_b64: &str) -> Vec<String> {
        self.get_key_metadata(public_key_b64, |store| &store.key_roles)
    }

    pub fn get_teams(&self, public_key_b64: &str) -> Vec<String> {
        self.get_key_metadata(public_key_b64, |store| &store.key_teams)
    }

    fn get_key_metadata(&self, public_key_b64: &str, selector: impl Fn(&KeyStore) -> &HashMap<String, Vec<String>>) -> Vec<String> {
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
        match self.store.read() {
            Ok(store) => selector(&store).get(&key_b64).cloned().unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use ssh_key::PrivateKey;
    use ed25519_dalek::{Signer, SigningKey};

    fn sign_challenge(priv_key: &PrivateKey, challenge: &str) -> String {
        let sig_bytes = match priv_key.key_data() {
            ssh_key::private::KeypairData::Ed25519(kp) => {
                let signing_key = SigningKey::from_bytes(&kp.private.to_bytes());
                signing_key.sign(challenge.as_bytes()).to_vec()
            }
            _ => panic!("Unsupported key type"),
        };
        STANDARD.encode(&sig_bytes)
    }

    #[test]
    fn test_should_generate_and_verify_challenge_when_alias_provided() {
        let dir = tempdir().unwrap();
        let auth_manager = AuthManager::new(dir.path(), SecurityTier::Open);
        let alias = "test-user";

        let challenge = auth_manager.generate_challenge(alias);
        assert_eq!(challenge.len(), 32); // Hex 16 bytes
        assert_eq!(auth_manager.get_challenge(alias), Some(challenge));
    }

    #[test]
    fn test_should_detect_teams_from_filename() {
        let dir = tempdir().unwrap();
        let pub_path = dir.path().join("devops_user_id_ed25519.pub");

        // Generate a real key for testing
        let mut rng = rand::rngs::OsRng;
        let priv_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
        let pub_openssh = priv_key.public_key().to_openssh().unwrap();
        fs::write(&pub_path, pub_openssh.as_bytes()).unwrap();

        let auth_manager = AuthManager::new(dir.path(), SecurityTier::Open);
        let teams = auth_manager.get_teams(&pub_openssh);
        assert!(teams.contains(&"devops".to_string()));

        let roles = auth_manager.get_roles(&pub_openssh);
        assert!(roles.contains(&"user".to_string()));
    }

    #[test]
    fn test_should_reject_rsa_key_when_provided() {
        let dir = tempdir().unwrap();
        let auth_manager = AuthManager::new(dir.path(), SecurityTier::Open);

        // A dummy RSA public key in OpenSSH format
        let rsa_pub = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQC8u5f9/v8v test@example.com";

        let result = auth_manager.verify_with_fingerprint(rsa_pub, "sig", "challenge");
        assert!(result.is_none(), "RSA key should be rejected even if parsing succeeded (which it shouldn't with features disabled)");
    }

    #[test]
    fn test_should_pick_up_newly_added_key_without_restart_on_reload() {
        let dir = tempdir().unwrap();
        let mut rng = rand::rngs::OsRng;

        // Seed one existing key so the empty-dir auto-generate fallback doesn't fire.
        let existing_priv = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
        fs::write(dir.path().join("existing_id_ed25519.pub"), existing_priv.public_key().to_openssh().unwrap()).unwrap();

        let auth_manager = AuthManager::new(dir.path(), SecurityTier::Open);

        // Enroll a brand-new key after the server has already started.
        let new_priv = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
        let new_pub_openssh = new_priv.public_key().to_openssh().unwrap();
        fs::write(dir.path().join("new_user_id_ed25519.pub"), new_pub_openssh.as_bytes()).unwrap();

        let challenge = "reload-test-challenge";
        let sig = sign_challenge(&new_priv, challenge);

        // Not authorized yet — it was added to disk after construction.
        assert!(auth_manager.verify_with_fingerprint(&new_pub_openssh, &sig, challenge).is_none());

        auth_manager.reload();

        // Authorized now, without any restart.
        assert!(auth_manager.verify_with_fingerprint(&new_pub_openssh, &sig, challenge).is_some());
    }

    #[test]
    fn test_should_not_drop_existing_keys_when_reloading() {
        let dir = tempdir().unwrap();
        let mut rng = rand::rngs::OsRng;

        let priv_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
        let pub_openssh = priv_key.public_key().to_openssh().unwrap();
        fs::write(dir.path().join("existing_id_ed25519.pub"), pub_openssh.as_bytes()).unwrap();

        let auth_manager = AuthManager::new(dir.path(), SecurityTier::Open);

        let challenge = "no-drop-test";
        let sig = sign_challenge(&priv_key, challenge);
        assert!(auth_manager.verify_with_fingerprint(&pub_openssh, &sig, challenge).is_some());

        // Reload with no changes on disk — the key must survive the swap.
        auth_manager.reload();
        assert!(auth_manager.verify_with_fingerprint(&pub_openssh, &sig, challenge).is_some());
    }

    #[test]
    fn test_should_retain_previous_keys_when_reload_finds_dir_transiently_empty() {
        let dir = tempdir().unwrap();
        let mut rng = rand::rngs::OsRng;

        let priv_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
        let pub_openssh = priv_key.public_key().to_openssh().unwrap();
        let key_path = dir.path().join("existing_id_ed25519.pub");
        fs::write(&key_path, pub_openssh.as_bytes()).unwrap();

        let auth_manager = AuthManager::new(dir.path(), SecurityTier::Open);

        // Simulate the directory being caught transiently empty (e.g. mid-rewrite).
        fs::remove_file(&key_path).unwrap();
        auth_manager.reload();

        let challenge = "transient-empty-test";
        let sig = sign_challenge(&priv_key, challenge);
        // The previously loaded key must still authenticate: reload must not clear on empty.
        assert!(auth_manager.verify_with_fingerprint(&pub_openssh, &sig, challenge).is_some());

        // And the auto-generate fallback must not have fired during reload.
        assert!(!dir.path().join("admin_id_ed25519").exists());
    }

    #[test]
    fn test_should_report_correct_counts_for_mixed_valid_and_invalid_keys() {
        let dir = tempdir().unwrap();
        let mut rng = rand::rngs::OsRng;

        let valid_priv = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
        fs::write(dir.path().join("valid_id_ed25519.pub"), valid_priv.public_key().to_openssh().unwrap()).unwrap();

        // Not a parseable OpenSSH key at all — exercises the "skipped" path.
        fs::write(dir.path().join("legacy_id_rsa.pub"), "ssh-rsa AAAAnotarealkey\n").unwrap();

        let result = load_keys_from_dir(dir.path());
        assert_eq!(result.store.authorized_keys.len(), 1, "exactly one valid Ed25519 key should load");
        assert_eq!(result.skipped.len(), 1, "exactly one invalid file should be reported as skipped");
    }

    #[test]
    fn test_open_tier_self_issues_admin_keypair_when_dir_empty() {
        let dir = tempdir().unwrap();
        let _auth_manager = AuthManager::new(dir.path(), SecurityTier::Open);

        assert!(dir.path().join("admin_id_ed25519").exists());
        assert!(dir.path().join("admin_id_ed25519.pub").exists());
    }

    #[test]
    fn test_protected_tier_refuses_to_self_issue_when_dir_empty() {
        let dir = tempdir().unwrap();
        let auth_manager = AuthManager::new(dir.path(), SecurityTier::Protected);

        assert!(!dir.path().join("admin_id_ed25519").exists());
        assert!(!dir.path().join("admin_id_ed25519.pub").exists());

        // No keys were authorized, so every authenticated action fails closed.
        let mut rng = rand::rngs::OsRng;
        let priv_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
        let pub_openssh = priv_key.public_key().to_openssh().unwrap();
        let sig = sign_challenge(&priv_key, "chal");
        assert!(auth_manager.verify_with_fingerprint(&pub_openssh, &sig, "chal").is_none());
    }

    #[test]
    fn test_scoped_tier_refuses_to_self_issue_when_dir_empty() {
        let dir = tempdir().unwrap();
        let _auth_manager = AuthManager::new(dir.path(), SecurityTier::Scoped);

        assert!(!dir.path().join("admin_id_ed25519").exists());
        assert!(!dir.path().join("admin_id_ed25519.pub").exists());
    }
}
