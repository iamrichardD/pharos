/* ========================================================================
 * Project: pharos
 * Component: Documentation & UX
 * File: key-auth-ux-and-privilege-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Three fixes found investigating a live "Private key not found" error from
 * `mdb auth sign`: (1) a real over-privileging security bug in how key
 * filenames grant roles (substring match, not token match), (2) an
 * incomplete/unhelpful error message that only reports the last of several
 * paths tried and gives no actionable next step, (3) mdb/ph never
 * initialize a logging backend at all, so the existing "waiting up to 60s"
 * progress message is completely silent, leaving a bare 60-second hang
 * before the confusing error appears.
 * * Traceability:
 * Found live-testing `mdb auth sign` on a client machine separate from the
 * hub (2026-08-03), panel-reviewed before this plan was written.
 * ======================================================================== */

# Plan: fix key-role over-privileging, error clarity, and silent CLI hang

## Part 1: role over-privileging via substring match (security bug, fix first)

### Background

`register_key()` (`pharos-server/src/auth.rs:262-291`, quoted verbatim, re-read fresh this
session):

```rust
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

            if filename.contains("peer") {
                roles.push("peer".to_string());
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
```

**The bug**: `str::contains("admin")` is a raw substring match, not a token match. A key file
named `administrator_backup_id_ed25519.pub` (or even `badminton_ops_id_ed25519.pub`) would be
silently granted the full root-equivalent `admin` role, since `"administrator".contains("admin")`
is `true`. Confirmed this is a real gap, not theoretical, by checking the file naming convention
this codebase already establishes in its own test (`test_should_detect_peer_role_from_filename`,
same file): filenames use `-`/`_` as **token delimiters** (`"nodeb-peer_id_ed25519"`,
`"admin-peer_id_ed25519"`) — the intent has always been per-token matching, the implementation
just never enforced it.

### The change

Split the filename stem into tokens on `-`/`_` and check for **exact token equality**, not
substring containment:

```rust
    fn register_key(
        authorized_keys: &mut Vec<PublicKey>,
        key_roles: &mut HashMap<String, Vec<String>>,
        key_teams: &mut HashMap<String, Vec<String>>,
        path: &Path,
        key: PublicKey
    ) {
        let key_b64 = STANDARD.encode(key.to_bytes().unwrap_or_default());
        authorized_keys.push(key);

        // Extract roles and teams from filename tokens (split on '-'/'_') — exact token match,
        // not substring containment, so "administrator_backup_id_ed25519" does NOT match "admin".
        let mut roles = Vec::new();
        let mut teams = Vec::new();
        if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
            let tokens: Vec<&str> = filename.split(|c: char| c == '-' || c == '_').collect();

            if tokens.contains(&"admin") {
                roles.push("admin".to_string());
            } else if tokens.contains(&"user") {
                roles.push("user".to_string());
            }

            if tokens.contains(&"peer") {
                roles.push("peer".to_string());
            }

            // Simple team detection: e.g. "devops_id_ed25519.pub"
            if tokens.contains(&"devops") {
                teams.push("devops".to_string());
            }
            if tokens.contains(&"security") {
                teams.push("security".to_string());
            }
        }
        key_roles.insert(key_b64.clone(), roles);
        key_teams.insert(key_b64, teams);
    }
```

Verified this preserves every existing real usage pattern seen in this codebase (auto-generated
`admin_id_ed25519`, and the test's `nodeb-peer_id_ed25519`/`admin-peer_id_ed25519`) — all still
tokenize to include the exact `"admin"`/`"peer"` tokens — while `"administrator_backup_id_ed25519"`
tokenizes to `["administrator", "backup", "id", "ed25519"]`, which does **not** contain the exact
token `"admin"`.

### Non-goals (Part 1)

- **Do not** change the delimiter set beyond `-`/`_` (matching the exact convention this
  codebase's own test already establishes) — don't add `.`/space/other delimiters not already in
  use.
- **Do not** change team detection logic beyond the same token-match fix — same bug class, same
  fix, no new team names or roles introduced.
- **Do not** touch key *storage*/*loading* (`load_keys_from_dir`) — only the *parsing* of roles
  from an already-identified filename.

## Part 2: incomplete error message in `sign_message_async`

### Background

`sign_message_async` (`crates/pharos-client/src/lib.rs:339-377`, quoted verbatim, re-read fresh
this session):

```rust
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
```

Real evidence: a user on a machine separate from the hub got
`Private key not found at "/home/rdelgado/.ssh/admin_id_ed25519" after 60s` — reporting only the
*last* path checked, not the personal-key path also tried, nor the hub-local fallback path also
checked. No guidance on what to actually do (generate a personal key and enroll it, vs. copy the
sensitive admin key).

### The change

Preserve the **exact existing control flow and timing** (same resolution order, same single wait,
same single fallback check — this is a reporting fix, not a logic change) but track which paths
were considered, and extract a small pure helper (trivially unit-testable, no env vars, no async,
no waiting) that builds the final "paths tried" list — directly fixing the existing empty test
stub `test_should_correctly_sign_challenge_when_key_exists`, which today does nothing:

```rust
    /// Pure and synchronous by design so it's cheaply unit-testable without touching real env
    /// vars, the filesystem, or the 60s wait loop in sign_message_async.
    fn describe_attempted_paths(
        personal_key_path: &str,
        used_personal_key: bool,
        resolved_path: &str,
        fallback_path: &str,
    ) -> Vec<String> {
        let mut tried = Vec::new();
        if !used_personal_key {
            tried.push(personal_key_path.to_string());
        }
        tried.push(resolved_path.to_string());
        tried.push(fallback_path.to_string());
        tried
    }

    pub async fn sign_message_async(message: &str) -> Result<(String, String)> {
        let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let personal_key_path = format!("{}/.ssh/id_ed25519", home);

        let (priv_key_path_str, used_personal_key) = match env::var("PHAROS_PRIVATE_KEY") {
            Ok(explicit) => (explicit, false),
            Err(_) => {
                if Path::new(&personal_key_path).exists() {
                    (personal_key_path.clone(), true)
                } else {
                    (format!("{}/.ssh/admin_id_ed25519", home), false)
                }
            }
        };

        let priv_key_path = Path::new(&priv_key_path_str);

        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(60);

        if !priv_key_path.exists() {
            log::info!("Waiting for private key at {:?} (timeout: 60s)...", priv_key_path);
        }

        while !priv_key_path.exists() && start.elapsed() < timeout {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }

        if !priv_key_path.exists() {
            let fallback_path = Path::new("/etc/pharos/keys/admin_id_ed25519");
            if fallback_path.exists() {
                log::info!("Primary key not found, but found fallback at {:?}", fallback_path);
                return Self::sign_with_key_path(fallback_path, message).await;
            }

            let tried = Self::describe_attempted_paths(
                &personal_key_path,
                used_personal_key,
                &priv_key_path_str,
                &fallback_path.display().to_string(),
            );
            let tried_list = tried.iter().map(|p| format!("  - {}", p)).collect::<Vec<_>>().join("\n");

            return Err(anyhow!(
                "No private key found for signing. Checked, in order:\n{}\n\n\
                 If this machine is separate from the hub that generated the admin key, generate \
                 a personal key here instead of copying the sensitive admin private key: \
                 `ssh-keygen -t ed25519 -f {}`, then enroll its PUBLIC half (the .pub file) in \
                 the hub's /etc/pharos/keys/ directory under a filename containing \"admin\" as \
                 its own token (e.g. <name>-admin_id_ed25519.pub), and reload pharos-server. Or \
                 set PHAROS_PRIVATE_KEY to point at a specific key file.",
                tried_list, personal_key_path
            ));
        }

        Self::sign_with_key_path(priv_key_path, message).await
    }
```

Replace the existing empty test stub with real, fast, synchronous tests of the new pure helper:

```rust
    #[test]
    fn test_should_list_all_attempted_paths_when_personal_key_not_used() {
        let tried = PharosClient::describe_attempted_paths(
            "/home/user/.ssh/id_ed25519",
            false,
            "/home/user/.ssh/admin_id_ed25519",
            "/etc/pharos/keys/admin_id_ed25519",
        );
        assert_eq!(tried, vec![
            "/home/user/.ssh/id_ed25519".to_string(),
            "/home/user/.ssh/admin_id_ed25519".to_string(),
            "/etc/pharos/keys/admin_id_ed25519".to_string(),
        ]);
    }

    #[test]
    fn test_should_omit_personal_key_from_list_when_it_was_the_resolved_path() {
        let tried = PharosClient::describe_attempted_paths(
            "/home/user/.ssh/id_ed25519",
            true,
            "/home/user/.ssh/id_ed25519",
            "/etc/pharos/keys/admin_id_ed25519",
        );
        assert_eq!(tried, vec![
            "/home/user/.ssh/id_ed25519".to_string(),
            "/etc/pharos/keys/admin_id_ed25519".to_string(),
        ]);
    }
```

Remove the old empty `test_should_correctly_sign_challenge_when_key_exists` stub and its comment
about needing a real key — these two new tests are the real coverage this function has been
missing.

### Non-goals (Part 2)

- **Do not** change the actual resolution order, the 60-second wait duration, or which path gets
  checked when — this is purely about what gets *reported* on total failure. Verify the existing
  behavior for the success paths (personal key found immediately, admin guess found immediately,
  hub-local fallback found after timeout) is completely unaffected.
- **Do not** attempt to make the full async wait loop itself unit-testable (e.g. via a
  configurable/injectable timeout) — that's a larger refactor than this fix warrants; the pure
  `describe_attempted_paths` helper is what actually needed test coverage (the reporting logic),
  and it's now covered without touching the timing-sensitive code at all.
- **Do not** touch `sign_with_key_path` or anything about how a found key is actually used to
  sign — unrelated, already correct.

## Part 3: mdb/ph never initialize a logging backend (silent 60s hang)

### Background

Confirmed this session: `mdb/Cargo.toml` and `ph/Cargo.toml` both depend on `log = "0.4"` (the
facade only), but neither `mdb/src/main.rs` nor `ph/src/main.rs` ever initializes any logger
backend. Every `log::info!`/`log::warn!` call anywhere in their dependency chain — including
`pharos-client`'s "Waiting for private key... (timeout: 60s)" message — is a complete no-op. A user
running `mdb auth sign` with no key set up experiences a **silent 60-second hang** with zero
output before the final error appears.

This workspace already has an established logging convention elsewhere (`pharos-server/src/main.rs`,
quoted verbatim):

```rust
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::builder()
                    .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
                    .from_env_lossy(),
            )
            .init();
```

...using `tracing`/`tracing-subscriber`, not `env_logger`. `pharos-client` (and therefore `mdb`/`ph`
transitively) uses the plain `log` facade, not `tracing` directly — `tracing-subscriber` does not
automatically capture plain `log` crate records without an explicit bridge (the `tracing-log`
crate's `LogTracer`).

### The change

**`mdb/Cargo.toml` and `ph/Cargo.toml`** — add, matching versions already used elsewhere in this
workspace:

```toml
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-log = "0.2"
```

(`log = "0.4"` is already a dependency of both — keep it, `pharos-client` and these binaries'
existing `log::` call sites, if any, still use it.)

**`mdb/src/main.rs` and `ph/src/main.rs`** — at the very top of `main()`, before any other logic,
initialize the same way `pharos-server` does, plus the `log` bridge:

```rust
    tracing_log::LogTracer::init().ok(); // bridges the `log` facade (used by pharos-client) into tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();
```

**Empirically verify this actually bridges `log::info!` calls** (don't assume the `tracing-log`
crate's behavior from documentation alone) — build `mdb`, run `mdb auth sign <any-uuid>` in an
environment where no private key exists, and confirm the "Waiting for private key at ... (timeout:
60s)..." message now actually appears on stderr/stdout promptly, instead of a silent hang. If it
does not appear, investigate whether an additional bridging step or crate feature is needed rather
than assuming the plan's snippet is sufficient as written.

### Non-goals (Part 3)

- **Do not** change the default log level below INFO, or add verbose/debug output beyond what
  `pharos-server`'s existing convention already establishes — match it exactly, don't invent a
  new default.
- **Do not** touch `pharos-scan`'s or `pharos-console`'s own separate logging setups — they're
  already initialized (`tracing_subscriber::FmtSubscriber`, `tracing_subscriber::fmt()`
  respectively per this session's own research), unrelated to this gap.
- **Do not** add logging initialization to `pharos-client` itself — libraries shouldn't install
  global logger state; only the binaries (`mdb`, `ph`) should.

## Verification steps (concrete — all three parts, in Podman per this repo's Zero-Host policy)

```bash
podman build -t pharos-test -f Containerfile.test .
podman run --rm pharos-test
```

Must include, at minimum:

**Part 1**:
1. `cargo test -p pharos-server` passes, including the existing
   `test_should_detect_peer_role_from_filename` test (regression — must still pass unmodified).
2. **New test**: add a test proving the fix — a key filename containing `"administrator"` (or
   similar superstring of `"admin"`) does **not** get the `admin` role, while
   `"someuser-admin_id_ed25519.pub"` (exact token) still does.

**Part 2**:
3. `cargo test -p pharos-client` (or workspace-wide with the feature unification this session
   already knows about — `cargo test --workspace -p pharos-client`) passes, including the two new
   `describe_attempted_paths` tests.
4. Confirm the old empty test stub is gone, not just left alongside the new ones.

**Part 3**:
5. Build `mdb` and `ph`, and for at least `mdb`, actually run `mdb auth sign <fake-uuid>` (in a
   throwaway environment/container with no private key present anywhere in the resolution chain —
   e.g. override `HOME` to an empty temp dir) and confirm the "Waiting for private key..." message
   is now visible in real output within the first second or two, not silent. This is the concrete
   proof the logging fix actually addresses the reported problem, not just that it compiles.
6. Confirm the final error message (once the 60s timeout is reached — acceptable to actually wait
   this out once for a definitive real-world check, or reason carefully about whether a shorter
   reproduction is possible without touching the deliberately-unchanged timing logic from Part 2)
   lists multiple paths and the actionable guidance text, matching Part 2's new message.

## Report back

State clearly: the exact diff for all touched files (`pharos-server/src/auth.rs`,
`crates/pharos-client/src/lib.rs`, `mdb/Cargo.toml`, `mdb/src/main.rs`, `ph/Cargo.toml`,
`ph/src/main.rs`), all 6 verification results, and confirmation no other file was touched. Do not
commit or push — this repo requires explicit instruction for that, every time.
