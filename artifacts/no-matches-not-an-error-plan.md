/* ========================================================================
 * Project: pharos
 * Component: pharos-client, CLI-mdb, CLI-ph
 * File: no-matches-not-an-error-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * `mdb *`/`ph *` against an empty (or non-matching) database prints
 * "Error: 501: No matches to query" and exits non-zero — making a
 * perfectly normal "the search ran fine, it just found nothing" outcome
 * look like a failure. Confirmed server-side: status 501 is used
 * identically for query/change/delete all finding zero matching records
 * in every case — it never represents anything actually going wrong.
 * * Traceability:
 * Live user observation 2026-08-04, panel review (Kathy Sierra, Robert
 * Martin, Martin Fowler, Kent Beck, Seth Godin).
 * ======================================================================== */

# Plan: stop treating "no matches" (501) as an error

## Background (already established — do not re-litigate)

- `pharos-server/src/lib.rs` emits `501` from exactly three sites, verbatim:
  `501:No matches to query`, `501:No matches to change`, `501:No matches to delete`
  — all three are the *successful* completion of an operation that found
  zero matching records, never a failure.
- `crates/pharos-client/src/lib.rs`'s `parse_response()` already has a
  precedent for special-casing a specific status code before the generic
  `c >= 400 => Error` catch-all: `506` is special-cased to
  `AuthenticationRequired` instead of falling through. `501` currently has
  no such special case and falls into the generic `Error` bucket, identical
  to a real failure (403, 516, malformed command, etc.).
- `PharosResponse::Matches { count: i32, records: Vec<PharosRecord> }`
  already exists and already correctly represents "the search executed,
  here's what it found" — a count of zero is a completely valid, already-
  representable case; no new enum variant is needed.
- `pharos-pulse` only ever sends `add`/`login`/`auth` in normal operation,
  never `query`/`change`/`delete` — this fix does not change its behavior
  in practice, and touching `parse_response()` here is in-scope (unlike the
  earlier interactive-prompting work, which had to stay *out* of
  `pharos-client` specifically because that was adding interactivity, not
  response classification).

## The change

### 1. `crates/pharos-client/src/lib.rs`: special-case `501` in `parse_response()`

Add a match arm immediately alongside the existing `506` special case
(same function, same `match code` block), before the generic
`c if c >= 400` catch-all:

```rust
501 => {
    // "No matches" for query/change/delete alike - the operation itself
    // succeeded, it just found nothing to act on. Not a failure: represent
    // it the same way a real, non-empty match set is represented, just
    // with zero records, rather than falling into the generic Error bucket.
    return Ok(PharosResponse::Matches { count: 0, records: Vec::new() });
}
```

### 2. `mdb/src/main.rs`'s `handle_response()`: give the empty case an explicit message

Currently, `PharosResponse::Matches { records, .. }` with an empty
`records` would print nothing at all — ambiguous (did it hang? fail
silently? find nothing?). Add an explicit message for the empty case:

```rust
PharosResponse::Matches { records, .. } => {
    if records.is_empty() {
        println!("No matches found.");
    } else {
        for record in records {
            for field in record.fields {
                let value = if human {
                    format_human(&field.key, &field.value)
                } else {
                    field.value
                };
                println!("{:>15}: {}", field.key, value);
            }
        }
    }
}
```

Exit code must be `0` (success) for this case — `handle_response` already
returns `Ok(())` after this match arm, so no further change is needed there
once `parse_response` no longer routes `501` into `Error`.

### 3. `ph/src/main.rs`: identical treatment at its own (inline, not a
separate function) `PharosResponse::Matches` arm

```rust
PharosResponse::Matches { records, .. } => {
    if records.is_empty() {
        println!("No matches found.");
    } else {
        for record in records {
            for field in record.fields {
                println!("{:>15}: {}", field.key, field.value);
            }
        }
    }
}
```

Same exit-code note: this arm already falls through to a normal (zero)
exit, unlike the `Error`/`AuthenticationRequired` arms which call
`process::exit(1)` — no change needed there once `501` no longer reaches
those arms.

## Non-goals (do not touch)

- **Do not** add a new `PharosResponse` enum variant — `Matches { count: 0,
  records: vec![] }` already means exactly this; inventing a parallel
  `NoMatches` variant would just be two ways to say the same thing.
- **Do not** change how any *other* status code is classified — this is
  scoped to `501` only. `516` (Forbidden), `403`, and other genuine
  failures must still route to `Error` exactly as today.
- **Do not** touch `pharos-pulse` or its own response handling — it doesn't
  send `query`/`change`/`delete` in normal operation and is unaffected
  either way, but nothing in it should be touched regardless.
- **Do not** change server-side behavior (`pharos-server/src/lib.rs`) —
  `501` is already correct and consistent there; this is purely a
  client-side classification/presentation fix.

## Verification steps (concrete, live)

1. Real disposable hub in Podman, empty database: run `mdb *` and `ph *`
   against it and confirm both print `No matches found.` and exit `0`
   (not `1`, not "Error: ...").
2. Same hub, add one record, then query for something that *does* match:
   confirm the existing "print the record's fields" behavior is completely
   unchanged (this fix must not affect the non-empty case at all).
3. Same hub: issue a `change`/`delete` against a selector that matches
   nothing and confirm those also now report a clean "no matches" outcome
   with exit `0`, not an error — since `501` is emitted identically for all
   three operations server-side.
4. Regression check: trigger a genuine error (e.g. a malformed command, or
   an unauthorized write attempt yielding `516`) and confirm it is still
   reported as an error with a non-zero exit code — `501`'s reclassification
   must not accidentally swallow real failures.
5. `cargo test --workspace` passes, including any existing tests that
   assert on `PharosResponse::Error` for status `501` if any exist (update
   them if they encode the old, incorrect classification — check first
   rather than assuming).
6. Clean up all disposable test containers.

## Report back

State clearly: the exact diff (`crates/pharos-client/src/lib.rs`,
`mdb/src/main.rs`, `ph/src/main.rs`), results of all 6 verification steps,
and explicit confirmation no other status code's classification changed and
`pharos-server`/`pharos-pulse` were not touched. Do not commit or push —
this repo requires explicit instruction for that, every time.
