/* ========================================================================
 * Project: pharos
 * Component: CLI-mdb, CLI-ph
 * File: mdb-ph-key-setup-dedup-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Issue #185's interactive key-setup implementation (mdb/src/main.rs,
 * ph/src/main.rs) shipped with ~250+ lines duplicated byte-for-byte between
 * the two files, plus a second within-file duplication (the "offer to
 * generate a key" pattern appears twice in each file: once in the inline
 * `auth sign` handler, once in `execute_with_interactive_setup`). Panel
 * review also found two small, concrete correctness/security gaps in this
 * same code, bundled into this pass since the fix touches the exact same
 * functions being extracted.
 * * Traceability:
 * Panel review 2026-08-04, following live verification of Issue #185.
 * ======================================================================== */

# Plan: deduplicate mdb/ph's interactive key-setup code + two panel findings

## Part 1: extract shared code into a new crate

### Background (already established — do not re-litigate)

`mdb` and `ph` are separate binary crates (`mdb/Cargo.toml`, `ph/Cargo.toml`), both
depending on `pharos-client` (`crates/pharos-client`) but with no shared crate between
themselves. `crates/pharos-pulse` also depends on `pharos-client` but must **never**
depend on anything interactive — this is the load-bearing constraint from the original
`interactive-key-setup-plan.md`: prompting code must stay out of `pharos-client` because
`pharos-pulse` runs unattended via systemd with no controlling terminal.

### The change

Create a new library crate `crates/pharos-cli-support` (new workspace member), depended
on **only** by `mdb` and `ph` — not by `pharos-client`, not by `pharos-pulse`, not by
`pharos-server`. Move these functions into it verbatim (currently duplicated identically
in both `mdb/src/main.rs` and `ph/src/main.rs`):

- `is_missing_key_error`
- `is_auth_failure_error`
- `default_personal_key_path`
- `generate_local_ed25519_key`
- `is_valid_ssh_target`
- `enroll_key_via_ssh`
- `print_manual_enrollment_instructions`
- `execute_with_interactive_setup` (operates on `&mut PharosClient`/`&str` from
  `pharos-client`, which the new crate should depend on — this is fine, `pharos-client`
  itself stays untouched and non-interactive; only this new crate gains the interactive
  behavior, and only `mdb`/`ph` depend on this new crate)

Their existing unit tests (`test_should_identify_missing_key_error`,
`test_should_not_identify_other_errors_as_missing_key`,
`test_should_validate_valid_ssh_targets`,
`test_should_reject_invalid_ssh_targets_and_argument_injection`) move with them into the
new crate — no need for two copies of the same tests either.

`mdb/src/main.rs` and `ph/src/main.rs` then just call these via
`pharos_cli_support::{...}` instead of defining their own copies. Each file keeps its own
`Cargo.toml`-declared dependency on the new crate, added alongside the existing
`pharos-client` dependency line.

### Also fix while touching this code: the second within-file duplication

Both files currently have the "offer to generate a key" prompt logic duplicated *within
the same file* — once inline in the `Commands::Auth { sub: AuthCommands::Sign { .. } }`
handler near the top of `main()`, and once inside `execute_with_interactive_setup`. Fold
the inline `auth sign` handler down to also call through
`pharos_cli_support::execute_with_interactive_setup`-style logic, or extract the
"generate on decline/accept" sub-flow into its own small shared function
(e.g. `offer_to_generate_key(&Path) -> Result<bool>`) that both call sites use, so the
Y/n-prompt-then-generate sequence exists in exactly one place in the new crate, not two.
Use your own judgment on the cleanest shape for this — the goal is one implementation of
"ask the user whether to generate a key, and generate it if they say yes", reused by both
call sites in both binaries.

## Part 2: two panel-review findings to fix in the same pass

### Finding A (Senior DevSecOps Specialist) — unsanitized `$USER` in remote SSH command

In `enroll_key_via_ssh`, currently:

```rust
let user = env::var("USER").unwrap_or_else(|_| "cli".to_string());
let remote_filename = format!("{}-admin_id_ed25519.pub", user);
let remote_path = format!("/etc/pharos/keys/{}", remote_filename);
```

`remote_path` is then embedded directly into a command string that gets shell-interpreted
on the remote hub (`format!("sudo tee {} >/dev/null && ...", remote_path)`). `$USER` is
client-side, environment-controlled, and unsanitized before this use — inconsistent with
this exact codebase's own already-fixed precedent for the identical problem:
`scripts/install.sh`'s `enroll_pulse_key_via_ssh()` sanitizes its equivalent value with
`node_label="$(hostname -s | tr -cd 'A-Za-z0-9-')"` before using it the same way.

**Fix**: sanitize `user` the same way — filter to ASCII alphanumeric + `-`/`_` only
before building `remote_filename`, e.g.:

```rust
let user: String = env::var("USER")
    .unwrap_or_else(|_| "cli".to_string())
    .chars()
    .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
    .collect();
let user = if user.is_empty() { "cli".to_string() } else { user };
```

### Finding B (Kathy Sierra) — enrollment offered on any retry failure, not just auth failures

In `execute_with_interactive_setup`, the missing-key branch currently offers SSH
enrollment on **any** error from the post-generation retry:

```rust
let retry_res = client.execute_authenticated(cmd_to_send).await;
match retry_res {
    Ok(resp) => return Ok(resp),
    Err(retry_err) => {
        // offers enrollment unconditionally here
```

The sibling branch (`else if is_auth_failure_error(&err)`) correctly gates the same offer
on the error actually being auth-related first. Apply the same gate here: only offer
enrollment when `is_auth_failure_error(&retry_err)` is true; otherwise return
`Err(retry_err)` directly without prompting (a freshly-generated, unenrolled key will
almost always produce an auth-failure-shaped error on this retry in the normal case, so
this should not change the common-path behavior — it only stops an irrelevant enrollment
prompt from appearing when the retry failed for an unrelated reason, e.g. a network drop).

## Non-goals (do not touch)

- **Do not** touch `crates/pharos-client/src/lib.rs` — the new crate depends on it, but
  its own contents stay exactly as they are.
- **Do not** touch `crates/pharos-pulse` — it must never depend on the new crate.
- **Do not** change any user-visible prompt wording, behavior, or exit codes beyond what
  Finding A and Finding B require — this is a refactor plus two targeted fixes, not a
  redesign.
- **Do not** change `scripts/install.sh`'s already-shipped, already-verified pulse-key
  enrollment code — it's referenced here only as the precedent to match, not something to
  edit.

## A note on your own live-verification approach (read before starting)

A prior attempt at live-verifying this exact interactive flow got stuck for a long time
using `expect` to drive a two-prompt pty session — `expect`'s `send` for the *second*
prompt did not reliably deliver input to the child process's second
`io::stdin().read_line()` call, causing an apparent hang that was actually a test-harness
problem, not an application bug. This was root-caused and resolved by using Python's
`pty` module directly (`pty.fork()` with an explicit `select`/`os.read`/`os.write` loop,
no `expect`), which worked reliably and deterministically. If your own verification needs
to drive a multi-prompt interactive session, prefer that approach (or an equivalent
byte-level pty driver) over `expect` or bare `script`, given this precedent.

Also: `sign_message_async` (untouched, pre-existing, in `pharos-client`) waits up to 60
seconds for a key to appear before giving its final "not found" error — budget your test
timeouts generously (90-120s+) to avoid mistaking this pre-existing wait for a hang.

## Verification steps (concrete)

1. `cargo build --workspace` and `cargo test --workspace --all-features` both pass, run
   inside Podman.
2. Re-run all 5 of the original plan's live verification scenarios for **both** `mdb` and
   `ph`, against a real disposable Protected-tier hub in Podman (reuse the pattern above):
   TTY-gating regression (no prompt, no hang, with `stdin` from `/dev/null`), interactive
   success path (real key generated + real SSH enrollment + successful retried query),
   decline path (no key generated), argument-injection rejection, and existing-wrong-type-
   key (no overwrite). All must behave identically to before this refactor — this is a
   structural change, not a behavior change (aside from Finding A/B).
3. Specifically confirm Finding A: attempt enrollment with `USER` set to a value
   containing shell metacharacters (e.g. `USER='x; touch /tmp/pwned'`) and confirm the
   resulting remote filename is sanitized and no injected command executes on the hub.
4. Specifically confirm Finding B: simulate a non-auth-related retry failure after key
   generation (e.g. kill the hub mid-retry, or point at an unreachable host after
   generation) and confirm the enrollment prompt is **not** offered — the original error
   propagates directly.
5. Confirm via `git diff --stat` that only `mdb/src/main.rs`, `ph/src/main.rs`,
   `Cargo.toml` (workspace members list), and the new `crates/pharos-cli-support/`
   directory are touched — `crates/pharos-client` and `crates/pharos-pulse` must show no
   diff at all.
6. Clean up all disposable test containers/network.

## Report back

State clearly: the exact diff (all touched files), results of all 6 verification steps,
and explicit confirmation `pharos-client` and `pharos-pulse` were not touched. Do not
commit or push — this repo requires explicit instruction for that, every time.
