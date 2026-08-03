/* ========================================================================
 * Project: pharos
 * Component: Documentation & UX
 * File: pulse-version-string-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Self-contained implementation plan for Gap 3 of the node-install UX
 * investigation: pharos-pulse prints a hardcoded, stale version string on
 * startup that matches neither its own Cargo.toml version nor the actual
 * release version a user just installed.
 * * Traceability:
 * Related to the node-install UX panel investigation (2026-08-02).
 * ======================================================================== */

# Plan: fix pharos-pulse's stale/hardcoded version string (Gap 3)

## Background (verified against current code, not assumed)

A live install test found: a fresh `v1.9.0` install (per `scripts/install.sh`'s `VERSION="1.9.0"`,
confirmed current at `scripts/install.sh:19`) produces a `pharos-pulse` binary that prints
`Starting pharos-pulse agent v1.3.1...` on startup — a number that matches neither the release
just installed (1.9.0) nor even its own crate's `Cargo.toml` (0.1.0). Three different numbers are
in play. Confidence-damaging even though functionally harmless: an operator reading their own
startup logs has no way to tell if this is a real version mismatch or just cosmetic.

Root cause, quoted verbatim:

- `crates/pharos-pulse/src/main.rs:27`:
  ```rust
      println!("Starting pharos-pulse agent v1.3.1...");
  ```
  A literal string, not derived from any build metadata.

- `crates/pharos-pulse/Cargo.toml:14-17`:
  ```toml
  [package]
  name = "pharos-pulse"
  version = "0.1.0"
  edition = "2021"
  ```

- Confirmed (root `Cargo.toml`, `crates/pharos-pulse`/other crates): there is no
  `[workspace.package]` version section anywhere in this workspace, so no crate uses version
  inheritance — each crate's `version` field is standalone and currently meaningless busywork,
  since these binaries are never published to crates.io.
- Confirmed (this project's release-cut process, see `AGENTS.md`/release-cut protocol): cutting a
  release only ever bumps `scripts/install.sh`'s `VERSION=` — no step bumps any crate's
  `Cargo.toml` version. So today, `Cargo.toml` versions and the actual release tag are two
  entirely disconnected numbers, and always will be unless something is changed here.

## The change

**1. `crates/pharos-pulse/src/main.rs:27`** — replace the hardcoded literal with the crate's real,
build-time version:

```rust
    println!("Starting pharos-pulse agent v{}...", env!("CARGO_PKG_VERSION"));
```

**2. `crates/pharos-pulse/Cargo.toml`** — bump `version = "0.1.0"` to `version = "1.9.0"` (the
*current* `scripts/install.sh` `VERSION` value at time of this fix), so the printed string is
accurate today:

```toml
[package]
name = "pharos-pulse"
version = "1.9.0"
edition = "2021"
authors = ["Richard D. <https://github.com/iamrichardd>"]
```

This is a one-time correction, not an automated sync — see Non-goals below for why building
CI automation to keep these numbers in lockstep going forward is explicitly out of scope for this
fix, and see "Follow-up worth flagging" for what to do about that separately.

## Non-goals (do not touch)

- **Do not** add a `[workspace.package]` version section, workspace-level version inheritance, or
  any CI/release-process step that automatically bumps `Cargo.toml` versions on release. That is a
  legitimate bigger idea (a real fix for *future* drift) but is a separate, larger scope decision
  involving the release-cutting process itself — do not fold it into this narrow fix.
- **Do not** touch any other crate's `Cargo.toml` version (`pharos-server`, `ph`, `mdb`,
  `pharos-scan`, `pharos-client`, `pharos-console`) — every one of them has the exact same
  "disconnected from the release tag" property, and none of them were reported as printing a
  visibly wrong version to a user (most don't print a version string at all, per this session's
  research — only `pharos-pulse` does). Fixing this crate's user-visible symptom does not require
  touching the others.
- **Do not** touch Gap 1 (`scripts/install.sh`'s new `--fetch-ca-ssh` flag) or Gap 2
  (`crates/pharos-pulse/src/main.rs`'s baseline-retry logic, lines 69-98 and the
  `send_baseline_until_success` function around lines 186-210) — both already shipped as separate,
  already-verified fixes. Line 27 is far from that code; touch only the one line plus the
  `println!` format-string change.
- **Do not** change `scripts/install.sh`'s `VERSION=` value — it is already correct (`"1.9.0"`) and
  is not part of this bug.

## Verification steps (concrete)

All build/test/lint runs happen in Podman per this repo's Zero-Host policy:

```bash
podman build -t pharos-test -f Containerfile.test .
podman run --rm pharos-test
```

Must include, at minimum:
1. `cargo build -p pharos-pulse` succeeds with no new warnings.
2. `cargo test --workspace -p pharos-pulse --bin pharos-pulse` passes — all existing tests
   (`test_should_collect_inventory_fields_when_invoked`,
   `test_should_format_presence_command_correctly_when_inventory_provided`, and Gap 2's
   `test_should_increase_delay_exponentially_up_to_limit_when_calculating_backoff`) must still
   pass unmodified; none of them touch the changed line.
3. **Live verification (panel-review stage, not builder)**: build the actual `pharos-pulse` binary
   and run it (even briefly, against any reachable/unreachable server address — the version line
   prints before any server connectivity is established, at the very first line of `main()`),
   confirm the startup output now reads `Starting pharos-pulse agent v1.9.0...` — i.e. actually
   run the compiled binary and read its real stdout, don't just trust that `env!("CARGO_PKG_VERSION")`
   compiles; confirm what it *actually prints* matches the new `Cargo.toml` version exactly.

## Follow-up worth flagging (do not implement as part of this fix — just note it)

Because release-cutting only bumps `scripts/install.sh`'s `VERSION=` and nothing else, this exact
one-line `Cargo.toml` bump will silently go stale again the next time a release is cut, unless
whoever cuts the release remembers to also bump `crates/pharos-pulse/Cargo.toml`. That's worth a
`TODO.md` backlog entry (per this project's TODO/GitHub-issue-sync discipline) to either (a) add a
reminder step to the release-cutting sequence, or (b) do the bigger workspace-version-inheritance
fix mentioned in Non-goals — but that's a decision for whoever reviews this fix to make
separately, not something to implement here.

## Report back

State clearly: build/test result (pass/fail + exact command output), the *actual* runtime stdout
line printed by the rebuilt binary (not just the source diff), and confirm no file other than
`crates/pharos-pulse/src/main.rs` (line 27 only) and `crates/pharos-pulse/Cargo.toml` (the version
field only) was changed. Do not commit or push — this repo requires explicit instruction for that,
every time.
