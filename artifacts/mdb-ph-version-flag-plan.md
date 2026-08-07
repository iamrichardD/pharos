/* ========================================================================
 * Project: pharos
 * Component: CLI-mdb, CLI-ph
 * File: mdb-ph-version-flag-plan.md
 * Author: Richard D. (https://github.com/iamrichardd/pharos)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * TODO.md Debt #35 (Issue #182): mdb/ph don't support --version — neither
 * Cli struct sets clap's `version` attribute. Add it to both.
 * ======================================================================== */

# Plan: `mdb --version` / `ph --version` support

## The change

In both `mdb/src/main.rs` and `ph/src/main.rs`, add `#[command(version)]`
to the `Cli` struct's existing `#[command(...)]` attributes (alongside
`name`/`about`), e.g.:

```rust
#[derive(Parser)]
#[command(name = "mdb")]
#[command(about = "Pharos Machine Database (MDB) CLI", long_about = None)]
#[command(version)]
struct Cli {
```

This pulls `CARGO_PKG_VERSION` automatically via clap's built-in mechanism
— the same source of truth `scripts/bump-version.sh` already keeps in sync
across the workspace, and the same mechanism `pharos-pulse`'s own startup
banner already uses (`env!("CARGO_PKG_VERSION")`). No other change needed.

## Non-goals

- Do not touch `pharos-pulse`, `pharos-scan`, or `pharos-server` — they
  already report their version elsewhere (startup banner / `siteinfo`).
- Do not add a custom version string format — clap's default
  (`mdb 1.10.20`) is sufficient and consistent with standard CLI tooling.

## Verification steps (concrete, live)

1. Build both binaries; confirm `mdb --version` and `ph --version` each
   print the correct current workspace version (matching
   `scripts/install.sh`'s `VERSION=`) and exit 0.
2. Confirm existing behavior (query/add/change/etc., `--help`) is
   completely unaffected.
3. `cargo test --workspace` passes.

## Report back

State clearly: the exact diff (both files), the live `--version` output
from both binaries, and confirmation `cargo test --workspace` passes. Do
not commit or push — this repo requires explicit instruction for that,
every time.
