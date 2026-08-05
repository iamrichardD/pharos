/* ========================================================================
 * Project: pharos
 * Component: Server Core, CLI-mdb, CLI-ph
 * File: record-type-integrity-plan.md
 * Author: Richard D. (https://github.com/iamrichardD/pharos)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * A record's `record_type` (a structured, typed cache used for mdb-vs-ph
 * query discriminator filtering) and its raw `fields["type"]` (the RFC
 * 2378 wire-protocol field) are two sources of truth for the same fact,
 * and nothing has ever kept them in sync after a record's initial
 * creation. Confirmed live in production: a record whose very first
 * successful registration attempt happened to omit `type` got permanently
 * stuck with `record_type: None`, invisible to every mdb/ph query forever
 * afterward, even though every subsequent heartbeat correctly resent
 * `type=machine` into its fields. This plan closes the gap at every layer
 * it can occur, consistent with this project's own established "fail
 * fast" architectural principle (TODO.md Phase 25).
 * * Traceability:
 * Found live 2026-08-05. Panel-reviewed (Kent Beck, Robert Martin, Martin
 * Fowler, Kathy Sierra, Senior DevSecOps Specialist).
 * ======================================================================== */

# Plan: record type integrity — force at creation, forbid mutation, self-heal existing data

## Background (already established — do not re-litigate)

- `RecordType` enum: `Person`, `Machine`, `Other(String)` — any string not
  exactly "person"/"machine" becomes `Other(that string)`. Type is never
  meant to be absent; the catch-all already assumes something is always
  given.
- `StorageError::InvalidArgument(String)` already exists and already maps
  to `512:Illegal value: {msg}` in the `Query` command handler
  (`pharos-server/src/lib.rs`) — reuse this exact convention, don't invent
  a new status code.
- `add_record()` (`pharos-server/src/storage.rs`) computes `record_type`
  from `fields.get("type")` **once**, at creation. `upsert_record()`'s
  existing-record branch updates `fields` on every call but never touches
  `record_type` again. `change_record()` has no field protection at all —
  it blindly applies every `(field, value)` in `modifications`, including
  `type`, updating `fields` with no corresponding `record_type` update
  either.
- **Confirmed live**: a record can have `record_type: None` while its own
  `fields["type"] == "machine"` sitting right there — i.e. `fields["type"]`
  is the reliable, always-current source of truth; `record_type` is the
  thing that goes stale. Any comparison this plan adds must read from
  `fields.get("type")`, never from `record_type`.

## The change

### 1. `mdb/src/main.rs` and `ph/src/main.rs`: force the correct type on `add`

When constructing the wire command for an `add` operation, ensure a
`type=machine` (mdb) / `type=person` (ph) token is present — inserting it
if absent. If the user already supplied a *different* explicit `type=`
value, override it, but print a one-line stderr note first:
```
Note: mdb always registers machine-type records; overriding type=<their value> to type=machine.
```
(Symmetric wording for `ph`/`person`.) If the user supplied no `type=` at
all, just add it silently — this is the expected, unremarkable case, not
worth a note.

### 2. `pharos-server/src/storage.rs`: reject a new record with no type

In `add_record()` (or the `upsert_record()` call site that routes to it
for a genuinely new hostname), if `fields.get("type")` is absent or empty,
return `Err(StorageError::InvalidArgument("a 'type' field is required (e.g. type=machine)".to_string()))`
instead of proceeding. Wire this into the `Add` command handler
(`pharos-server/src/lib.rs`, currently missing an `InvalidArgument` arm
entirely — it falls through to the generic `500:Internal storage error`)
with a new match arm mirroring the `Query` handler's existing one:
```rust
Err(crate::storage::StorageError::InvalidArgument(msg)) => {
    writer.write_all(format!("512:Illegal value: {}\n", msg).as_bytes()).await?;
}
```

### 3. `pharos-server/src/storage.rs`: reject a type mismatch on `upsert_record`'s existing-record path

In the existing-record branch (before applying field updates): compare the
incoming `fields.get("type")` (if present in this call's fields) against
**the record's own existing `fields.get("type")`** — not `record_type`.
- If the incoming call has no `type` at all, proceed as today (unrelated
  fields still update normally — most heartbeat-style upserts for
  established records may not always resend every original field).
- If incoming `type` matches the existing stored `fields["type"]`
  (including the common case of a repeat, unchanged reassertion, e.g.
  every pulse heartbeat), proceed normally — this must stay a silent no-op
  for the field itself, exactly as it is today.
- If incoming `type` differs from the existing stored `fields["type"]`,
  return `Err(StorageError::InvalidArgument("type is immutable after creation and cannot be changed".to_string()))`
  — reject the **whole** upsert, not just that one field, and do not apply
  any of the other field updates in the same call either (fail closed, not
  partial).

### 4. `pharos-server/src/storage.rs`: forbid `type` in `change_record`'s modifications

Before applying any modifications in `change_record()`, check whether
`modifications` contains a `type` key at all (regardless of what value it
holds, including a same-value no-op reassertion). If so, return
`Err(StorageError::InvalidArgument("type cannot be modified via change - it is set once at record creation".to_string()))`
and apply none of the modifications in that call. Wire the corresponding
`InvalidArgument` arm into the `Change` command handler in
`pharos-server/src/lib.rs` (check whether one already exists there before
assuming; add if missing, matching the same `512:Illegal value` mapping).

### 5. `pharos-server/src/storage.rs`: one-time startup self-heal

Fold this into the **existing, synchronous** `load_from_disk()` path
(`FileStorage`'s startup loading, already `#[instrument]`-traced) — it
must complete before the server starts accepting connections, not run as
a separate/async task that could race with item 3's new mismatch-check on
a fresh restart. For every loaded record:
- If `fields.get("type")` is present and parses to a `RecordType` that
  differs from the record's current (possibly `None` or stale)
  `record_type`, recompute and correct `record_type` from `fields["type"]`.
  Log a `warn!` summarizing how many records were corrected (a count is
  sufficient; per-record detail is optional).
- If `fields.get("type")` is absent entirely (no field to derive from at
  all — a genuinely different, rarer case than a stale cache), leave
  `record_type` as-is and `warn!` explicitly per such record ("record ID
  {id} has no type field, cannot self-heal, remains invisible to
  mdb/ph queries — needs manual correction") so operators have visibility
  into anything that still needs manual attention.

## Non-goals (do not touch)

- **Do not** change how `pharos-pulse` handles a rejected upsert (e.g. no
  retry/backoff logic changes) — the panel raised this as a real,
  separate reliability question worth considering later, deliberately not
  folded into this plan.
- **Do not** remove or restructure the `record_type`/`fields["type"]`
  duplication itself (the more invasive Option B discussed earlier) —
  this plan keeps both fields but makes them structurally impossible to
  desync going forward, which is the agreed, lower-risk approach.
- **Do not** touch `crates/pharos-client` or `crates/pharos-pulse` —
  `pharos-pulse` already always sends `type=machine`; nothing there needs
  to change for this plan.
- **Do not** add a database migration tool or CLI command for the
  self-heal — it runs automatically and silently (aside from `warn!`
  logging) on every server startup, as part of loading, not as a separate
  operator-invoked step.

## Verification steps (concrete, live)

1. Real disposable hub in Podman. Run `mdb add hostname=srv-1` (no
   explicit type) and `ph add name="Jane Doe"` (no explicit type); confirm
   both wire commands actually sent include `type=machine`/`type=person`
   respectively (verify via the hub's own journalctl-equivalent log
   output, or the new `--debug` flag from the prior session's work).
2. Run `mdb add hostname=srv-2 type=person` (deliberately conflicting) and
   confirm: the stderr note appears, the wire command sent still has
   `type=machine` (overridden), and the created record is genuinely
   `type=machine` on the server.
3. Directly exercise the storage layer (not just through mdb/ph) with a
   fields map that has no `type` key at all; confirm `add_record`/
   `upsert_record` returns the new `InvalidArgument` error and the `Add`
   command handler surfaces `512:Illegal value: ...` over the wire — not
   the generic `500`.
4. Add a real record with `type=machine`, then attempt to upsert the same
   hostname with `type=person`; confirm the whole upsert is rejected with
   `512`, and confirm the record's fields are completely unchanged
   afterward (not partially applied).
5. Add a real record, then attempt `change hostname=<x> type=anything`
   (including reasserting the *same* value); confirm it's rejected with
   `512` in both cases, and no other modifications in the same `change`
   call were applied either.
6. **The critical regression check from panel review**: manually construct
   a `data.json` with a record that has `record_type: null` but
   `fields["type"] == "machine"` (reproducing your laptop's exact current
   broken state) *before* starting the server. Start it, confirm the
   self-heal log line appears, confirm the record's `record_type` is now
   correctly `Machine`, and confirm a subsequent real heartbeat-style
   upsert for that same hostname (still asserting `type=machine`) succeeds
   normally — proving the self-heal completes before any new write traffic
   could be incorrectly rejected by item 3's mismatch-check.
7. Construct a record with no `type` field in `fields` at all; confirm the
   self-heal leaves it alone and logs the specific per-record warning
   rather than crashing or silently ignoring it.
8. `cargo test --workspace` passes.
9. Clean up all disposable test containers.

## Report back

State clearly: the exact diff (all touched files), results of all 9
verification steps (especially #6, the ordering/regression-critical one),
and explicit confirmation `crates/pharos-client` and `crates/pharos-pulse`
were not touched. Do not commit or push — this repo requires explicit
instruction for that, every time.
