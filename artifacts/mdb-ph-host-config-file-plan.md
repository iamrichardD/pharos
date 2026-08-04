/* ========================================================================
 * Project: pharos
 * Component: Installer / CLI-mdb / CLI-ph
 * File: mdb-ph-host-config-file-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * mdb/ph only ever find the hub if PHAROS_SERVER or PHAROS_HOST/PHAROS_PORT
 * are exported in the operator's shell - a real, live-confirmed friction
 * point (Issue #183): `PHAROS_HOST=host` (no `export`) shows correctly via
 * `echo $PHAROS_HOST` in the current shell, but is invisible to mdb/ph,
 * which run as child processes and only inherit exported environment
 * variables. install_pulse() already knows the hub host at install time
 * (it just used it to configure pharos-pulse.service) - this plan persists
 * that already-known value so mdb/ph work with zero shell configuration.
 * * Traceability:
 * Panel review 2026-08-04 (Kathy Sierra, Seth Godin, Robert Martin, Martin
 * Fowler, Kent Beck, Ubuntu System Specialist, Senior DevSecOps Specialist),
 * following a live user report reproducing Issue #183's exact failure mode.
 * ======================================================================== */

# Plan: persist the hub host at install time so mdb/ph work without shell exports

## Background (already established — do not re-litigate)

- `mdb`'s (and `ph`'s, identical logic) current host resolution
  (`mdb/src/main.rs:108-114`, unchanged by this plan):
  ```rust
  let addr = if let Ok(server) = env::var("PHAROS_SERVER") {
      server
  } else {
      let host = env::var("PHAROS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
      let port = env::var("PHAROS_PORT").unwrap_or_else(|_| "2378".to_string());
      format!("{}:{}", host, port)
  };
  ```
- `install_pulse()` (`scripts/install.sh`) already computes the fully-resolved
  `${host}` (host:port form) before this plan's change point, and already uses
  it to write `Environment=PHAROS_SERVER=${host}` into `pharos-pulse.service`.
  That same value has simply never been persisted anywhere `mdb`/`ph` (run
  interactively by the operator, not by the pulse service) can find it.
- `crates/pharos-cli-support` already exists (Issue #185's dedup follow-up),
  depended on by `mdb` and `ph` only — the natural, already-established home
  for a new shared helper, avoiding reintroducing the exact duplication
  problem that crate was created to fix.

## The change

### 1. `scripts/install.sh`: write a small config file during `install_pulse()`

Immediately after the existing `Environment=PHAROS_SERVER=${host}` write to the
systemd unit (same `${host}` variable, no recomputation), write:

```bash
    ${SUDO} mkdir -p "${PHAROS_DIR}"
    echo "PHAROS_SERVER=${host}" | ${SUDO} tee "${PHAROS_DIR}/client.conf" > /dev/null
    ${SUDO} chmod 644 "${PHAROS_DIR}/client.conf"
```

- `${PHAROS_DIR}` is already `/etc/pharos` (existing constant, unchanged).
- World-readable (`644`) is correct and intentional: this file holds only a
  hostname:port, not a secret — any local user running `mdb`/`ph` needs to
  read it, unlike the `600`-permissioned private keys elsewhere in this file.
- Always written (not conditional on whether env vars are already set) —
  the file is purely a fallback of last resort; env vars still take
  precedence when present, so nothing changes for an operator who already
  exports them or manages multiple hubs from one machine.
- Idempotent: overwritten on every `install_pulse()` run, matching how the
  rest of this function already behaves on re-installs.
- Scope: `install_pulse()` only. Do **not** add this to `install_server()`
  (hub role) — the existing hardcoded `127.0.0.1` default already correctly
  handles running `mdb`/`ph` locally on the hub itself; a config file there
  would be redundant, not a gap.

### 2. `crates/pharos-cli-support`: add the read-side fallback

Add one new function, `read_configured_server() -> Option<String>`, that
reads `/etc/pharos/client.conf` (path via a constant, not hardcoded inline)
and returns the value of a `PHAROS_SERVER=` line if present and non-empty.
Missing file, unreadable file, or missing/malformed line all resolve to
`None` (fall through silently — this is a convenience fallback, never a hard
requirement, so it must never error or panic).

### 3. `mdb/src/main.rs` and `ph/src/main.rs`: extend the resolution chain

Add the new fallback as the *last* step, after `PHAROS_HOST`/`PHAROS_PORT`,
before the hardcoded `127.0.0.1` default:

```rust
let addr = if let Ok(server) = env::var("PHAROS_SERVER") {
    server
} else if let Ok(host) = env::var("PHAROS_HOST") {
    let port = env::var("PHAROS_PORT").unwrap_or_else(|_| "2378".to_string());
    format!("{}:{}", host, port)
} else if let Some(server) = pharos_cli_support::read_configured_server() {
    server
} else {
    "127.0.0.1:2378".to_string()
};
```

Precedence, most to least specific, all preserved/extended rather than
reordered: `PHAROS_SERVER` env var → `PHAROS_HOST`/`PHAROS_PORT` env vars →
`/etc/pharos/client.conf` → hardcoded `127.0.0.1:2378` default.

## Non-goals (do not touch)

- **Do not** touch `crates/pharos-client` or `crates/pharos-pulse` — this is
  purely an `mdb`/`ph` convenience; the pulse agent already gets its host
  explicitly via its own systemd unit's `Environment=` line, unaffected.
- **Do not** change `install_server()` (hub role) — out of scope, see above.
- **Do not** make the config file's presence a hard requirement anywhere —
  every existing invocation style (env vars set, or nothing set at all on a
  hub defaulting to localhost) must keep working exactly as it does today.
- **Do not** put anything sensitive in this file — host:port only.

## Verification steps (concrete, live)

1. Run the actual `install_pulse()` path (real or extracted-function test, per
   this repo's established convention) against a remote host argument in
   Podman; confirm `/etc/pharos/client.conf` is created with the exact
   expected `PHAROS_SERVER=host:port` line and `644` permissions.
2. In a **completely clean shell environment** (no `PHAROS_SERVER`,
   `PHAROS_HOST`, or `PHAROS_PORT` set at all — the exact scenario that broke
   for the live user), run `mdb`/`ph` and confirm they resolve the host from
   `/etc/pharos/client.conf` and attempt to connect to the right address
   (verify via a real disposable hub in Podman, confirming actual successful
   connection/query, not just a resolved string).
3. Confirm precedence is unchanged: with `PHAROS_SERVER` (and separately,
   `PHAROS_HOST`) set to a *different* address than what's in
   `client.conf`, confirm the env var wins.
4. Confirm graceful fallback: with no env vars set and no `client.conf` file
   present at all (e.g. a `toolbelt`-only install), confirm `mdb`/`ph` still
   default to `127.0.0.1:2378` exactly as today, with no error/panic from the
   new read path.
5. Re-run `install_pulse()` a second time (upgrade/re-run scenario) and
   confirm `client.conf` is cleanly overwritten, not duplicated/corrupted.
6. `cargo test --workspace` passes; new pure-logic tests for
   `read_configured_server()` in `crates/pharos-cli-support` cover: valid
   file, missing file, empty file, file present but no matching line.

## Report back

State clearly: the exact diff (`scripts/install.sh`, `crates/pharos-cli-support`,
`mdb/src/main.rs`, `ph/src/main.rs`), results of all 6 verification steps, and
explicit confirmation `crates/pharos-client`, `crates/pharos-pulse`, and
`install_server()` were not touched. Do not commit or push — this repo
requires explicit instruction for that, every time.
