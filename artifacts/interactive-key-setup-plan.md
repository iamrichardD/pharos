/* ========================================================================
 * Project: pharos
 * Component: Documentation & UX
 * File: interactive-key-setup-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * mdb/ph currently surface a static, if now well-written, error when no
 * signing key exists, requiring the operator to context-switch out of the
 * tool to run several commands by hand across two machines. Panel-reviewed
 * (Kathy Sierra's framing: keep the operator in flow instead of ejecting
 * them into "go figure this out yourself" mode). Split into two parts: (1)
 * interactive local key generation, safe to automate since it only touches
 * the local filesystem; (2) opt-in remote enrollment, reusing install.sh's
 * already-proven --fetch-ca-ssh pattern rather than inventing a new
 * mechanism, since granting hub admin trust is a more consequential action
 * than generating a local file.
 * * Traceability:
 * User request 2026-08-04, following live testing of the key-auth error
 * message (v1.10.6) and its enrollment guidance.
 * ======================================================================== */

# Plan: interactive key setup for mdb/ph (local generation + opt-in remote enrollment)

## Background / architecture decision (already made — do not re-litigate)

**Critical constraint found during investigation, not assumed**: `pharos-pulse`
(`crates/pharos-pulse/src/main.rs:163,167`) calls the exact same
`PharosClient::connect`/`execute_authenticated` — and therefore the same
`sign_message_async` — code path that `mdb`/`ph` use. `pharos-pulse` runs as an unattended
systemd service (`User=pharos`, a `nologin` shell, no controlling terminal). **Any interactive
prompting must not live inside `pharos-client`'s shared library code** — doing so would make the
daemon attempt to read from a stdin that will never be answered, regressing exactly the kind of
silent-hang bug already fixed this session (Part 3 of the key-auth-ux plan). The shared library
(`sign_message_async`, `execute_authenticated`, `authenticate`) stays **exactly as it is today** —
this plan only adds behavior at the CLI layer (`mdb`/`ph`'s own `main()` functions), which is where
an interactive terminal can actually be assumed to exist, and even then only after confirming it.

## Part 1: interactive local key generation (do this — low risk, high value)

### The change

In both `mdb/src/main.rs` and `ph/src/main.rs`, wrap the existing call site that invokes
`execute_authenticated` (or, for `mdb auth sign`, the direct `sign_message_async` call) with a
catch for the specific "no private key found" error, gated on the process actually having a real
interactive terminal:

```rust
    let resp = match client.execute_authenticated(&cmd_to_send).await {
        Ok(resp) => resp,
        Err(e) if is_missing_key_error(&e) && std::io::stdin().is_terminal() => {
            eprintln!("No signing key found for this identity.");
            eprint!("Generate a new personal key now at ~/.ssh/id_ed25519? [Y/n] ");
            std::io::Write::flush(&mut std::io::stderr()).ok();
            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer)?;
            if answer.trim().eq_ignore_ascii_case("n") {
                return Err(e);
            }
            generate_local_ed25519_key(&default_personal_key_path()?)?;
            eprintln!("Generated. You'll still need to enroll the .pub file on your hub before this works - see below.");
            // Retry once now that a key exists.
            client.execute_authenticated(&cmd_to_send).await?
        }
        Err(e) => return Err(e),
    };
```

Notes:
- `is_missing_key_error(&e)` — a small helper matching on the specific error text/kind
  `sign_message_async` returns for "no private key found" (not a generic catch-all — a TLS
  failure, network error, or authorization denial must NOT trigger this prompt).
- `std::io::stdin().is_terminal()` (stable in Rust since 1.70, `std::io::IsTerminal` trait) — the
  load-bearing safety check. If stdin isn't a real terminal (piped input, cron, CI, systemd), skip
  straight to returning the original error exactly as today — no prompt, no hang.
- Key generation itself should shell out to the system `ssh-keygen` (already a dependency this
  project assumes is present, matching `scripts/install.sh`'s own reliance on it for
  `--fetch-ca-ssh`) with `-t ed25519 -N ""` (no passphrase) — matching the existing auto-generated
  admin key's own convention (`pharos-server/src/auth.rs`'s `auto_generate_admin_key`), so this new
  key behaves consistently with every other Pharos-managed key: file-permission-protected (600),
  not passphrase-gated. Do not prompt for a passphrase — that would make every subsequent
  `mdb`/`ph` invocation require interactive passphrase entry, which is a UX regression, not an
  improvement.
- Default new-key path: `~/.ssh/id_ed25519` (matching `sign_message_async`'s own existing
  "personal key" default) — so once generated, the very next `sign_message_async` resolution
  finds it with zero further configuration, no `PHAROS_PRIVATE_KEY` needed.
- If a file already exists at that path (e.g. an RSA key), do not overwrite it — fall back to the
  existing static error, since this plan's automation is for the "nothing there yet" case, not for
  replacing an existing key of the wrong type.

## Part 2: opt-in remote enrollment (reuse the proven --fetch-ca-ssh pattern)

### The change

After Part 1 successfully generates a local key (or if a suitable local key already exists but
isn't yet enrolled — detected by the *retry* in Part 1 still failing with an authorization/auth
error rather than succeeding), offer a **second, separate, explicit** prompt — enrollment is a more
consequential action (granting hub trust) than generating a local file, and must be its own opt-in
step, not bundled silently into Part 1's confirmation:

```rust
eprint!("Enroll this key's public half on a hub now via SSH? Enter [user@]host, or leave blank to skip: ");
let mut target = String::new();
std::io::stdin().read_line(&mut target)?;
let target = target.trim();
if !target.is_empty() {
    enroll_key_via_ssh(target, &pub_key_path)?; // see below
}
```

`enroll_key_via_ssh` should mirror `scripts/install.sh`'s `fetch_ca_via_ssh` function's proven
safety properties exactly, translated to Rust (shelling out to the system `ssh`/`scp` binaries via
`std::process::Command`, not reimplementing SSH):
- `-o BatchMode=yes -o ConnectTimeout=10` — never prompts, never hangs; fails closed if key-based
  auth to the target isn't already set up (exactly like `--fetch-ca-ssh`'s own documented
  behavior).
- Do **not** auto-accept an unknown SSH host key (no `StrictHostKeyChecking=accept-new`) — same
  reasoning as `--fetch-ca-ssh`: if the hub isn't already in the operator's `known_hosts`, fail
  closed to "couldn't enroll automatically, here's the manual command" rather than silently
  trusting a new host key.
- Validate the `target` string the same way `install.sh` validates `--fetch-ca-ssh`'s argument
  (reject anything not matching `[user@]host` shape, particularly a leading `-` — this is the
  exact SSH argument-injection class found and fixed in this session's DevSecOps hardening pass;
  a Rust reimplementation must not reintroduce it) before passing it to `Command::new("ssh")`.
- On success, print the fingerprint of what was enrolled (matching `--fetch-ca-ssh`'s own
  transparency principle) and remind the operator a filename containing "admin" as its own token
  is what grants the admin role (matching Part 1 of the earlier key-auth-ux-and-privilege-plan's
  token-based, not substring-based, role parsing).
- On any failure, fall back to printing the exact manual commands (today's existing guidance) —
  never leave the operator with only a cryptic SSH error and no path forward.

## Non-goals (do not touch)

- **Do not** add any prompting inside `pharos-client`'s shared library functions
  (`sign_message_async`, `authenticate`, `execute_authenticated`) — this is the load-bearing
  architectural constraint of this whole plan; `pharos-pulse` depends on these staying pure.
- **Do not** reimplement SSH's protocol in Rust — shell out to the system `ssh`/`scp` binaries,
  matching how `install.sh` itself does this (consistency, and avoids a much larger dependency /
  security-surface addition).
- **Do not** prompt for or support passphrase-protected keys in the auto-generated case — match
  the existing no-passphrase convention exactly.
- **Do not** touch `scripts/install.sh`'s own `--fetch-ca-ssh`/`fetch_ca_via_ssh` — this plan reuses
  its *design pattern*, not its code (different language, different call site), so the bash
  function itself is unrelated and untouched.
- **Do not** make Part 2 (remote enrollment) trigger without the operator explicitly typing a
  target — an empty/blank answer must cleanly skip it, falling back to printing the manual
  command, exactly like today.

## Verification steps (concrete)

1. **TTY-gating regression check (the most important safety property)**: run `mdb *` with stdin
   redirected from `/dev/null` (or a pipe) and no key present — confirm it returns the existing
   static error immediately, with **no** prompt attempted and no hang. This is the check that
   proves scripts/cron/CI usage isn't broken.
2. **Interactive success path**: run `mdb *` from a real pty (e.g. via `script`/`expect`, or a
   Podman container's `-t` allocated pty) with no key present, answer "y" to both prompts
   (generate, then enroll against a disposable test hub reachable via the same
   disposable-Podman-sshd-container pattern already used and proven for `--fetch-ca-ssh`'s own
   verification this session), and confirm: a real Ed25519 key now exists at `~/.ssh/id_ed25519`,
   it was actually enrolled on the test hub with an `admin`-token filename, and the retried
   `execute_authenticated` call now succeeds.
3. **Decline path**: same setup, answer "n" to the first prompt — confirm no key is generated and
   the original error is returned, unchanged.
4. **Argument-injection regression check**: feed `enroll_key_via_ssh` a target starting with `-`
   (e.g. `-oProxyCommand=...`) and confirm it's rejected by validation before ever reaching
   `Command::new("ssh")` — this is the exact vulnerability class already found and fixed once this
   session; a new Rust implementation must not reintroduce it.
5. **Existing-wrong-type-key check**: place an RSA key at `~/.ssh/id_ed25519` first, confirm Part 1
   does not overwrite it and instead falls through to the existing static error.
6. Full existing test suite must still pass unmodified (nothing in `pharos-client` changes).
7. Clean up all test containers/keys/SSH config changes.

## Report back

State clearly: the exact diff for all touched files, results of all 7 verification steps
(especially #1 and #4, the two safety-critical ones), and confirmation `pharos-client`'s shared
library code was not touched. Do not commit or push — this repo requires explicit instruction for
that, every time.
