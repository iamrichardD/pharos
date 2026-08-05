/* ========================================================================
 * Project: pharos
 * Component: CLI-mdb, CLI-ph, Shared CLI Support
 * File: dedup-enforce-add-record-type-plan.md
 * Author: Richard D. (https://github.com/iamrichardd/pharos)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * The record-type-integrity fix (Issue #194) added `enforce_add_record_type`
 * and its helper `tokenize_cmd` to both mdb/src/main.rs and ph/src/main.rs
 * as byte-for-byte identical duplicates (confirmed via diff). This project
 * already has crates/pharos-cli-support specifically to prevent this class
 * of duplication between the two CLIs — that's why it exists. Move both
 * functions there, matching the established pattern.
 * * Traceability:
 * Panel-reviewed follow-up to Issue #194 (record type integrity).
 * ======================================================================== */

# Plan: move enforce_add_record_type/tokenize_cmd into pharos-cli-support

## The change

1. In `crates/pharos-cli-support/src/lib.rs`, add `pub fn tokenize_cmd(line: &str) -> Vec<String>`
   and `pub fn enforce_add_record_type(cmd_str: &str, expected_type: &str, cli_name: &str) -> String`
   — move the two functions verbatim from `mdb/src/main.rs` (currently lines 229-289),
   including their doc comments if any. Keep the existing function bodies unchanged;
   this is a pure move, not a rewrite.

2. Move their unit tests too (`mdb/src/main.rs`'s `#[cfg(test)]` cases for these two
   functions, and `ph/src/main.rs`'s equivalent cases — check both, they may not be
   identical to each other since each file only tests its own expected_type) into
   `pharos-cli-support/src/lib.rs`'s existing `#[cfg(test)] mod tests`.

3. Delete both function definitions and their local tests from `mdb/src/main.rs` and
   `ph/src/main.rs`. Replace the call site in each (`mdb/src/main.rs:134`,
   `ph/src/main.rs:148`) with `pharos_cli_support::enforce_add_record_type(...)` —
   same arguments, same call shape, just qualified.

4. Confirm both `mdb` and `ph` already depend on `pharos-cli-support` in their
   Cargo.toml (they should — `warn_if_looks_like_glob_expansion` from the prior fix
   already comes from there). Do not add a new dependency edge to
   `pharos-client` or `pharos-pulse`.

## Non-goals

- Do not change behavior at all — this is a pure move/dedup, no logic changes.
- Do not touch `crates/pharos-client` or `crates/pharos-pulse`.
- Do not touch anything in `pharos-server`.

## Verification steps

1. `cargo build --workspace` and `cargo test --workspace` both pass, run inside the
   project's Podman build container (not on the host).
2. `grep -n "fn enforce_add_record_type\|fn tokenize_cmd" mdb/src/main.rs ph/src/main.rs`
   returns nothing (both fully removed from both files).
3. Confirm mdb and ph still correctly force type=machine/type=person on `add` with no
   explicit type, and still override+warn on a conflicting explicit type — same
   behavior as before, now backed by the shared implementation. A couple of direct
   unit-test-level checks are sufficient; no need to spin up a live server for a pure
   refactor.

## Report back

State clearly the exact diff (all touched files) and confirm `cargo test --workspace`
passes. Do not commit or push — this repo requires explicit instruction for that,
every time.
