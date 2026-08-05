/* ========================================================================
 * Project: pharos
 * Component: Web Console, Server Core, CI
 * File: console-version-self-report-plan.md
 * Author: Richard D. (https://github.com/iamrichardd/pharos)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * The Implementation Team found `pharos-console-web` running v1.10.11 while
 * the rest of the fleet was on v1.10.14 — the systemd unit only pulls
 * `:latest` when there's no local image cache at all, so it silently never
 * moved after a migration wiped that cache once. Panel-reviewed conclusion:
 * don't build a parallel Terraform-side version checker. Pharos already IS
 * the fleet's source of truth and already has a "Dead Man's Switch" staleness
 * alerting mechanism (`pharos-server/src/alerting.rs`) — extend that same
 * machinery to also catch version drift, instead of inventing a second one.
 * * Traceability:
 * Panel-reviewed (Kent Beck, Robert Martin, Martin Fowler, Kathy Sierra,
 * Seth Godin, Ubuntu System Specialist, Senior DevSecOps Specialist).
 * ======================================================================== */

# Plan: pharos-console-web self-reports its version; pharos-server alerts on drift

## Background (already established — do not re-litigate)

- `pharos-console-web/package.json`'s `"version": "1.3.0"` is the Astro app's
  own internal semver and is **not** what operators track — the container
  image tag actually deployed (`v1.10.14`) comes from the git release tag via
  `docker/metadata-action` in `.github/workflows/ci.yml` (`type=semver,pattern={{version}}`).
  Any self-reported version must come from the release tag, never from
  `package.json`.
- `pharos-server/src/alerting.rs` already implements a generic, tested
  pattern for this exact shape of problem: a pure filter function
  (`find_newly_stale`), a dedup-by-last-value `AlertState`, and generic
  `fire_webhook`/`fire_script` senders, called once per health-monitor tick
  from `main.rs`. Reuse this pattern, don't invent a parallel one.
- `pharos-console-web/src/lib/pharos.ts`'s `executePharosQuery(clientId, queryStr, host?, port?)`
  is the existing building block for sending an authenticated command to the
  hub — it already handles TLS, SSH-challenge signing, and reads
  `PHAROS_HOST`/`PHAROS_PORT`/`PHAROS_CA_CERT`/`PHAROS_PRIVATE_KEY` env vars
  the same way the rest of the ecosystem does.
- **Deliberately not building**: pharos-server does not fetch "the latest
  release" from GitHub itself (no new external dependency, no autonomous
  unreviewed network egress) and never auto-redeploys/auto-upgrades anything.
  `expected_version` is set only by an operator or by Terraform, the same
  place that already owns the pinned image tag — this stays alert-only, a
  human always makes the actual upgrade decision.
- **Also deliberately not part of this plan** (separate, Implementation-Team-side
  work, tracked as a companion issue in `iamrichardD/home_network`, not this
  repo): moving off the mutable `:latest` tag, pinning by digest, and any
  scheduled "file an Issue on drift" job. This plan only makes Pharos itself
  capable of *detecting and alerting on* version drift once it's told what
  to expect — it doesn't change how the Implementation Team pins or deploys
  images.

## The change

### 1. `pharos-console-web/Containerfile` + CI: know your own version at build time

Add `ARG PHAROS_VERSION=dev` to the Containerfile, and `ENV PHAROS_CONSOLE_VERSION=${PHAROS_VERSION}`
in the runtime stage. In `.github/workflows/ci.yml`'s `publish-images` job,
pass `build-args: PHAROS_VERSION=${{ github.ref_name }}` to the
`pharos-console-web` matrix entry's `docker/build-push-action` step (only
that entry needs this — the Rust binaries already get their version from
`CARGO_PKG_VERSION`/`scripts/bump-version.sh`). This ties the self-reported
version to the actual release tag, not `package.json`.

While touching this file: bump both `FROM public.ecr.aws/docker/library/node:22-slim`
lines (build stage and runtime stage) to `node:24-slim` — current Node LTS,
noticed as stale while working on this plan. Rebuild and run the full test
suite against the new base before treating this as done; a major Node bump
can change behavior, not just be a version-string edit.

### 2. `pharos-console-web`: self-register on startup, re-send periodically

New small module (e.g. `src/lib/selfReport.ts`), called once from
`server.mjs` at startup and then on an interval (mirror `pharos-pulse`'s
heartbeat cadence — every 60 minutes is fine, this only needs to keep
`last_seen_at` fresh, not report anything that changes more often):

- Identify itself via a `PHAROS_CONSOLE_HOSTNAME` env var (falls back to
  `os.hostname()` with a one-time startup warning that this may be a
  meaningless container ID unless explicitly set — containers get an opaque
  default hostname unless `podman run --hostname` or this env var is set).
- Send (via `executePharosQuery`, reusing the existing baked-in
  `keys/admin_id_ed25519` identity already used elsewhere in this app):
  ```
  add hostname="<name>" type=machine version="<PHAROS_CONSOLE_VERSION>" role="pharos-console-web"
  ```
  Use `add` unconditionally (not a manual existence check first) — the
  server's existing `upsert` semantics already handle the "record already
  exists" case by updating fields (including `last_seen_at`) rather than
  erroring, exactly like `pharos-pulse`'s own heartbeat pattern.
- If the initial connection at container startup fails (hub not up yet),
  retry a few times with backoff rather than crashing the console process —
  the console's own web UI must stay usable even if self-registration never
  succeeds.
- Log clearly on both success and failure — this must not become a second
  silent-failure trap.

### 3. `pharos-console-web/src/layouts/ConsoleLayout.astro`: show the version in the footer

The footer (currently a static tagline, no version) is the cheapest possible
way for a human glancing at the console to see its own version without
running an `mdb` query or waiting for a drift alert. Thread the same
`PHAROS_CONSOLE_VERSION` env var from step 1 through to the footer. Keep the
existing tagline; just append the version, using the exact copy:

```
Pharos v1.10.14
```

Fall back to a clearly-labeled placeholder (e.g. `dev`, matching the
Containerfile's `ARG PHAROS_VERSION=dev` default) rather than a blank string
if the env var is ever unset, so a misconfigured build is obvious rather than
silently missing.

### 4. `pharos-server/src/alerting.rs`: detect version mismatch, reuse existing alert senders

Add a second pure filter function alongside `find_newly_stale`, following
the exact same shape:

```rust
/// Returns machine records whose self-reported `version` field disagrees with
/// their own `expected_version` field (both must be present — a record with
/// only one or neither is not a mismatch, just not opted into this check),
/// and haven't already been alerted for this exact (version, expected_version)
/// pair.
pub fn find_version_mismatches<'a>(
    records: &'a [Record],
    alert_state: &AlertState,
) -> Vec<&'a Record> { ... }
```

- Add a second `HashMap<String, (String, String)>` field to `AlertState`
  (or a small sibling struct) for version-mismatch dedup, keyed by hostname
  → `(version, expected_version)` last alerted for — same reasoning as the
  existing staleness dedup: a record that gets its `expected_version` bumped
  (Terraform declares a new target) or its `version` bumped (console
  redeployed) must be able to re-alert.
- Reuse `fire_webhook`/`fire_script` — extend the webhook JSON payload with
  an `event` field (`"version_mismatch"` vs the existing `"node_down"`) and
  the relevant version strings, rather than adding a third bespoke sender.
- Call this from the same per-tick health-monitor loop in `main.rs` that
  already calls `check_presence` (Task 15.3), immediately after it — same
  `webhook_url`/`script_path` config, no new env vars needed for the
  webhook/script destination itself.

### 5. Documentation

- `website/src/content/docs/console.mdx`: document the self-registration
  behavior, the `PHAROS_CONSOLE_HOSTNAME` env var, and that this record's
  `version` field is what an operator compares against when deciding whether
  the console is current.
- Note (for the Implementation Team, via the companion `home_network` issue,
  not this repo): setting `expected_version` is a `mdb change hostname=<x>
  make expected_version=1.10.14` call, intended to run as part of whatever
  already bumps the pinned image tag in Terraform — the two values should
  change in the same commit/apply.

## Non-goals (do not touch)

- No GitHub API polling from `pharos-server` — `expected_version` is always
  operator/Terraform-set, never auto-discovered.
- No auto-upgrade, auto-restart, or auto-redeploy of anything — alert-only,
  exactly like the existing Dead Man's Switch.
- Does not touch `mdb`/`ph`/`pharos-pulse`/`pharos-scan`.
- Does not change the Implementation Team's actual deployment/pinning
  strategy (`:latest` vs. digest, systemd unit, Terraform) — that's the
  companion `home_network` issue, out of scope here.

## Verification steps (concrete, live)

1. Build `pharos-console-web`'s image locally with
   `--build-arg PHAROS_VERSION=v9.9.9-test` (Podman, per this repo's
   Zero-Host policy) and confirm `PHAROS_CONSOLE_VERSION=v9.9.9-test` is set
   at runtime (`podman exec ... env`). Confirm the image actually built on
   `node:24-slim` (`podman exec ... node --version` reports a v24.x runtime)
   and the full existing test suite (`npm test`/`vitest`) still passes
   unchanged on the new base.
2. Run that image against a real disposable `pharos-server` in Podman;
   confirm via a real `mdb` query that a machine record appears with
   `version=v9.9.9-test` and `role=pharos-console-web` within the retry
   window, with no manual step. Also load the console's own page in a
   browser/curl and confirm the footer renders "Pharos v9.9.9-test" — same
   build-arg, one visible confirmation point.
3. Kill/restart the console container; confirm the record's `last_seen_at`
   updates and `version` stays consistent (no duplicate record created).
4. `mdb change hostname=<console-host> make expected_version=v9.9.9-test`
   (matching) — confirm the health-monitor tick does **not** fire an alert.
5. `mdb change hostname=<console-host> make expected_version=v1.0.0-different`
   (mismatched) — confirm the webhook fires with `event=version_mismatch`
   and the correct version strings; confirm it does **not** re-fire on the
   next tick for the same unchanged pair (dedup working); confirm it fires
   again after changing either `version` or `expected_version` again.
6. Confirm the existing Dead Man's Switch (`check_presence`/`find_newly_stale`)
   tests and behavior are completely unaffected.
7. `cargo test --workspace` (server-side) and the console's own test suite
   (`npm test`/`vitest`, in Podman) both pass.
8. Clean up all disposable test containers.

## Report back

State clearly: the exact diff (all touched files, both the TypeScript and
Rust sides plus the CI workflow), results of all 8 verification steps, and
explicit confirmation `mdb`/`ph`/`pharos-pulse`/`pharos-scan` were not
touched. Do not commit or push — this repo requires explicit instruction for
that, every time.
