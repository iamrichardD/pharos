/* ========================================================================
 * Project: pharos
 * Component: Documentation & UX
 * File: systemd-restart-on-reinstall-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * install.sh rewrites a service's systemd unit file (new binary, new
 * PHAROS_SERVER host, new CA config) on every run, but activate_systemd_service()
 * uses `systemctl enable --now`, which is a no-op for a service that's
 * already active - so none of that rewritten config, nor the freshly
 * downloaded binary, ever actually takes effect until something else
 * restarts the process. Caught live by the Implementation team: after
 * re-running install.sh with a new --fetch-ca-ssh/TLS-trust-probe fix and a
 * new hub hostname, pulse kept connecting to its OLD host (127.0.0.1,
 * config from 13 hours earlier) and hitting the OLD TLS failure, because
 * the already-running process was never told to restart.
 * * Traceability:
 * Found live-testing v1.10.3's TLS-trust-probe fix on the Implementation
 * team's own machine (2026-08-03). Affects install_server() and
 * install_pulse() equally, via the shared activate_systemd_service() helper.
 * ======================================================================== */

# Plan: restart (not just enable) on every install/reinstall

## Background

Live evidence (Implementation team, `rdelgadoXPS15`): ran
`install.sh -- node pharos-01.iamrichardd.com`. The installer correctly detected the hub's
Let's Encrypt cert as trusted (v1.10.3's new TLS-trust probe) and printed the right message. But
`systemctl status pharos-pulse` showed the service had been `active (running)` for **13 hours** —
i.e. it was never restarted by this run — and the journal showed it still connecting to
`127.0.0.1:2378` (a stale config from an earlier test), still hitting the exact `UnknownIssuer`
failure the fresh install should have resolved. The new binary was downloaded to
`/usr/local/bin/pharos-pulse` and the new unit file was written with the correct
`PHAROS_SERVER=pharos-01.iamrichardd.com`, but the *running process* never picked either up.

**Root cause** — `activate_systemd_service()` (quoted verbatim, `scripts/install.sh:36-39`,
re-read fresh this session):

```bash
activate_systemd_service() {
    local service_name=$1
    ${SUDO} systemctl daemon-reload
    ${SUDO} systemctl enable --now "${service_name}"
}
```

`systemctl enable --now X` enables the unit for boot (if not already) and starts it **only if not
already running** — for an already-active service, `--now` is a complete no-op on the running
process. Every `install.sh` run rewrites the entire unit file unconditionally (new
`ExecStart`/`Environment=` lines), so this bug means: **any reinstall or reconfiguration via
install.sh silently fails to apply anything to an already-running service** — not just this one
TLS scenario. This is called from exactly two places, both affected identically:
`install_server()` (`scripts/install.sh:369`, for `pharos-server`) and `install_pulse()`
(`scripts/install.sh:430`, for `pharos-pulse`).

## The change

**File: `scripts/install.sh`, `activate_systemd_service()` only:**

```bash
activate_systemd_service() {
    local service_name=$1
    ${SUDO} systemctl daemon-reload
    ${SUDO} systemctl enable "${service_name}"
    ${SUDO} systemctl restart "${service_name}"
}
```

`enable` (without `--now`) is idempotent and safe to run every time regardless of current state.
`restart` unconditionally applies the rewritten unit file and freshly downloaded binary — for a
not-yet-running service, `restart` behaves exactly like `start` (there's nothing to stop first),
so the never-installed-before case is unaffected; for an already-running service, it now actually
picks up whatever changed.

## Non-goals (do not touch)

- **Do not** try to detect whether the unit file's content actually changed before deciding to
  restart — this script already unconditionally rewrites the whole unit file on every run
  regardless of whether anything changed; adding diff-based change-detection here would be new
  scope beyond this fix. Always restarting is consistent with that existing "always regenerate"
  philosophy, not a new category of behavior.
- **Do not** add a restart-free reload path for `pharos-pulse` — unlike `pharos-server` (which has
  a SIGHUP-based reload for its TLS cert/key specifically, Debt #19/Issue #166), `pharos-pulse` has
  no such mechanism, and building one is unrelated, larger scope.
- **Explicitly accepted tradeoff, not a gap to close here**: this means re-running `install.sh`
  against an already-running `pharos-server` (the hub, potentially serving live clients) will now
  cause a brief restart every time, even if nothing meaningfully changed. That's judged acceptable
  — the alternative (silently not applying config/binary updates, which is the actual bug being
  fixed) is worse. If avoiding hub restarts on truly-no-op reinstalls is wanted later, that's a
  separate, bigger feature (config diffing), not part of this fix.
- **Do not** touch anything about the TLS-trust-probe logic, the CA-fallback message, or the
  console/Issue #180 fix — this is purely about the enable/restart mechanics, unrelated to what
  gets written into the unit file.

## Verification steps (concrete)

**Do not attempt to run a real systemd-as-PID-1 container for this** — already tried in this
session's own environment (`podman run --systemd=always ...`) and confirmed it doesn't work here
(`Failed to connect to bus: Host is down`, even with cgroup mounts and a systemd-installed image).
Don't spend time re-discovering this or trying alternate container/privilege flags — it's a
sandbox limitation, not something to work around for this fix. `systemctl enable`/`enable --now`/
`restart`'s documented semantics (a no-op restart-wise for an already-active unit under `--now`;
`restart` unconditionally reapplies, behaving like `start` when nothing was running) are
well-established, stable systemd behavior — not something this fix needs to independently
rediscover through a live system. What this fix's own code IS responsible for getting right is
simply: does `activate_systemd_service()` actually issue `enable` then unconditional `restart`
(not `enable --now`)? Verify that directly:

1. **Command-capture test**: stub `systemctl` as a function/script that just logs its arguments to
   a file instead of executing anything, run the fixed `activate_systemd_service "pharos-pulse"`
   (extracted per this repo's established testing convention, stubbed `SUDO=""`), and confirm the
   captured log shows, in order: `daemon-reload`, `enable pharos-pulse` (no `--now`),
   `restart pharos-pulse`. This directly proves the function issues the right commands — no
   `--now` flag anywhere, and `restart` is always called unconditionally regardless of any prior
   state (there's no branching in the fixed function on whether the service was already running,
   which is itself the point: it always restarts, unconditionally).
2. **Regression check against the current (buggy) version**: run the same command-capture test
   against the *current, unfixed* `activate_systemd_service` (`git show HEAD:scripts/install.sh` or
   just the in-memory pre-edit version) and confirm it captures `enable --now` instead — showing
   concretely what changed and why the old version could no-op on an already-running service.
3. Clean up any temp files created for the capture test.

## Report back

State clearly: the exact diff (`scripts/install.sh`'s `activate_systemd_service()` only), the
captured command sequences from both verification steps (before/after), and confirmation no other
file was touched. Do not commit or push — this repo requires explicit instruction for that, every
time.
