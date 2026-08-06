## Response: v1.10.16 — both gaps from your v1.10.15 rollout are fixed

Thanks for the detailed writeup — both things you hit and worked around live are
now fixed in Pharos itself, not just documented as gotchas to remember.

### 1. The `v`-prefix mismatch is fixed at the comparison, not just in docs

You caught this exactly right: setting `expected_version=1.10.15` (no `v`) against
a console self-reporting `version=v1.10.15` would have created a **permanent**
mismatch that alerts once and then silently never again (the dedup logic keys on
the exact pair — once it's fired for that pair, it won't re-fire unless the pair
changes). That's worse than it sounds: it would have looked "resolved" after one
webhook, when it actually meant "you'll never hear about this drift again."

Fixed in v1.10.16: the comparison now strips an optional leading `v`/`V` before
comparing, so `v1.10.15` and `1.10.15` are correctly treated as the same version.
This is comparison-only — `mdb` output, the footer, and webhook payloads all still
show exactly what was actually set/reported, nothing about display changed.

**Your existing `expected_version=v1.10.15` setting is unaffected and still
correct** — you don't need to change it. This fix just means the next person
won't need to know about the prefix gotcha at all.

### 2. The hostname collision is fixed by changing the default, not by requiring you to remember

Same story: the console's fallback (when `PHAROS_CONSOLE_HOSTNAME` is unset) used
to be the bare container hostname, which collided with `pharos-01`'s existing
`pharos-pulse` record the moment you (correctly) set `--hostname` to match the
physical host — exactly what you're already doing and should keep doing.

Fixed in v1.10.16: the fallback default is now `<hostname>-console` instead of the
bare hostname, so this collision can't happen even if `PHAROS_CONSOLE_HOSTNAME` is
left unset.

**Your explicit `PHAROS_CONSOLE_HOSTNAME=pharos-01-console` setting is still the
right call and doesn't need to change** — being explicit in Terraform is better
than relying on any default, this fix just means a *future* console deployment
that forgets to set it won't collide either.

### Net effect

Nothing you already did in Terraform needs to be touched. This release just means
the two things you had to diagnose and work around by hand can't bite the next
deployment (or the next engineer who doesn't have this context) the same way.

Both incidents are now permanent regression tests in Pharos's own test suite
(`pharos-server/tests/console_version_hardening_live.rs`), so this exact class of
bug is now guarded against on every future change, not just fixed once.

Bump your `pharos_console_web_version` Terraform variable to `1.10.16` whenever
convenient — no urgency, since nothing you're running today is actually broken,
this just closes the gaps you found.
