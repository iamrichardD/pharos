/* ========================================================================
 * Project: pharos
 * Component: Documentation & UX
 * File: pulse-baseline-retry-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Self-contained implementation plan for Gap 2 of the node-install UX
 * investigation: pharos-pulse doesn't promptly recover from a baseline
 * registration failure caused by a transient TLS/cert-trust race on first
 * boot, leaving the node unregistered for up to an hour.
 * * Traceability:
 * Related to the node-install UX panel investigation (2026-08-02).
 * ======================================================================== */

# Plan: pharos-pulse baseline-registration retry (Gap 2)

## Background / root cause (verified against current source, not assumed)

A live install test found: on a hub, `pharos-pulse`'s first registration attempt raced a
certificate rotation on the server side and failed with a TLS trust error. The operator had to
manually run `systemctl restart pharos-pulse` to fix it — it did not recover on its own in any
reasonable time.

**Two hypotheses were checked against the actual code; only one is real:**

- ❌ **Not a systemd crash-loop / start-limit issue.** `pharos-pulse` never exits non-zero on this
  failure, so `Restart=always` / `StartLimitBurst` are irrelevant here. Do not touch
  `scripts/install.sh`'s systemd unit template (lines 355-369) for this fix.
- ✅ **Confirmed: an application-level retry gap in `crates/pharos-pulse/src/main.rs`.**

Current code, quoted verbatim (`crates/pharos-pulse/src/main.rs:69-98`):

```rust
    // 1. Baseline (ONLINE)
    println!("Collecting baseline inventory...");
    let inventory = collect_inventory();
    if let Err(e) = send_presence(&server_addr, &machine_name, "online", Some(inventory)).await {
        eprintln!("Failed to send baseline presence: {:?}", e);
    } else {
        println!("Baseline inventory sent successfully (Status: online).");
    }

    // 2. Heartbeat & Shutdown handling
    let mut heartbeat_interval = interval(Duration::from_secs(3600));
    // First tick finishes immediately, we already sent baseline, so skip first tick
    heartbeat_interval.tick().await; 

    println!("Entering heartbeat loop (60 minute intervals)...");

    tokio::select! {
        _ = async {
            loop {
                heartbeat_interval.tick().await;
                println!("Sending periodic heartbeat...");
                if let Err(e) = send_presence(&server_addr, &machine_name, "online", None).await {
                    eprintln!("Failed to send heartbeat: {:?}", e);
                }
            }
        } => {},
        _ = &mut shutdown => {
            println!("Shutdown signal received, initiating graceful exit...");
        },
    }
```

If `send_presence` fails here (e.g. `UnknownIssuer`/`BadCertificate` during a cert-rotation
window), the error is logged and **swallowed** — execution falls straight into the heartbeat
timer, whose first tick is deliberately consumed immediately (comment: "skip first tick"), so the
*next* real attempt to register is a full `Duration::from_secs(3600)` (60 minutes) later. That is
the entire "doesn't retry on its own" behavior the operator hit; the manual `systemctl restart`
just forced an immediate retry instead of waiting out the hour.

This file already has an established retry-with-backoff idiom for exactly this kind of transient
startup failure — `wait_for_server` (`crates/pharos-pulse/src/main.rs:167-182`):

```rust
async fn wait_for_server(server_addr: &str) {
    let mut delay = Duration::from_secs(1);
    loop {
        match tokio::net::TcpStream::connect(server_addr).await {
            Ok(_) => {
                println!("Connectivity verified to pharos-server at {}", server_addr);
                break;
            }
            Err(e) => {
                eprintln!("Waiting for pharos-server at {}: {} (Retrying in {:?})", server_addr, e, delay);
                sleep(delay).await;
                delay = std::cmp::min(delay * 2, Duration::from_secs(60));
            }
        }
    }
}
```

The fix: apply the same 1s→60s exponential-backoff idiom to baseline registration, instead of
inventing a new retry policy.

## The change

**File: `crates/pharos-pulse/src/main.rs`**

1. Add a new function, placed near `wait_for_server` (after it, e.g. immediately following line
   182), that retries baseline registration until it succeeds:

```rust
async fn send_baseline_until_success(
    server_addr: &str,
    machine_name: &str,
    inventory: HashMap<String, String>,
) {
    let mut delay = Duration::from_secs(1);
    loop {
        match send_presence(server_addr, machine_name, "online", Some(inventory.clone())).await {
            Ok(_) => {
                println!("Baseline inventory sent successfully (Status: online).");
                return;
            }
            Err(e) => {
                eprintln!(
                    "Failed to send baseline presence: {:?} (retrying in {:?})",
                    e, delay
                );
                sleep(delay).await;
                delay = std::cmp::min(delay * 2, Duration::from_secs(60));
            }
        }
    }
}
```

2. Replace the current baseline block (`crates/pharos-pulse/src/main.rs:69-76`) with a call to it,
   wrapped in the same `tokio::select!` + `&mut shutdown` pattern already used for
   `wait_for_server` at lines 59-67 (so a shutdown signal during a long retry loop still exits
   promptly instead of blocking on the backoff):

```rust
    // 1. Baseline (ONLINE) — retry with backoff until it succeeds or shutdown is requested, so a
    // transient failure (e.g. TLS/cert not yet trusted right after boot) doesn't leave the node
    // unregistered for up to an hour waiting on the next heartbeat tick.
    println!("Collecting baseline inventory...");
    let inventory = collect_inventory();

    tokio::select! {
        _ = send_baseline_until_success(&server_addr, &machine_name, inventory) => {},
        _ = &mut shutdown => {
            println!("Shutdown signal received during startup, exiting gracefully...");
            return Ok(());
        }
    }
```

3. Leave everything else in `main()` untouched, including the `else` branch's old success message
   (now redundant — remove the old inline `if/else` entirely since success logging moved inside
   `send_baseline_until_success`).

## Non-goals (do not touch)

- **Do not** modify the heartbeat loop's own failure handling (`main.rs:90-92`, `eprintln!("Failed
  to send heartbeat...")`). A heartbeat failure after baseline already succeeded is a different,
  lower-severity case — it self-heals at the next hourly tick, and nothing in this investigation
  found that to be a problem. Retrying heartbeats aggressively is out of scope here.
- **Do not** touch `scripts/install.sh`'s systemd unit template (lines 355-369) — confirmed above
  this is not a systemd-level problem.
- **Do not** touch `wait_for_server` itself — it already behaves correctly and is only being
  reused as a pattern reference.
- **Do not** touch `pharos-client`'s TLS/connection logic (`crates/pharos-client/src/lib.rs`).
- **Do not** touch the hardcoded version string at `main.rs:27` (`"Starting pharos-pulse agent
  v1.3.1..."`) or `crates/pharos-pulse/Cargo.toml`'s version — that is a separate, already-scoped
  fix (Gap 3) with its own plan; don't fold it into this change.
- **Do not** touch remote-node CA-trust distribution (`scripts/install.sh:446-451`) — that is Gap
  1, a separate design decision, not part of this fix.

## Verification steps (concrete)

All build/test/lint runs happen in Podman per this repo's Zero-Host policy — never on the host,
regardless of host Rust toolchain version:

```bash
podman build -t pharos-test -f Containerfile.test .
podman run --rm pharos-test
```

That must include, at minimum:
1. `cargo build -p pharos-pulse` succeeds with no new warnings.
2. `cargo test -p pharos-pulse` passes — the two existing tests in `main.rs`'s `#[cfg(test)]`
   module (`test_should_collect_inventory_fields_when_invoked`,
   `test_should_format_presence_command_correctly_when_inventory_provided`) must still pass
   unmodified; they don't touch the changed code path so should be unaffected.
3. **New test**: add a unit-level or integration-level test that proves the retry actually
   happens — e.g. a fake/mock server (or a `TcpListener` bound but immediately dropped/refusing
   connections for the first N attempts) that fails `send_presence` some number of times before
   succeeding, asserting `send_baseline_until_success` returns only after the eventual success and
   that it made more than one attempt. If a full mock server is impractical given `PharosClient`'s
   current design, at minimum test that the backoff delay sequence itself (1s, 2s, 4s, ... capped
   at 60s) matches `wait_for_server`'s existing sequence, by refactoring the backoff-sequence logic
   into a small pure helper both functions share — but only do this refactor if it's a *natural*
   extraction, not a forced one; if it doesn't fall out naturally, two independent copies of the
   same 3-line backoff idiom is fine and matches this codebase's existing style (no shared helper
   exists between `wait_for_server` and other retry sites currently).
4. **Live verification (panel-review stage, not builder):** on a real host with `pharos-server`
   and `pharos-pulse` installed via `scripts/install.sh`, deliberately break TLS trust for pulse's
   first connection attempt (e.g. temporarily point `PHAROS_CA_CERT` at a wrong/missing file, start
   pulse, observe the `eprintln!` retry-with-backoff messages in `journalctl -u pharos-pulse -f`),
   then fix the CA path and confirm baseline registration succeeds within the backoff window
   (well under a minute) without any manual `systemctl restart`.

## Report back

State clearly: build/test result (pass/fail + exact command output), whether the new test was
added and what it actually exercises, and confirm no other files besides `crates/pharos-pulse/src/main.rs`
(and its test module) were changed. Do not commit or push — this repo requires explicit
instruction for that, every time.
