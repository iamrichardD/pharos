/* ========================================================================
 * Project: pharos
 * Component: CLI-mdb, CLI-ph, Shared CLI Support
 * File: mdb-ph-debug-mode-and-glob-warning-plan.md
 * Author: Richard D. (https://github.com/iamrichardD/pharos)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * An extended live investigation into an apparent mdb query bug turned out
 * to be entirely a shell-quoting mistake (unquoted `mdb *` expanded by bash
 * into every filename in the operator's current directory before mdb ever
 * saw a literal `*`). Two real gaps this surfaced: (1) mdb/ph give zero
 * visibility into what they actually sent/received, forcing a slow,
 * manual cross-reference with server-side logs to diagnose; (2) a common,
 * easy-to-make shell mistake silently sent an operator's full directory
 * listing (including sensitive filenames) to the hub and into its
 * plaintext, indefinitely-retained journalctl log, with zero warning.
 * * Traceability:
 * Issues #192 (debug mode) and #193 (glob-expansion warning), found live
 * 2026-08-05, panel-reviewed.
 * ======================================================================== */

# Plan: mdb/ph debug mode + shell-glob-expansion warning

## Background (already established — do not re-litigate)

- `mdb`/`ph`'s command-construction flow (both files, same shape):
  `cli.query: Vec<String>` (raw argv tokens, already shell-split) →
  `pharos_client::join_wire_args(&cli.query)` (re-quotes tokens containing
  whitespace) → command classification (`add`/`change`/.../`query <text>`
  fallback) → `pharos_cli_support::resolve_server_address()` →
  `execute_authenticated()`.
- `crates/pharos-cli-support` already exists, depended on by `mdb`/`ph`
  only — the correct home for both new shared pieces of logic below, to
  avoid reintroducing the exact duplication problem that crate was created
  to fix.
- `PharosResponse` already derives `Debug` — no changes needed there to
  print a raw response.
- `resolve_server_address()` already returns `(String, &'static str)` —
  the address and its source are already available at the exact point
  debug output needs them.

## The change

### 1. `crates/pharos-cli-support`: shared glob-expansion heuristic

Add one new function:

```rust
/// Heuristic only, never blocks execution: if a query looks like it might be an
/// unquoted shell glob that expanded into a directory listing (many bare tokens,
/// none in field=value form), warn to stderr so the operator notices before
/// assuming a clean "no matches" is a genuine empty result. A single correctly-
/// quoted `*` is exactly one token and never trips this.
pub fn warn_if_looks_like_glob_expansion(tokens: &[String]) {
    const THRESHOLD: usize = 15;
    if tokens.len() < THRESHOLD {
        return;
    }
    if tokens.iter().all(|t| !t.contains('=')) {
        eprintln!(
            "Warning: this command included {} arguments with no field=value pairs. If you meant to search with a wildcard (e.g. mdb '*'), make sure to quote it — otherwise your shell may have expanded it into a list of filenames from the current directory before mdb/ph ever saw it. Proceeding anyway.",
            tokens.len()
        );
    }
}
```

Add unit tests: a single-token `["*"]` (must NOT warn — the correctly-quoted
case), a small legitimate multi-token case below threshold (must not warn),
and a large all-bare-token case at/above threshold (must warn) — check via
capturing stderr or by extracting the boolean condition into its own
testable helper if capturing `eprintln!` output directly proves awkward
(prefer whichever keeps the test simple and deterministic).

### 2. `mdb/src/main.rs` and `ph/src/main.rs`: call the heuristic + add `--debug`

Call `pharos_cli_support::warn_if_looks_like_glob_expansion(&cli.query)`
once, right after argument parsing, before any command classification or
joining — using the raw, pre-join token list so token count and `=`
presence reflect exactly what the shell handed the process.

Add a new clap flag to each `Cli` struct:
```rust
/// Print the resolved server address/source, the exact wire command sent,
/// and the raw response received — off by default, zero output change
/// otherwise.
#[arg(long = "debug")]
debug: bool,
```

At the three relevant points already present in each file's `main()`, gate
new `eprintln!` calls on `if cli.debug`:
- Immediately after `resolve_server_address()`: print the resolved address
  and its source string.
- Immediately before sending (`execute_authenticated`/`execute`): print the
  exact `cmd_to_send` string.
- Immediately after receiving a response (success case): print the raw
  `PharosResponse` via `{:?}`.

## Non-goals (do not touch)

- **Do not** add an interactive confirmation prompt for the glob-warning
  case — deliberately deferred (a bigger design question per the panel
  review), this plan is the non-blocking warning only.
- **Do not** add server-side `WARN`-level logging for anomalous selection
  counts — also deliberately deferred, a separate concern from the client-
  side fix.
- **Do not** touch `crates/pharos-client` or `crates/pharos-pulse` —
  `PharosResponse`'s existing `Debug` derive is sufficient; no changes
  needed there.
- **Do not** change any existing output when `--debug` is not passed, or
  the exit code / success-vs-failure behavior of any existing command.

## Verification steps (concrete, live)

1. Real disposable hub in Podman. Run `mdb '*'` (correctly quoted, single
   token) and confirm no glob-expansion warning appears.
2. In a directory containing 15+ files, run `mdb *` **unquoted** and
   confirm the warning appears on stderr, the command still proceeds
   (doesn't block/error out), and normal stdout output is unaffected.
3. Run a normal, small, legitimate multi-token or `field=value` query and
   confirm no warning appears (no false positive on ordinary usage).
4. Run `mdb --debug hostname=some-host` against a real hub and confirm all
   three debug lines appear (resolved address+source, wire command sent,
   raw response) and that they land on stderr, not stdout (so piping
   normal output is unaffected). Confirm `ph --debug ...` does the same.
5. Run the same command **without** `--debug` and confirm zero behavior/
   output difference from before this change.
6. `cargo test --workspace` passes, including the new
   `warn_if_looks_like_glob_expansion` unit tests.
7. Clean up all disposable test containers.

## Report back

State clearly: the exact diff (`crates/pharos-cli-support`, `mdb/src/main.rs`,
`ph/src/main.rs`), results of all 7 verification steps, and explicit
confirmation `crates/pharos-client` and `crates/pharos-pulse` were not
touched, and that omitting `--debug` produces byte-identical output to
before this change. Do not commit or push — this repo requires explicit
instruction for that, every time.
