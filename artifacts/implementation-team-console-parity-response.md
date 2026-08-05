## Response: pharos-console-web version drift (v1.10.11 vs. fleet v1.10.14)

Thanks for catching this — the root cause you found was correct: the systemd unit
pins the `:latest` tag but only pulls when there's no local image cache at all, so
it silently never moved after the Issue #3 migration wiped that cache once. Nothing
in Pharos itself could see or alert on this, which is the deeper gap v1.10.15 closes.

### What shipped in v1.10.15

1. **`pharos-console-web` now self-reports its own version into Pharos.** On startup,
   and every 60 minutes after, it registers itself as a Machine record with its real
   version (sourced from the actual release tag the image was built from, not an
   internal file that was already out of sync with it). You can query it like any
   other machine:
   ```
   mdb hostname=<your-console-hostname>
   ```
   and you'll see a `version` field reporting exactly what's running.

2. **The console's own footer now shows it too** — `Pharos v1.10.15` — so anyone
   looking at the web UI directly can confirm the version without a query.

3. **`pharos-server` now alerts on version drift**, reusing the same Dead Man's
   Switch webhook/script mechanism you already have configured for presence
   alerting. Set an `expected_version` field on the console's record — **use the
   same `v`-prefixed format the console actually self-reports** (`version` comes
   through as `v1.10.15`, not `1.10.15` — the image tag itself has no `v`, but the
   self-reported field does; the comparison is an exact string match with no
   normalization, so a mismatched prefix here is a permanent false-positive, not a
   typo that self-corrects):
   ```
   mdb change hostname=<your-console-hostname> make expected_version=v1.10.15
   ```
   If the console's self-reported `version` ever disagrees with `expected_version`,
   you'll get the same webhook alert you already get for a node going stale —
   `event: "version_mismatch"` in the payload, with both values included.

4. **Set `PHAROS_CONSOLE_HOSTNAME` explicitly — don't rely on the fallback.**
   If unset, the console falls back to the container's own hostname. If your
   systemd unit/podman config sets `--hostname` to match the physical host (common
   practice, and likely what you're already doing), this **collides** with that
   host's existing `pharos-pulse`-owned machine record and every self-registration
   attempt fails with `511:Collision`. Give the console its own distinct hostname,
   e.g. `<host>-console`.

5. **`mdb change` on the console's own record needs the console's own key, not
   admin's** — records are owned by whichever key created them, and the console
   self-registers with its own baked-in identity. Setting `expected_version`
   requires authenticating as that key (or an owner/admin override, if your
   security tier supports one), not just any authorized admin key.

This is deliberately **alert-only**: Pharos will never auto-upgrade or auto-restart
anything, and it never reaches out to GitHub to figure out "the latest version"
itself — `expected_version` is always something you set, ideally in the same
Terraform apply/commit that bumps the pinned image tag, so the two can never
silently disagree without you finding out.

### To actually fix the drift you found

1. Pull and deploy `ghcr.io/iamrichardd/pharos-console-web:1.10.15` (note: no `v`
   prefix on the image tag itself, unlike the git release tag).
2. Set `PHAROS_CONSOLE_HOSTNAME` to something distinct from the host's own
   pulse-reported hostname (see above).
3. Set `expected_version=v1.10.15` — with the `v` prefix — on that host's record
   via the `mdb change` command above, using the console's own key.

### What's still on your side (not fixed by v1.10.15, tracked separately)

Pharos can now tell you *when* the console drifts, but it can't fix *how* your
deployment stays on a mutable `:latest` tag in the first place. Worth doing on
your end, independent of any future Pharos release:

- Move `pharos-console-web`'s deployment off `:latest` onto an explicit pinned
  version (ideally by digest, not just tag, since even a version tag can in
  principle be re-pushed) — matching how the rest of the fleet's binaries are
  already versioned.
- Make that pinned reference a Terraform-managed value, so a version bump is one
  deliberate `apply`, not a side effect of whatever happened to be in podman's
  local image cache at restart time.
- Whenever you bump that pinned value, also run the `mdb change ... expected_version=`
  command above in the same step — that's what lets Pharos's new alerting actually
  catch the next drift instead of just this one.

Happy to help scope that Terraform-side work if useful — it's deliberately kept
out of this Pharos release since it's fleet-deployment tooling, not something
the protocol/console themselves should own.
