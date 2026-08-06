/* ========================================================================
 * Project: pharos
 * Component: Server Core, pharos-pulse
 * File: multi-value-ip-mac-fields-plan.md
 * Author: Richard D. (https://github.com/iamrichardd/pharos)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Supersedes artifacts/multi-nic-ip-mac-fields-plan.md, whose core premise
 * was wrong: RFC 2378 DOES support multi-valued fields. Its response
 * grammar (section 2.2) is `result code:[entry index:][field name:]text`
 * — the field name is optional per line, and a continuation line omitting
 * it adds another value to the previous field. Confirmed against real,
 * first-hand operator experience with an actual RFC 2378 implementation
 * showing exactly this format for ip_addr/mac_addr. This plan replaces the
 * interface-suffixed naming convention shipped in v1.10.16 with genuine
 * RFC-native multi-valued fields.
 * * Real-world driving use case (from direct operator experience, not
 * hypothetical): a security-response service that receives an IP or MAC
 * address from virus-scanning software, looks up the matching device via
 * Pharos, and either shuts the switch interface(s) or blackholes the
 * device — using additional fields like `user`/`office_location` to notify
 * the right people. Confirmed directly: this workflow has no known need
 * to correlate which IP belongs to which MAC or which interface — pure
 * membership lookup ("is this IP/MAC anywhere on this record") is
 * sufficient. Do not build IP-to-MAC-to-interface correlation.
 * * Traceability:
 * Panel-reviewed (Kent Beck, Robert Martin, Martin Fowler, Kathy Sierra,
 * Seth Godin, Senior DevSecOps Specialist) on the representation choice:
 * a parallel `multi_fields: HashMap<String, Vec<String>>` on `Record`,
 * scoped to genuinely multi-valued attributes only, over either (a) a
 * global `Record.fields: HashMap<String, Vec<String>>` rewrite (forces
 * every existing single-value consumer to become list-aware for a
 * capability they don't need) or (b) joining multiple values into one
 * delimited string (a fragile parsing/escaping hack).
 * ======================================================================== */

# Plan: RFC-native multi-valued `ip_addr`/`mac_addr` fields

## Background (already established — do not re-litigate)

- RFC 2378's response grammar (`artifacts/rfc2378.md`, section 2.2):
  `result code:[entry index:][field name:]text`. Worked example format for
  a multi-valued field (confirmed by direct operator experience, not
  present as a worked example in the RFC text itself):
  ```
  S: -200:1:     ip_addr: 192.168.86.5
  S: -200:1:            : 192.168.86.6
  S: -200:1:     mac_addr: e0:51:d8:1d:e3:22
  S: -200:1:             : 3c:3b:ad:6b:f2:c2
  ```
  First line of a field carries the field name; subsequent lines for the
  same field, same entry index, omit it (blank-padded to the same column).
- `Record.fields: HashMap<String, String>` (`pharos-server/src/storage.rs`)
  and every one of its current consumers (query matching, `type`/
  `record_type` validation, self-heal, `alerting.rs`'s version comparison,
  LDAP field mapping, wire serialization, `mdb`/`ph` output formatting)
  stay completely untouched by this plan — none of them need to become
  multi-value-aware.
- The v1.10.16 interface-suffixed convention (`ip4_eth0`/`mac_eth0`,
  `crates/pharos-pulse/src/main.rs`'s `collect_inventory()`,
  `pharos-server/src/storage.rs`'s `validate_ip_mac_field`) is being
  **replaced**, not kept alongside this — remove it as part of this plan,
  don't leave two competing conventions live simultaneously.
- Explicitly **not** building: any IP-to-MAC-to-interface correlation or
  pairing. The real driving use case only needs "is this IP/MAC anywhere
  on this record," confirmed directly — don't add complexity for a
  correlation need that doesn't exist.

## The change

### 1. `pharos-server/src/storage.rs`: `Record` gains a parallel multi-value map

```rust
pub struct Record {
    pub id: usize,
    pub record_type: Option<RecordType>,
    pub fields: HashMap<String, String>,
    pub multi_fields: HashMap<String, Vec<String>>,
    pub owner_fingerprint: Option<String>,
    pub owner_team: Option<String>,
}
```

Reserve `multi_fields` for a small, explicit whitelist of field names —
`ip_addr` and `mac_addr` only, for now. Do not make this a general-purpose
mechanism any field name can opt into yet; that's a bigger, separate
design question if it's ever needed. Every existing `fields` consumer is
unaffected — this is purely additive.

### 2. `add`/`change` command semantics: append, explicit rule for each

- `add ip_addr=1.2.3.4 ip_addr=5.6.7.8` (repeated `field=value` pairs in
  one command) appends both values to `multi_fields["ip_addr"]` — matches
  historical RFC/Ph `add`/`make` behavior for repeated field-value pairs.
- A **separate, later** `add`/`change` supplying `ip_addr=` for an
  already-existing record **appends** a new value if not already present
  (idempotent — adding the same value twice must not create a duplicate
  entry), rather than replacing the list. This matches how a real host
  gaining a second NIC/IP should be reflected (the old value is still
  real and shouldn't silently disappear because of an unrelated update).
- There is currently no `delete`-a-single-value-from-a-multi-valued-field
  operation in RFC 2378's command set, and this plan does not add a Pharos
  extension for it — removing a stale IP/MAC requires deleting and
  re-adding the record, exactly as RFC 2378 itself would require. Document
  this limitation, don't build around it.

### 3. Query matching: membership, not equality

Update `query()`'s selection-matching loop (`pharos-server/src/storage.rs`,
currently `record.fields.get(field_name)` → single-value `self.matches()`)
so that when `field_name` is `"ip_addr"` or `"mac_addr"`, the match checks
`multi_fields.get(field_name)` and succeeds if **any** value in the list
matches (reusing the existing `self.matches()` word/wildcard logic per
candidate value, not a new matching algorithm). The unqualified/no-field
"match anywhere" case (`field_opt: None`) must also check `multi_fields`
values, not just `fields`, or a bare `mdb 192.168.86.5` search would miss
IPs stored this way.

### 4. Wire protocol: emit the real continuation-line format

Wherever `pharos-server` currently serializes a `Query`/`Change` response
per matched record (`pharos-server/src/lib.rs`), extend it to also emit
each `multi_fields` entry as one line per value: first line
`field_name: value`, subsequent lines for the same field blank-padded
(field-name column empty) — matching the exact format above. `mdb`/`ph`
already print whatever lines the server sends; confirm their existing
output formatting doesn't need a code change to display this correctly
(it may already "just work" since it's parsing line-by-line), but verify
this directly rather than assuming.

### 5. `pharos-pulse`: revert to plain `ip_addr`/`mac_addr`, keep the topology filter

Replace the v1.10.16 interface-suffixed emission
(`ip4_<iface>`/`ip6_<iface>`/`mac_<iface>`) in `collect_inventory()` with
plain `ip_addr`/`mac_addr` multi-value collection — same enumeration logic
and same "skip any interface with no assigned IP at all" filter already
proven correct against real bonded/bridged Proxmox topology (keep that
filter exactly as-is, it's still correct and necessary), just emitting
into the new multi-value representation instead of per-interface field
names. `pharos-pulse` needs a way to send a `multi_fields`-shaped payload
in its `add`/heartbeat command — check how the wire command is currently
built and extend it to send repeated `ip_addr=`/`mac_addr=` pairs (matching
the append semantics in item 2), not a single field=value each.

### 6. Format validation

Keep validation from the superseded plan, adapted to the new field names:
each value written to `ip_addr` must parse as a valid `IpAddr` (v4 or v6),
each value written to `mac_addr` must be a valid MAC. Reject with
`512:Illegal value` on any invalid value in the set, consistent with the
existing `StorageError::InvalidArgument` convention. Validate every value
in a multi-value `add`/`change`, not just the first.

### 7. Documentation

Update `website/src/content/docs/automation.mdx` and `cli-clients.mdx` to
remove the interface-suffixed convention description (from the superseded
plan) and document the real multi-valued `ip_addr`/`mac_addr` format,
including a query example (`mdb ip_addr=192.168.86.5`) and an explicit
note on the append-only/no-single-value-delete limitation from item 2.

## Non-goals (do not touch)

- No IP-to-MAC-to-interface correlation/pairing — confirmed directly, no
  known use case needs it.
- No general-purpose multi-value mechanism any field can opt into — scope
  strictly to `ip_addr`/`mac_addr` for now.
- Do not change `Record.fields`'s existing type or touch any of its
  current consumers (`alerting.rs`, self-heal, `type`/`record_type`
  validation, LDAP mapping) — `multi_fields` is fully separate.
- Do not touch `pharos-scan` — out of scope, per the superseded plan's
  same reasoning (single-IP-per-probe is correct for what it does).
- Do not add a "remove one value from a multi-valued field" command — not
  part of RFC 2378, not requested.

## Verification steps (concrete, live)

1. Directly exercise the storage layer: `add ip_addr=192.168.86.5
   ip_addr=192.168.86.6 mac_addr=e0:51:d8:1d:e3:22` in one command; confirm
   both values land in `multi_fields["ip_addr"]` (as two entries, not one
   malformed one) and the MAC in `multi_fields["mac_addr"]`.
2. A separate, later `add`/`change` supplying a new `ip_addr=` for the same
   existing record: confirm it appends (list grows), doesn't duplicate an
   already-present value, and doesn't touch `mac_addr`.
3. Real disposable `pharos-server`; issue a raw `query ip_addr=192.168.86.5`
   over the wire; confirm the actual response bytes show the RFC continuation
   -line format (field name on the first line, blank-padded on the second)
   for a record with 2+ IPs — not just that the right record is returned.
4. Confirm `mdb hostname=<x>` (or equivalent) against a multi-IP record
   displays correctly through the real `mdb` binary — check whether any
   client-side change is actually needed or whether it already renders
   correctly, and report which.
5. Malformed value in a multi-value `add` (e.g. `ip_addr=not-an-ip`);
   confirm `512:Illegal value` and that *no* values from that command were
   partially applied (fail closed, not partial).
6. Real disposable node in Podman; run the updated `pharos-pulse` baseline
   registration on a host with at least one real non-loopback interface;
   confirm via a real `mdb` query that `ip_addr`/`mac_addr` show correctly
   as multi-valued fields, and confirm interfaces with no assigned IP are
   still correctly excluded (reproduce the same bonded/bridged-topology
   check already proven in the superseded plan's work).
7. `cargo test --workspace` passes.
8. Clean up all disposable test containers.

## Report back

State clearly: the exact diff (all touched files), results of all 8
verification steps (especially #3, the actual wire-format bytes, and #2,
the append-not-replace behavior), and explicit confirmation `mdb`/`ph`'s
own code, `pharos-scan`, and `Record.fields`'s existing type/consumers
were not touched. Do not commit or push — this repo requires explicit
instruction for that, every time.
