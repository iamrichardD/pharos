# IEEE OUI registry snapshots

Vendored copies of the three IEEE MAC-address-prefix registries, used by
`pharos-scan`'s `OUIResolver` to resolve a device's manufacturer from its
MAC address. Fetched directly from IEEE, not a third-party mirror.

| File | Registry | Prefix length | Source |
|---|---|---|---|
| `oui-ma-l.csv` | MA-L | 24-bit | https://standards-oui.ieee.org/oui/oui.csv |
| `oui-ma-m.csv` | MA-M | 28-bit | https://standards-oui.ieee.org/oui28/mam.csv |
| `oui-ma-s.csv` | MA-S | 36-bit | https://standards-oui.ieee.org/oui36/oui36.csv |

All three are plain CSV: `Registry,Assignment,Organization Name,Organization Address`.

**Snapshot date: 2026-08-08.** IEEE's registry is a living document — these
files are a point-in-time copy, vendored (not fetched at build time) so
`pharos-scan` builds reproducibly with zero network dependency. This will
drift as new prefixes get registered. Refreshing is a manual process: re-run
the three `curl` commands above against the same URLs and commit the diff -
not automated, not attempted at build time.
