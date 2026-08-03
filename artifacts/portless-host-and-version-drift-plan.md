/* ========================================================================
 * Project: pharos
 * Component: Documentation & UX
 * File: portless-host-and-version-drift-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Two independent bugs found live-testing v1.10.4 on the Implementation
 * team's own machine: (1) a portless PHAROS_SERVER value (the exact form
 * install.sh's own example usage shows) makes pharos-pulse's TcpStream::connect
 * fail permanently with "invalid socket address" - a severe, previously
 * undiscovered connectivity bug, not cosmetic; (2) crates/pharos-pulse/Cargo.toml's
 * version has drifted stale again (stuck at 1.10.1 through v1.10.2/.3/.4),
 * despite this exact recurrence being flagged as a risk the first time it was
 * fixed. This plan fixes both, and this time actually prevents the second
 * one from recurring a 5th time via a small helper script instead of relying
 * on a human remembering.
 * * Traceability:
 * Found live-testing v1.10.4 (2026-08-03). The portless-host bug traces back
 * to the TLS-trust-probe fix (v1.10.3), which added port-defaulting for its
 * own probe copy of the host value but didn't apply it to the actual
 * PHAROS_SERVER value used by the real pulse agent.
 * ======================================================================== */

# Plan: fix portless PHAROS_SERVER connectivity failure + prevent version drift

## Part 1: portless-host connectivity bug (severe, fix first)

### Background

Live evidence: `curl ... | bash -s -- node pharos-01.iamrichardd.com` (no port — exactly the form
`install.sh`'s own example comment shows, `install.sh -- node 192.168.1.5`). Journal showed:

```
Waiting for pharos-server at pharos-01.iamrichardd.com: invalid socket address (Retrying in 1s)
```

...repeating forever. Root cause, confirmed against the actual code this session:
`tokio::net::TcpStream::connect()` (`crates/pharos-pulse/src/main.rs:175`, unmodified, correct code)
requires a full `host:port` string — a bare hostname with no colon fails immediately with exactly
this error, and no amount of retrying fixes a fundamentally unparseable address.

The actual bug is in `scripts/install.sh`'s `install_pulse()` (quoted verbatim, re-read fresh this
session):

```bash
    local host="${host_arg:-${PHAROS_HOST:-127.0.0.1:2378}}"
    if [[ ! "${host}" =~ ^[A-Za-z0-9.-]+(:[0-9]+)?$ ]]; then
        error "Invalid host value for Pharos Pulse: '${host}'"
    fi

    log "Installing Pharos Pulse Agent..."
    download_binary "pharos-pulse"

    log "Checking whether ${host} already presents a trusted certificate (e.g. a public CA like Let's Encrypt)..."
    ensure_openssl
    local ca_cert_line=""
    local host_only="${host%%:*}"
    local host_for_probe="${host}"
    [[ "${host}" == *:* ]] || host_for_probe="${host}:2378"
    if [[ "${host_only}" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        pulse_host_is_ip="yes"
    else
        pulse_host_is_ip="no"
    fi
    if echo | timeout 10 openssl s_client -connect "${host_for_probe}" -verify_hostname "${host_only}" -verify_return_error >/dev/null 2>&1; then
        pulse_ca_found="trusted"
    else
        ...
```

v1.10.3's TLS-trust-probe fix added `host_for_probe` — a port-defaulted *copy* of `host` — but only
used it for the `openssl` probe. The systemd unit further down still writes
`Environment=PHAROS_SERVER=${host}` using the **original, possibly-portless** `host` — so the probe
correctly defaults the port for its own connectivity check, while the real value handed to the
actual `pharos-pulse` binary does not. The probe passing ("already trusted, no CA configuration
needed") actively misled the operator into thinking everything was fine, while the real agent was
permanently broken underneath.

### The change

Apply the port-default to `host` itself, once, right after validation — so every subsequent use
(probe and the real `PHAROS_SERVER` value) is consistently correct, and delete the now-redundant
`host_for_probe` variable entirely (a simplification, not just a fix):

```bash
    local host="${host_arg:-${PHAROS_HOST:-127.0.0.1:2378}}"
    if [[ ! "${host}" =~ ^[A-Za-z0-9.-]+(:[0-9]+)?$ ]]; then
        error "Invalid host value for Pharos Pulse: '${host}'"
    fi
    [[ "${host}" == *:* ]] || host="${host}:2378"

    log "Installing Pharos Pulse Agent..."
    download_binary "pharos-pulse"

    log "Checking whether ${host} already presents a trusted certificate (e.g. a public CA like Let's Encrypt)..."
    ensure_openssl
    local ca_cert_line=""
    local host_only="${host%%:*}"
    if [[ "${host_only}" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        pulse_host_is_ip="yes"
    else
        pulse_host_is_ip="no"
    fi
    if echo | timeout 10 openssl s_client -connect "${host}" -verify_hostname "${host_only}" -verify_return_error >/dev/null 2>&1; then
        pulse_ca_found="trusted"
    else
        ...
```

Everything after this point in the function (the fallback branch, the systemd unit heredoc, the
`Next Steps` printing block) is completely unchanged — they already correctly reference `${host}`,
which now always has a port.

### Non-goals (Part 1)

- **Do not** touch `pharos-pulse`'s Rust code (`TcpStream::connect`, `wait_for_server`) — it's
  already correct; the bug is entirely in what `install.sh` hands it.
- **Do not** change the validation regex to *require* a port (rejecting portless input) — the
  existing behavior of accepting a portless host and defaulting it is correct and matches the
  file's own documented usage example; only the *application* of that default was incomplete.
- **Do not** touch the fallback message text, the bare-IP hint, or anything from the
  already-shipped fixes this session — this is purely fixing where the port-default gets applied.

## Part 2: version-drift prevention (recurring bug — fix the process, not just the symptom)

### Background

`crates/pharos-pulse/src/main.rs` prints its version via `env!("CARGO_PKG_VERSION")` (fixed
correctly once, v1.10.1). But `crates/pharos-pulse/Cargo.toml`'s `version` field has not been
touched since — confirmed this session: it still reads `1.10.1` while `scripts/install.sh`'s
`VERSION` has since moved to `1.10.4` across three more releases. This is the *exact* drift this
project's own history already flagged as a risk the first time it happened — flagging it again in
a commit message clearly isn't sufficient; a human reliably forgets a step nothing enforces.

### The change

**New file: `scripts/bump-version.sh`** — a small helper that updates every version-bearing file
in one command, replacing the manual `sed -i` step on `scripts/install.sh` alone that's been used
for every release this session:

```bash
#!/usr/bin/env bash
# ========================================================================
# Project: pharos
# Component: Release Tooling
# File: scripts/bump-version.sh
# Author: Richard D. (https://github.com/iamrichardd)
# License: AGPL-3.0 (See LICENSE file for details)
# * Purpose (The "Why"):
# Updates every version-bearing file in this repo together, so cutting a
# release never again means remembering to update N different places by
# hand - crates/pharos-pulse/Cargo.toml drifted stale for 3 releases in a
# row despite this exact risk being documented after the first time it
# happened.
# ========================================================================

set -euo pipefail

NEW_VERSION="${1:?Usage: scripts/bump-version.sh X.Y.Z}"
if [[ ! "${NEW_VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Invalid version: '${NEW_VERSION}' (expected X.Y.Z)" >&2
    exit 1
fi

sed -i "s/^VERSION=\".*\"/VERSION=\"${NEW_VERSION}\"/" scripts/install.sh
grep -q "^VERSION=\"${NEW_VERSION}\"\$" scripts/install.sh || {
    echo "Failed to update scripts/install.sh — expected line pattern not found." >&2
    exit 1
}

sed -i "s/^version = \".*\"/version = \"${NEW_VERSION}\"/" crates/pharos-pulse/Cargo.toml
grep -q "^version = \"${NEW_VERSION}\"\$" crates/pharos-pulse/Cargo.toml || {
    echo "Failed to update crates/pharos-pulse/Cargo.toml — expected line pattern not found." >&2
    exit 1
}

echo "Bumped to ${NEW_VERSION} in:"
echo "  scripts/install.sh"
echo "  crates/pharos-pulse/Cargo.toml"
echo ""
echo "Next: rebuild (cargo build -p pharos-server -p pharos-pulse, in Podman per this repo's"
echo "Zero-Host policy) to refresh Cargo.lock before committing."
```

Also `chmod +x scripts/bump-version.sh`.

**Why the `grep` checks after each `sed`, found during panel review**: a bare `sed -i` silently
does nothing if its pattern doesn't match (e.g. if either file's format ever changes slightly) —
exit code 0, no error, and the exact same silent-drift failure mode this script exists to prevent
would just resurface in a new shape (the script *appears* to succeed while quietly not updating
one of the files). Verifying the replacement actually landed, and failing loudly if not, matches
this project's own established "fail fast" principle (see `TODO.md`'s Phase 25) rather than
introducing a tool whose one job is "don't let this drift silently" that can itself drift
silently.

**Also**: run it once now to fix the current drift — `scripts/bump-version.sh 1.10.4` (matching
whatever `scripts/install.sh`'s `VERSION` is at the time this plan is executed; re-read it fresh,
don't assume `1.10.4` is still current).

### Non-goals (Part 2)

- **Do not** touch any other crate's `Cargo.toml` version (`pharos-server`, `ph`, `mdb`,
  `pharos-scan`, `pharos-client`, `pharos-console`) — same reasoning as the original version-string
  fix: none of them print a version string a user would see, only `pharos-pulse` does.
- **Do not** add `[workspace.package]` version inheritance or any CI-level enforcement — that's a
  bigger structural change (touches every crate's `Cargo.toml`) genuinely out of scope for this
  fix; the helper script is the proportionate fix for the actual problem (a human forgetting one of
  two files), not a workspace-wide refactor.
- **Do not** have this script invoke `cargo` itself (e.g. to refresh `Cargo.lock` automatically) —
  running cargo is a build action subject to this repo's Zero-Host policy; the script just does
  text replacement and reminds the operator to rebuild in Podman as a separate step.
- **Do not** change the release-cut sequence described elsewhere (this repo's own process
  documentation/memory) beyond substituting this one command for the old manual `sed -i` step —
  don't rewrite unrelated parts of that process.

## Verification steps (concrete)

**Part 1 (portless-host fix)**:
1. Extract `install_pulse` (per this repo's established convention — never `source
   scripts/install.sh` directly) with stubbed `download_binary`/`ensure_openssl`/`fetch_ca_via_ssh`/
   `activate_systemd_service`/`SUDO=""`/`PHAROS_DIR=<temp dir>`, call it with a **portless** host
   argument (e.g. `pharos-test.local`, no port), and confirm the resulting systemd unit content
   (captured via a stubbed `tee`, or by writing to a real temp file path) contains
   `Environment=PHAROS_SERVER=pharos-test.local:2378` — **with** the port — not the bare hostname.
2. Confirm the TLS probe still runs against the correctly-ported value (reuse this session's
   already-established disposable-container-with-a-real-cert approach if a live network check is
   wanted, or at minimum confirm the same `${host}` value is used for both the probe and the unit
   file by inspecting the code path — they must be identical now, not two separately-computed
   values that could drift apart from each other again).
3. Regression: confirm a host **with** an explicit port (e.g. `pharos-test.local:9999`) is left
   completely unchanged (the `[[ "${host}" == *:* ]] ||` guard must not clobber an explicit port).
4. Regression: confirm the no-argument/default case (`PHAROS_HOST` unset, no `host_arg`) still
   resolves to `127.0.0.1:2378` exactly as before (it already has a port, so the new line is a
   no-op for this case).

**Part 2 (version-drift script)**:
5. Run `scripts/bump-version.sh 9.9.9` (a throwaway test version, on a scratch copy of the repo or
   reverted immediately after) and confirm **both** `scripts/install.sh`'s `VERSION=` and
   `crates/pharos-pulse/Cargo.toml`'s `version =` end up `9.9.9`. Confirm it rejects a malformed
   version string (e.g. `scripts/bump-version.sh abc`) with a clear error and non-zero exit,
   without touching either file.
6. **Prove the fail-fast check is real, not decorative**: temporarily alter one file's line format
   so the `sed` pattern won't match (e.g. change `crates/pharos-pulse/Cargo.toml`'s line to
   `version="1.10.1"` with no spaces, breaking the exact pattern the script expects), run the
   script, and confirm it exits non-zero with the "Failed to update..." error instead of silently
   reporting success — this is the exact silent-failure mode the grep check exists to catch, so
   prove it actually catches it. Revert the deliberate format change afterward.
7. Then run it for real with whatever the actual next version should be (re-read
   `scripts/install.sh`'s current `VERSION=` fresh at execution time to know what "the current
   drifted-behind value" actually is — don't assume `1.10.1`, confirm it directly) — this is the
   one-time fix for the current drift, using the new script rather than a manual `sed`.

## Report back

State clearly: the exact diff for all three touched files (`scripts/install.sh`,
`crates/pharos-pulse/Cargo.toml`, new `scripts/bump-version.sh`), results of all 6 verification
steps, and confirmation no other file was touched. Do not commit or push — this repo requires
explicit instruction for that, every time.
