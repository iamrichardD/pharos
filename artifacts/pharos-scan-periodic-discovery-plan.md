/* ========================================================================
 * Project: pharos
 * Component: Network Scanner (pharos-scan)
 * File: pharos-scan-periodic-discovery-plan.md
 * Author: Richard D. (https://github.com/iamrichardD/pharos)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Panel-reviewed plan for a systemd-timer-driven pharos-scan mode that
 * discovers non-pulse devices (hostname, IP, MAC, manufacturer) every
 * 10 minutes, for asset-management visibility - not port/service scanning.
 * ======================================================================== */

# Plan: unattended periodic device discovery (`pharos-scan --auto`)

## Goal

Discover devices on the LAN that don't run `pharos-pulse` (IoT devices,
switches, anything with no Pharos agent) and register them as queryable
Pharos records with **hostname, IP address(es), MAC address(es), and
manufacturer (OUI)** - nothing else. Run this automatically every 10
minutes via a systemd timer, not on demand by a human.

## Explicit non-goals (clarified directly by the operator)

- **No port scanning, no service/role inference.** `pharos-scan`'s existing
  `scan_subnet()`/`probe_node()`/`Fingerprinter::infer_role()` are TCP-probe-
  and port-based; none of that is reused for this mode. Asset management
  needs to know a device exists, not what's listening on it.
- Not touching the existing interactive `pharos-scan <subnet>` / `--json`
  tool at all - it keeps its current port-probe behavior for on-demand,
  human-reviewed provisioning. This is a second, separate mode.
- Not attempting to keep the OUI database live-updated automatically (see
  below) - a periodically-refreshed static snapshot, not a network fetch
  at scan time.
- Not solving device-ownership transfer (see "Known limitation" below) -
  documented as a caveat, not fixed in this pass.

## What already exists and is being reused as-is

- `parse_arp_cache`/`read_arp_cache` (`pharos-scan/src/engine.rs`) - reads
  `/proc/net/arp`, already correctly skips incomplete (`00:00:00:00:00:00`)
  entries, already tested.
- `OUIResolver::resolve` (`pharos-scan/src/oui.rs`) - MAC-prefix-to-vendor
  lookup mechanism is correct; the *data* is not (see below).
- `lookup_hostname` (reverse DNS) - already decoupled from port probing.
- The multi-valued `ip_addr`/`mac_addr` `Record` fields shipped in Debt
  #50/Issue #197 - exactly the right shape for a device with more than one
  NIC or that's changed IP over time.
- The wire `add` command, which the server always routes through
  `upsert_record` (`pharos-server/src/storage.rs`) - no new server-side
  command needed.

## Real problems found while grounding this plan in the actual code

### 1. `upsert_record`'s dedup key is `hostname`/`alias` only - not MAC or IP

```rust
let identifier = fields.iter().find(|(k, _)| k == "hostname" || k == "alias")...
```

If no `hostname`/`alias` field is present, `upsert_record` *always* falls
through to `add_record` - a brand new record every call. Most silent IoT
devices (smart plugs, sensors) have no reverse-DNS name and no mDNS name.
Running this every 10 minutes against such a device would create a new
duplicate record on every single cycle - hundreds of ghost records within
days, not a MAC-keyed dedup as originally assumed in panel discussion.

**Fix chosen:** don't touch `upsert_record`'s matching logic at all (it's
correct and well-tested for its existing hostname/alias-keyed use).
Instead, `pharos-scan --auto` synthesizes a **stable `alias`** for any
device with no real hostname: `alias = format!("device-{mac_no_colons}")`
(e.g. `device-aabbccddeeff`), derived from the MAC address, which *is*
stable across DHCP lease changes (unlike IP). A device that does have a
real hostname keeps using it as `hostname`, matching existing behavior for
everything else in this system. This keeps 100% of the existing,
tested `upsert_record` code path untouched.

### 2. The OUI table is a 6-entry hardcoded stub, not a real database

`OUIResolver::default()` currently only recognizes VMware, VirtualBox,
"Proxmox (Hypothetical)", Raspberry Pi, and Hyper-V prefixes - it would
resolve `None` for the overwhelming majority of real IoT vendors (Espressif/
ESP32, TP-Link, Sonoff, Shelly, Tuya-based devices, Philips Hue, etc.).
Since manufacturer/OUI is one of the four explicit fields this feature
exists to deliver, this needs real coverage to be useful, not a token
gesture.

**Fix:** vendor real IEEE OUI registry data directly into the repo and
embed it via `include_str!`, parsed once at startup (`once_cell`/
`std::sync::LazyLock`) into a `HashMap`. Checked and confirmed live: IEEE
actually publishes **three separate registries**, not one - using only
the legacy large-block one (as the operator-supplied gist mirror does)
would miss exactly the vendors this feature cares about most:

| Registry | File | Prefix | Entries | Size | Notes |
|---|---|---|---|---|---|
| MA-L | `https://standards-oui.ieee.org/oui/oui.csv` | 24-bit | ~39,900 | 3.8MB | Legacy, large blocks (older/big vendors) |
| MA-M | `https://standards-oui.ieee.org/oui28/mam.csv` | 28-bit | ~6,500 | 745KB | Mid-size blocks |
| MA-S | `https://standards-oui.ieee.org/oui36/oui36.csv` | 36-bit | ~7,100 | 661KB | Small blocks - **where modern/smaller IoT vendors typically land** |

All three are clean, directly-parseable CSV (`Registry,Assignment,
Organization Name,Organization Address`), confirmed via direct fetch
during this planning pass - not the operator-supplied gist mirror
(`gist.githubusercontent.com/.../oui.txt`), which is MA-L-only, pinned to
a specific unknown-date gist revision (won't reflect anything registered
since), and in IEEE's old verbose multi-line-per-entry text format (harder
to parse reliably than the CSVs, for a strict subset of the coverage).

**Architecture: vendor, don't fetch-at-build-time.** A `build.rs` that
downloads these over the network on every `cargo build` would violate this
repo's Zero-Host/reproducible-build expectations (a build's output
depends on network availability and on *when* it happened, not just its
source) and adds a new failure mode to every future build if IEEE's
endpoint is ever unreachable. Instead: fetch all three CSVs once as part
of implementing this, commit them into the repo (e.g.
`pharos-scan/data/oui-{ma-l,ma-m,ma-s}.csv`), and parse them via
`include_str!` at program startup - zero network dependency at build or
run time, fully reproducible, ~5.2MB combined added to the repo (and to
the `pharos-scan` binary's embedded data, negligible for a native binary).
Documented as a point-in-time snapshot (dated in a comment at vendor time)
that will drift as new prefixes get registered - a periodic manual
refresh (re-run the same fetch, commit the diff) is a follow-up debt item,
not solved here or automated.

### 3. Record visibility: resolved via a general, server-derived `source` field

`mdb '*'` implicitly filters to `record_type == Machine` unless the query
explicitly names a `type=` selection (`pharos-server/src/storage.rs`'s
`query()`). This is now resolved as **`type=machine`, always** (visible in
the default fleet view, matching the stated asset-management goal), backed
by a `source` field that isn't scan-specific but a *general* provenance
marker on every record, however it was created:

**`source` values:** `pharos-scan | mdb | ph | web-console | pharos-pulse`

**Derived server-side from `context.id`, not client-supplied.** Every
client already sends `id <client_id>` immediately on connect
(`PharosClient::connect`/the console's `executePharosQuery`), and the
server already stores it in `context.id` (`pharos-server/src/lib.rs`'s
`Command::Id` handler) - this is already exactly the right signal, just
not yet normalized or persisted onto the record. Deriving `source` from
it server-side (rather than trusting each client to remember to pass a
`source=` field themselves) means it can't be omitted or spoofed by a
buggy or malicious client.

Checked the actual client_id strings in use today - normalization is
needed, they don't already line up cleanly with the five values above:

| Real client_id(s) seen on the wire | Normalizes to |
|---|---|
| `mdb` | `mdb` |
| `ph` | `ph` |
| `pharos-scan` | `pharos-scan` (once `--auto` mode exists) |
| `pulse-{hostname}` (dynamic per machine, `crates/pharos-pulse/src/main.rs`) | `pharos-pulse` |
| `web-console`, `web-console-add`, `web-mdb-search`, `web-mcp`, `pharos-console-web` (five distinct strings depending which console feature is writing) | `web-console` |
| anything else (unrecognized/custom client, or no `id` sent at all) | `source` field omitted entirely - no invented sixth value |

New `normalize_source(client_id: &str) -> Option<&'static str>` helper
(e.g. in `pharos-server/src/lib.rs`, prefix-matched: `starts_with("pulse-")`
→ `pharos-pulse`, `starts_with("web-")` or `== "pharos-console-web"` →
`web-console`, exact match otherwise) - a single source of truth reused
both for setting the field (below) and for labeling the new Prometheus
counters (see the metrics section), so the two can never drift apart.

**Immutable on creation, like `type`.** `source` describes a record's
*provenance* (how did this record come to exist), not "who last touched
it" - a human running `mdb change` on a pulse-managed machine to fix a
typo shouldn't flip it to `source=mdb`. `Command::Add`'s handler
(`pharos-server/src/lib.rs`) injects the normalized `source` into the
outgoing `fields` before calling `upsert_record`, exactly as it would for
any other field; `upsert_record`'s existing-record branch
(`pharos-server/src/storage.rs`) gets one small addition mirroring its
existing `type`-immutability check: an incoming `source` on a record that
already has one is silently ignored (kept as-is), not written over -
unlike `type`'s immutability check, this is *not* an error/rejection,
since a legitimate later write from a different tool on an existing
record is completely normal and shouldn't fail, it just shouldn't be
allowed to rewrite provenance.

The auto-discovery write path's "never clobber a pulse-managed record"
rule (below) follows directly from this: skip any existing record whose
`source` is present and isn't `pharos-scan`. Note (Martin Fowler,
panel review): this is a *permanent* precedence rule, not a temporary
one - once a record has any non-scan `source`, the scanner can never
touch it again, even if its `ip_addr`/`mac_addr` later goes stale. Worth
stating explicitly in the implementation's code comment so a future
reader doesn't wonder why such a record's data never self-corrects.

### 3a. Known limitation: `source` doesn't survive multi-server replication (not solved here)

Checked `pharos-server/src/sync.rs`'s `replicate_command()`: it forwards the
*raw wire command text* to peer servers, connecting as a hardcoded
`PharosClient::connect(&peer, "pharos-sync")` - not the original writer's
client_id. Since `source` is derived fresh from `context.id` independently
on each server (Section 3), a peer receiving a `SYNC add ...` would derive
its own `source` from `context.id = "pharos-sync"`, which
`normalize_source()` doesn't recognize (`None`) - so a replicated record's
copy on a peer server would end up with **no `source` field at all**,
while the originating server's own copy correctly has one. Not a wrong
value, just a missing one on replicas - and only relevant to multi-server
(`peer`-role) deployments, not the common single-hub case.

**Deliberately deferred, not solved in slice 1.** Fixing this properly
means either having the originating server bake the resolved `source`
explicitly into the replicated wire text (and the receiving peer trust it
*only* when `is_trusted_sync_peer()` - already exists,
`pharos-server/src/lib.rs` - confirms genuine authenticated peer
forwarding, never from an ordinary client typing `SYNC` in front of their
own command), or changing `replicate_command()`'s signature to take
structured fields instead of a raw string. Real scope, its own follow-up -
tracked here so it isn't silently discovered later as a surprise bug.
For slice 1: a client-supplied `source=` field is always stripped and
ignored (never trusted from any connection, sync or otherwise) - the
field is always either correctly derived from `context.id`, or absent.

### 4. Known limitation: no ownership-transfer path (not solved here)

`upsert_record` bonds a record to whichever `owner_fingerprint` first
creates it and rejects (`511:Collision`) any *different* fingerprint
writing to the same hostname/alias afterward. If `pharos-scan --auto`
discovers a device first (bonding it to the scanner's own key) and someone
later installs `pharos-pulse` on that same device (a *different* key), the
pulse agent's own `add` would collide against the scan-created record.
**Documented as an operator-facing caveat** (delete the scan-created
record before installing pulse on a previously-discovered device) rather
than building a transfer mechanism now - real scope, deliberately deferred.

## The new discovery mechanism (replacing port-probing for this mode)

Per direct operator clarification: no TCP/service probing. Passive
ARP-table reads alone are too incomplete (kernel ARP entries expire after
a few minutes of inactivity, so a quiet device the host hasn't recently
talked to won't appear) to be useful on a 10-minute cycle. The chosen
mechanism:

1. **ICMP ping sweep** of the local subnet(s) - one echo request per host
   address, to freshen ARP entries for devices the host hasn't talked to
   recently. **Panel-resolved: shell out to the system `ping` binary**,
   not an in-process raw socket. `ping` already carries `cap_net_raw+ep`
   as a file capability on essentially every distro, so this needs zero
   capability grant on the `pharos-scan` process itself - smaller
   privilege footprint than `AmbientCapabilities=CAP_NET_RAW` (which is
   usually insufficient alone under a hardened unit anyway, needing
   `CapabilityBoundingSet=` too), and avoids running custom raw-socket/
   untrusted-packet-parsing code with elevated capabilities in-process.
2. **Read the ARP/neighbor table** (`read_arp_cache`, already correct)
   immediately after the sweep.
3. **OUI-resolve** each MAC (expanded table, see above).
4. **Reverse-DNS** each IP for a hostname, best-effort (already correct,
   already decoupled from ports).
5. No port probing, no role inference, anywhere in this path.

Subnet(s) to sweep: reuse the host's own local interface CIDR(s) (already
derivable the same way `pharos-pulse`'s `if-addrs`-based interface
collection works, per Debt #50) rather than requiring the operator to
hand-configure a subnet - the timer unit shouldn't need a CIDR argument to
function out of the box.

## New `pharos-scan --auto` mode

- New CLI flag (or subcommand) that skips the interactive TUI entirely -
  mirrors how `--json` already skips it, but writes instead of printing.
- For each discovered device: build a `Record` write with `type=machine`,
  `hostname` (if resolved) or a MAC-derived `alias` (if not), multi-valued
  `ip_addr`/`mac_addr`, and `manufacturer`. Send via the existing `add`
  wire command, connected with client_id `pharos-scan` (so `source` is
  set automatically server-side, per Section 3 - `pharos-scan --auto`
  itself never sends a `source=` field). Server-side upsert handles
  create-vs-update transparently, as it already does for `pharos-pulse`.
- Skips devices already known **to be managed by something else** (an
  existing record whose `source` is present and isn't `pharos-scan`) -
  never overwrite a pulse-managed, manually-added, or console-added
  record's data with scan-inferred data; `source`'s immutability (Section
  3) means this check is a simple field read, not a heuristic. Devices
  already `source=pharos-scan` get their `ip_addr`/`mac_addr`/
  `last_seen_at` refreshed normally via the existing upsert
  append-idempotent logic.
- Every run logs a summary (devices swept, new vs. refreshed, any
  write failures) - this runs unattended under a timer with no human
  watching stdout, so failures need to be discoverable via
  `journalctl`/existing alerting, not silently swallowed. **Including the
  zero-activity case** (swept N hosts, 0 new, 0 refreshed, 0 failures) -
  an operator debugging via `journalctl` needs to be able to tell "running
  fine, found nothing new" apart from "not running at all," not just see
  silence in both cases.

## Its own identity (mirrors Debt #34's fix for `pharos-pulse`)

`add` requires authentication under every tier except a fully open one.
`pharos-scan --auto` needs its own dedicated Ed25519 key and systemd
enrollment, following the exact pattern `install_pulse()` already
established: generate `${PHAROS_DIR}/keys/scan_id_ed25519` at install
time, `PHAROS_PRIVATE_KEY` pointed at it in the unit's `Environment=`,
auto-enrolled via the same `--fetch-ca-ssh` flow (falling back to a
printed one-liner) - not a new mechanism, reuse of a proven one.

**Also needs the `admin` role, explicitly.** Panel catch (DevSecOps):
under `SecurityTier::Scoped`, write commands additionally require the
`admin` role (`SecurityTierMiddleware`'s `is_write_command` check,
`pharos-server/src/middleware.rs`) - just enrolling the key isn't enough
on a Scoped-tier hub, it would get a surprise `516` on its first `add`.
The scan key needs the `admin` role granted via the same filename
convention already used for `pharos-pulse`'s own key.

## systemd units (first `.timer` in this repo - no existing precedent to diverge from)

```ini
# pharos-scan-auto.service
[Unit]
Description=Pharos Scan - Periodic Passive Device Discovery
After=network.target

[Service]
Type=oneshot
ExecStart=${INSTALL_DIR}/pharos-scan --auto
User=pharos
Environment=PHAROS_SERVER=${host}
Environment=PHAROS_PRIVATE_KEY=${scan_key_path}
${ca_cert_line}
AmbientCapabilities=CAP_NET_RAW   ; only if the ping sweep needs it directly
                                   ; (not needed if shelling out to system ping)
```

```ini
# pharos-scan-auto.timer
[Unit]
Description=Run Pharos Scan discovery every 10 minutes

[Timer]
OnBootSec=2min
OnUnitActiveSec=10min
Persistent=true   ; a missed run (host was off) still fires once on boot

[Install]
WantedBy=timers.target
```

## Installation: opt-in, not automatic

Per the DevSecOps review, unattended background network scanning is more
sensitive than the existing interactive tool a human explicitly runs and
reviews - `install_toolbelt()` installing `pharos-scan` the binary does
**not** imply enabling the timer. A new, separate `install_scan_auto()`
function (or an explicit `--enable-scan-timer` flag to `install.sh`) that
an operator opts into deliberately, following the same
`activate_systemd_service`-at-the-end convention already used elsewhere.

## Prometheus counters, labeled by the same normalized `source`

Raised separately (does Pharos track add/update/delete metrics for
deviation alerting - it didn't, at all, before this plan): three new
counters in `pharos-server/src/metrics.rs`, alongside the existing
gauges - `pharos_records_added_total`, `pharos_records_updated_total`,
`pharos_records_deleted_total` - each labeled `source="<normalized>"`
using the *exact same* `normalize_source()` helper from Section 3, so the
stored field and the metric label can never drift apart. This is what
actually delivers on "alert on unusual deviations" - a Prometheus
`rate(pharos_records_added_total{source="pharos-scan"}[10m])` rule can
catch the new unattended writer misbehaving (e.g. a bug causing runaway
duplicate creation) independently from normal `mdb`/`ph`/human-driven
write volume, without Pharos itself hand-rolling anomaly-detection math.
Idiomatic split: Pharos exposes accurate counters, Prometheus/Alertmanager
(or Grafana) owns the actual alerting rules. Deliberately not duplicating
this into the existing `alerting.rs` threshold-check/webhook mechanism in
this pass - flagged as a follow-up for operators not running a full
Prometheus/Alertmanager stack, not solved here.

## Test plan

- Unit tests for `normalize_source()` covering all five mappings, the
  `pulse-{hostname}` prefix match (varying hostnames, always normalizing
  to `pharos-pulse`), all five real web-console client_id strings
  normalizing to `web-console`, and an unrecognized client_id returning
  `None` (field omitted, not a sixth invented value).
- Unit test confirming `source` is immutable: a record created with
  `source=pharos-scan`, then upserted again with a different client_id
  (e.g. `mdb`), keeps `source=pharos-scan` - mirrors the existing `type`
  immutability test's shape.
- Unit tests for the ping-sweep helper and the MAC-derived-alias synthesis
  (`device-{mac}` formatting, collision-safety if a hostname genuinely
  looks like that pattern already).
- Unit test confirming a device with a resolvable hostname keeps using it
  (not the synthesized alias).
- Integration test: seed a `MemoryStorage` with one pre-existing
  `source=pharos-scan` record, run the auto-discovery write path against a
  synthetic discovery result for the *same* device (same MAC, new IP) -
  assert it's an update (multi-value `ip_addr` gained an entry), not a
  duplicate record.
- Integration test: seed a pre-existing pulse-managed
  (`source=pharos-pulse`) record at the same hostname/MAC as a
  "discovered" device - assert the auto-discovery path does **not**
  attempt to overwrite it.
- Integration test: confirm the new Prometheus counters increment with
  the correct `source` label for an `add` via each of `mdb`, `ph`, and
  `pharos-scan` client_ids.
- OUI table: a test asserting a handful of real, well-known vendor
  prefixes (not just the current stub's 6) resolve correctly, to guard
  against a bad future regeneration silently truncating the table.

## Live verification (required before shipping)

Run `pharos-scan --auto` for real against the actual home LAN
(`scripts/live-verify` can't help here - it can't simulate a real L2
broadcast domain with real IoT devices) - confirm real devices without
`pharos-pulse` installed (e.g. `technitium-01`/`02` if not pulse-managed,
or genuine IoT devices on the network) show up in a plain `mdb '*'` with
`source: pharos-scan` and correct IP, MAC, and a real resolved
manufacturer name, not `None`. Confirm the timer actually fires on
schedule (`systemctl list-timers pharos-scan-auto.timer`) and that a
second run 10 minutes later updates `last_seen_at`/refreshes `ip_addr`
rather than duplicating the record. Confirm
`pharos_records_added_total{source="pharos-scan"}` increments on the
first run and `pharos_records_updated_total{source="pharos-scan"}` on the
second, via a real scrape of `/metrics`.

## Recommended dispatch sequencing (panel: Kent Beck)

This plan is larger than one shippable unit. Rather than one dispatch
covering everything, three independently plannable/testable/shippable
slices, in this order:

1. **`source` field + `normalize_source()` + immutability + Prometheus
   counters** - purely a `pharos-server` change, testable today against
   the *existing* clients (`mdb`, `ph`, `pharos-pulse`, the web console)
   without `pharos-scan --auto` existing yet at all.
2. **OUI database expansion** (vendor the three IEEE CSVs, rewrite
   `OUIResolver`) - independently useful, improves the existing
   interactive `pharos-scan <subnet>`/`--json` tool immediately, zero
   dependency on slice 1 or 3.
3. **`pharos-scan --auto`** (ping sweep, MAC-derived alias, the new CLI
   mode, its own key/role provisioning, the systemd service+timer,
   `install_scan_auto()`) - the actual new feature, built on top of
   slices 1 and 2 once both are shipped and live-verified independently.

## Report back

State clearly: the exact diff, all new test names and pass/fail, the OUI
table's real entry count (not just "expanded"), and the live verification
transcript - specific discovered devices, their resolved fields, and
confirmation the second scan cycle updated rather than duplicated. Do not
commit, push, tag, or close the GitHub issue - explicit instruction
required for each, every time.
