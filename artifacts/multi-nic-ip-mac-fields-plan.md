/* ========================================================================
 * SUPERSEDED 2026-08-06 by artifacts/multi-value-ip-mac-fields-plan.md.
 * This plan's core premise — "RFC 2378 has no multi-valued-field mechanism"
 * — was WRONG. The response grammar's `[field name:]` is optional per line
 * (RFC 2378 section 2.2); a continuation line omitting the field name adds
 * another value to the previous field. Confirmed against real, first-hand
 * operator experience with an actual RFC 2378 implementation showing
 * exactly this format. The interface-suffixed naming convention
 * (`ip4_eth0`/`mac_eth0`) already shipped in v1.10.16 is being replaced by
 * genuine RFC-native multi-valued fields. Left in place for history only —
 * do not implement anything from this file.
 * ======================================================================== */
/* ========================================================================
 * Project: pharos
 * Component: pharos-pulse, Server Core
 * File: multi-nic-ip-mac-fields-plan.md
 * Author: Richard D. (https://github.com/iamrichardd/pharos)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Pharos has no tracked plan or structured support for IP/MAC address
 * fields today — mdb/ph accept `ip=`/`mac=` as arbitrary free-form text,
 * pharos-scan captures a single mac internally (OUI lookup only, never
 * sent to the server), and there is no way to represent a device with
 * multiple NICs or both an IPv4 and IPv6 address, since RFC 2378 has no
 * multi-valued-field mechanism (confirmed against artifacts/rfc2378.md's
 * own grammar) and Record.fields is HashMap<String, String> throughout
 * every storage backend — one value per field name, full stop.
 * * Traceability:
 * Design question raised 2026-08-06. Panel-reviewed (Kent Beck, Robert
 * Martin, Martin Fowler, Kathy Sierra, Seth Godin, Senior DevSecOps
 * Specialist) on the core architecture choice: an interface-based field
 * naming convention (RFC-consistent, zero storage-layer changes) over a
 * Record.fields: HashMap<String, Vec<String>> rewrite (touches every
 * storage backend, query matching, wire serialization, and every existing
 * .fields.get() consumer, for a case the RFC itself doesn't model).
 * ======================================================================== */

# Plan: interface-based IP/MAC field convention + pharos-pulse auto-population

## Background (already established — do not re-litigate)

- RFC 2378's grammar (`artifacts/rfc2378.md`) is strictly one value per field
  name (`response = code [index] [field] text CRLF`, one field per response
  line). Its own answer to "multiple related values" is separate field
  names (its example schema has `phone`/`office_phone`/`fax`, not one
  `phone` field holding several numbers). Do not build multi-valued field
  support into `Record.fields` — stay `HashMap<String, String>`.
- `pharos-scan`'s `DiscoveredNode` (`pharos-scan/src/lib.rs`) already
  captures a single `mac: Option<String>` (used only for OUI vendor lookup
  during discovery, never sent in the `add` command it issues) and a single
  `ip: IpAddr` (the address it was probed at). Network scanning inherently
  discovers one IP per probed target — it is not the right place to solve
  multi-NIC/dual-stack; leave `pharos-scan` untouched by this plan.
- `pharos-pulse` already collects real hardware inventory via `sysinfo`
  0.30 (CPU, RAM, etc. — see `crates/pharos-pulse/src/main.rs`) using the
  established "Baseline vs. Delta" strategy (Task 14.11/Issue #99): full
  inventory once at `ONLINE`, minimal fields on every `HEARTBEAT`. It is
  the one component that can reliably enumerate a host's *own* real network
  interfaces, and is the right place to auto-populate this — not something
  to push onto humans typing `mdb add` by hand, who will invent
  inconsistent names.
- Open technical question this plan must resolve during implementation,
  not assume: does `sysinfo` 0.30's `Networks` API expose each interface's
  assigned IP address(es), or only MAC address and traffic stats? If IP
  enumeration isn't available from `sysinfo` directly, evaluate the
  smallest reasonable addition (e.g. the `if-addrs` crate — small, no
  transitive dependency bloat, cross-platform) rather than a heavier
  networking crate. Do not guess; check the actual `sysinfo` 0.30 docs/API
  and report which path was needed.

## The change

### 1. Field naming convention: interface-based, not numbered

Adopt `ip4_<iface>`, `ip6_<iface>`, `mac_<iface>` (e.g. `ip4_eth0`,
`ip6_eth0`, `mac_eth0`, `ip4_wlan0`) as the canonical convention for
multi-NIC/dual-stack cases — keeps an interface's IP and MAC semantically
linked, unlike a numbered `ip_2`/`ip_3` scheme that loses that association.
Keep the existing unsuffixed `ip`/`mac` fields as the simple/legacy
single-value case for backward compatibility with existing docs
(`cli-clients.mdx`'s `mdb add ... ip="10.0.0.5"` example) and quick manual
entries — do not deprecate or remove them.

### 2. `pharos-pulse`: auto-populate real interface data

Extend `collect_inventory()` (or wherever the baseline `ONLINE` payload is
built) to enumerate the host's real network interfaces and emit
`ip4_<iface>`/`ip6_<iface>`/`mac_<iface>` fields for each one found
(skip loopback). This is baseline-only data (matches the existing
"Baseline vs. Delta" pattern — interfaces don't change every heartbeat;
don't resend this on every `HEARTBEAT`, only at `ONLINE`/full-inventory
time, same cadence as CPU/RAM fields).

### 3. `pharos-server`: optional format validation

When an `ip4_*`, `ip6_*`, or `mac_*` field is present in an `add`/`change`
command, validate its format (a real IPv4/IPv6 address, a real MAC address)
and reject malformed values with `512:Illegal value: ...` — matching the
existing `StorageError::InvalidArgument` convention already used for
`type`. **Do not** make these fields required — most records legitimately
have none (a person record, a cloud instance behind NAT, a hostname-only
placeholder). Validate only when the field is actually supplied. Apply the
same validation to the plain `ip`/`mac` fields too, for consistency.

### 4. Documentation

Update `website/src/content/docs/cli-clients.mdx` and/or `automation.mdx`
(wherever `pharos-pulse`'s inventory fields are documented) with the new
convention and a real example, so an operator querying `mdb hostname=X`
knows which field names to expect/search for a multi-NIC host.

## Non-goals (do not touch)

- Do not change `Record.fields`'s type (`HashMap<String, String>`) — no
  multi-valued-field architecture change, per the panel decision above.
- Do not touch `pharos-scan` — single-IP-per-probe is correct for what it
  does; this plan is about a host's *own* self-reported interfaces via
  `pharos-pulse`, a different data source entirely.
- Do not touch `mdb`/`ph`'s own code — they already pass through arbitrary
  `field=value` pairs; no client-side change needed for the new field
  names to work.
- Do not require `ip4_*`/`ip6_*`/`mac_*` fields on any record type.

## Verification steps (concrete, live)

1. Confirm what `sysinfo` 0.30's `Networks` API actually exposes (IP
   address availability specifically) before writing any collection code —
   report this finding explicitly, even if it changes the implementation
   approach (e.g. requires `if-addrs`).
2. Real disposable node in Podman with at least one non-loopback interface;
   run the updated `pharos-pulse` baseline registration; confirm via a real
   `mdb` query that `ip4_<iface>`/`mac_<iface>` (and `ip6_<iface>` if the
   interface has one) fields appear with real, correct values matching
   what `ip addr` reports inside that same container.
3. Directly exercise the storage layer with a malformed `ip4_eth0` value
   (e.g. `"not-an-ip"`) via `add`; confirm `512:Illegal value` over the
   wire, not a silent accept or generic `500`.
4. Confirm a well-formed value for all four field families
   (`ip4_*`/`ip6_*`/`mac_*`/plain `ip`/`mac`) is accepted normally.
5. Confirm a record with none of these fields (e.g. a `ph`-created person
   record) is completely unaffected — no validation triggers, no fields
   injected.
6. `cargo test --workspace` passes.
7. Clean up all disposable test containers.

## Report back

State clearly: the exact diff (all touched files), the `sysinfo`
capability finding from step 1 (and what was added if anything), results
of all 7 verification steps, and explicit confirmation `mdb`/`ph`/
`pharos-scan`/`pharos-server`'s `Record.fields` type were not touched. Do
not commit or push — this repo requires explicit instruction for that,
every time.
