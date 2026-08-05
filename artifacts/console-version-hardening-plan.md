/* ========================================================================
 * Project: pharos
 * Component: Server Core, Web Console
 * File: console-version-hardening-plan.md
 * Author: Richard D. (https://github.com/iamrichardd/pharos)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Real production deployment of v1.10.15's console-version-self-report
 * feature (Issue #195) surfaced two gaps within hours of shipping: (1) an
 * exact-string version comparison that a `v`-prefix mismatch turns into a
 * permanent, silently-never-repeating false positive, and (2) a hostname
 * fallback default that deterministically collides with the same host's
 * own pulse-owned record for the common case of an operator who already
 * sets the container's hostname to match the physical host. Both fixes
 * make the footgun structurally harder to hit, rather than relying on
 * documentation alone.
 * * Traceability:
 * Found live 2026-08-05 during the Implementation Team's v1.10.15 rollout.
 * Panel-reviewed (Kent Beck, Robert Martin, Kathy Sierra, Seth Godin,
 * Senior DevSecOps Specialist).
 * ======================================================================== */

# Plan: harden version-comparison and hostname-fallback defaults

## Background (already established — do not re-litigate)

- `pharos-console-web` self-reports `version` with a `v` prefix (e.g.
  `v1.10.15`, from the `PHAROS_CONSOLE_VERSION` env var, itself derived from
  the git release tag). The container image tag itself has no `v` prefix.
  Both are correct in their own context — the mismatch is only a problem at
  the comparison site.
- `pharos-server/src/alerting.rs`'s `find_version_mismatches()` currently
  compares `version == expected_version` as an exact string match, with the
  result deduped by `AlertState.version_mismatches_alerted` keyed on the
  exact `(version, expected_version)` pair per hostname. A permanently
  mismatched pair (e.g. differing only by a `v` prefix) alerts exactly once
  and then never again, since the pair never changes — this reads as
  "resolved" to an operator watching for repeat alerts, when it actually
  means "will never be reported again."
- `pharos-console-web/src/lib/selfReport.ts`'s `getConsoleHostname()`
  currently falls back to bare `os.hostname()` when `PHAROS_CONSOLE_HOSTNAME`
  is unset. Confirmed live: an operator who sets `--hostname` on the
  container to match the physical host (common practice) gets an immediate,
  deterministic `511:Collision` against that host's existing
  `pharos-pulse`-owned record — not a rare misconfiguration, the expected
  outcome for anyone already following that practice.

## The change

### 1. `pharos-server/src/alerting.rs`: normalize version strings before comparing

Add a small pure helper:

```rust
/// Strips a single leading 'v'/'V' if present, so "v1.10.15" and "1.10.15"
/// compare equal. Used only for comparison — never changes what's stored
/// or displayed.
fn normalize_version(v: &str) -> &str {
    v.strip_prefix(['v', 'V']).unwrap_or(v)
}
```

Update `find_version_mismatches()` to compare
`normalize_version(version) == normalize_version(expected_version)` instead
of the raw fields directly. Do **not** change what's stored in
`AlertState.version_mismatches_alerted` (keep the raw, original strings) —
only the comparison itself is normalized, so the dedup key still reflects
exactly what was actually seen.

### 2. `pharos-console-web/src/lib/selfReport.ts`: collision-safe hostname default

Change `getConsoleHostname()`'s fallback from bare `os.hostname()` to
`${os.hostname()}-console` when `PHAROS_CONSOLE_HOSTNAME` is unset. Keep the
existing one-time startup warning (update its wording to mention the
`-console` suffix is being applied), and keep `PHAROS_CONSOLE_HOSTNAME` as a
full override for anyone who wants a custom name instead.

## Non-goals (do not touch)

- Do not change what `version`/`expected_version` actually store or display
  anywhere (`mdb` output, the footer, webhook payloads) — normalization is
  comparison-only.
- Do not add any collision pre-check or existence query before
  self-registering — the new default avoids the collision by construction
  for the common case; this plan doesn't attempt to solve every possible
  hostname collision, just the one already confirmed live.
- Do not touch `mdb`/`ph`/`pharos-pulse`/`pharos-scan`.

## Verification steps (concrete, live)

1. Unit test `normalize_version` directly: `"v1.10.15"` and `"1.10.15"` both
   normalize to `"1.10.15"`; a string with no `v` prefix is unchanged; an
   uppercase `V1.10.15` also normalizes correctly.
2. Unit test `find_version_mismatches` with `version="v1.10.15"`,
   `expected_version="1.10.15"` — must **not** be flagged as a mismatch
   (this is the exact scenario hit in production).
3. Confirm a genuine mismatch (e.g. `version="v1.10.15"`,
   `expected_version="v1.0.0"`) is still correctly flagged — the fix must
   not make real drift undetectable.
4. Unit test `getConsoleHostname()`: with `PHAROS_CONSOLE_HOSTNAME` unset,
   confirm the result is `<os.hostname()>-console`, not the bare hostname.
   With it set, confirm the explicit value is used unchanged.
5. Live end-to-end: real disposable `pharos-server` + a real
   `pharos-pulse`-style Machine record for hostname `test-host`, then start
   the console container with `PHAROS_CONSOLE_HOSTNAME` unset on the same
   container hostname (`test-host`) — confirm the console registers
   successfully as `test-host-console` with **no** `511:Collision`.
6. Live end-to-end: set `expected_version=1.10.15` (no `v` prefix) against a
   console self-reporting `version=v1.10.15` — confirm **no** webhook fires.
   Then set `expected_version=v2.0.0-different` — confirm the webhook
   **does** fire.
7. `cargo test --workspace` and the console's own test suite both pass.
8. Clean up all disposable test containers.

## Report back

State clearly: the exact diff (all touched files), results of all 8
verification steps (especially #2 and #5 — the exact production scenarios
this plan exists to fix), and explicit confirmation `mdb`/`ph`/
`pharos-pulse`/`pharos-scan` were not touched. Do not commit or push — this
repo requires explicit instruction for that, every time.
