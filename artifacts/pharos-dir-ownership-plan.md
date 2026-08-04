/* ========================================================================
 * Project: pharos
 * Component: Installer
 * File: pharos-dir-ownership-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * install_server() never chowns/chmods PHAROS_DIR (/etc/pharos) itself —
 * only its certs/ and keys/ subdirectories. It's left root:root 755 from
 * a plain `mkdir -p` run as root. pharos-server's atomic persistence
 * (persist_to_disk_atomic: write a temp file, then rename() over the
 * target) needs write access on the CONTAINING directory, not just on
 * data.json itself — so the pharos user (User=pharos in the systemd
 * unit) can never create that temp file, and every persistence attempt
 * silently fails with "Permission denied", forever, on every hub install.
 * * Traceability:
 * Found live in production 2026-08-04 by the Implementation Team while
 * migrating hub state to durable storage (data.json had never once
 * persisted since setup, despite pulse registering repeatedly).
 * ======================================================================== */

# Plan: chown/chmod PHAROS_DIR itself in install_server()

## Background (already established — do not re-litigate)

- `pharos-server/src/storage.rs`'s `persist_to_disk_atomic()` writes a temp
  file alongside the target path and calls `std::fs::rename(tmp_path, path)`
  — both operations require write permission on the *containing directory*
  (`/etc/pharos`), not merely on `data.json` itself.
- `install_server()` (`scripts/install.sh`) already chowns/chmods its
  subdirectories correctly (`${PHAROS_DIR}/certs` → `root:pharos 750`,
  `${PHAROS_DIR}/keys` → `pharos:pharos 700`, confirmed at lines 376-377)
  but never touches `${PHAROS_DIR}` itself anywhere in the script — it's
  left as whatever a plain `${SUDO} mkdir -p` produces (`root:root`,
  default umask, typically `755`), which is NOT writable by the
  `pharos-server` process (`User=pharos`/`Group=pharos` in its own
  systemd unit).
- `pharos:pharos 700` (the exact convention already used for
  `${PHAROS_DIR}/keys`) is sufficient and consistent: the `pharos-server`
  process runs *as* that same identity, so owner-only permissions are all
  it needs to traverse into `certs/`/`keys/` beneath it; any operator
  action via `sudo` bypasses permission checks entirely regardless of this
  directory's mode, so nothing else is affected.

## The change

In `install_server()` (`scripts/install.sh`), add ownership/permissions on
`PHAROS_DIR` itself. Place this right after `ensure_system_user` and before
`setup_pki()` is called (so the directory already has correct ownership
before anything is written into it), or immediately after `setup_pki()` —
either is fine since `chown`/`chmod` on the directory itself doesn't affect
already-set subdirectory permissions (confirmed: `chown`/`chmod` without
`-R` only touches the directory's own metadata, not its children's):

```bash
    ${SUDO} mkdir -p "${PHAROS_DIR}"
    ${SUDO} chown pharos:pharos "${PHAROS_DIR}"
    ${SUDO} chmod 700 "${PHAROS_DIR}"
```

This must run *before* `PHAROS_STORAGE_PATH` (`${PHAROS_DIR}/data.json`) is
ever written to — i.e., before `activate_systemd_service "pharos-server"`
starts the service for the first time — so a fresh install never hits this
bug even once.

## Non-goals (do not touch)

- **Do not** touch `install_pulse()` — pulse doesn't persist a `data.json`
  of its own; this is a `pharos-server` (hub) concern only.
- **Do not** change the existing, already-correct `certs/`/`keys/`
  subdirectory ownership/permissions — verify explicitly (per Verification
  step 2) that they remain exactly as they are today, unaffected by the
  new parent-directory chown/chmod.
- **Do not** touch `pharos-server/src/storage.rs` — the atomic-write
  pattern itself is correct and doesn't need to change; the fix is purely
  about the directory permissions the installer sets up for it.

## Verification steps (concrete, live)

1. Run the real `install_server()` path (hub target) in Podman with a real
   Ed25519 key setup, real TLS cert generation, and the real `pharos-server`
   binary actually started as `User=pharos` (matching production exactly —
   do not skip to just checking permissions statically). Add a record via
   `mdb add` or equivalent, and confirm `data.json` actually appears on
   disk with the added record's content — the concrete, end-to-end proof
   this bug is fixed, not just that permissions look right on paper.
2. Confirm `${PHAROS_DIR}/certs` and `${PHAROS_DIR}/keys` still show their
   existing, correct ownership/permissions (`root:pharos 750` and
   `pharos:pharos 700` respectively) — unaffected by the new parent-level
   chown/chmod.
3. Confirm `${PHAROS_DIR}` itself is `pharos:pharos 700` after a fresh
   install.
4. Re-run `install_server()` a second time (upgrade/re-run scenario) and
   confirm ownership/permissions are idempotently reapplied without error
   (chown/chmod on an already-correct target is a safe no-op).
5. `cargo test --workspace` passes (unaffected by this bash-only change,
   but confirms nothing else broke).
6. Clean up all disposable test containers.

## Report back

State clearly: the exact diff (`scripts/install.sh` only), results of all 6
verification steps (especially #1 — a real persisted `data.json` on disk,
proving the actual production bug is fixed), and explicit confirmation
`certs/`/`keys/` subdirectory permissions are unchanged and
`install_pulse()`/`pharos-server/src/storage.rs` were not touched. Do not
commit or push — this repo requires explicit instruction for that, every
time.
