---
name: pharos-release-cut
description: The exact sequence for cutting a Pharos release (vX.Y.Z), including the non-negotiable live-download verification against the real published artifact.
---

# Pharos release cut

Use this when explicitly asked to "cut vX.Y.Z" (or equivalent). Do not cut a release unless asked —
this is a distinct, explicit action, not something to bundle automatically after a fix lands.

## Steps

1. Bump `VERSION=` near the top of `scripts/install.sh` to the new version.
2. Commit that one-line change, push.
3. `git tag -a vX.Y.Z -m "<one-line summary of what this release ships>"`.
4. Push the tag: `git push origin vX.Y.Z`.
5. Watch CI to completion (`gh run watch <run-id> --exit-status`). On this repo it reliably
   exceeds a 120s foreground timeout (multi-platform build + several GHCR image-publish jobs) —
   that's expected; let it background and wait for the notification rather than re-polling.
6. Verify release assets actually published:
   `gh release view vX.Y.Z --json assets`. Expect exactly 15 binaries:
   `{ph,mdb,pharos-pulse,pharos-scan,pharos-server}` ×
   `{linux-x86_64, macos-aarch64, windows-x86_64.exe}`.
7. **Non-negotiable — do not skip:** download the actual published binary or script (from the
   GitHub release download URL, or `raw.githubusercontent.com/.../main/...` for scripts) and
   live-test it against whatever this specific release changed. Never consider a release verified
   from a local build alone — CI passing and local builds working are necessary but have not been
   sufficient in this project's history (see `AGENTS.md`'s live-verification section for why). This
   is review-stage work, same carve-out as `pharos-fix-workflow` step 3 — running it on the host is
   fine only when `rustc --version` matches `rust-toolchain.toml`; otherwise do it in Podman.
8. Update `TODO.md`'s corresponding `Debt #NN (Issue #NNN)` line to `[x]`, commit, push.

## Notes

- A commit message containing `Fixes #NNN`, once pushed to the default branch, auto-closes the
  GitHub issue on its own — verify this happened (`gh issue view NNN --json state`) rather than
  assuming, and don't also manually close it (that would just be a redundant no-op, but confirm
  state either way before reporting the issue as closed).
- If step 7's live test finds a real gap the fix didn't cover, treat it exactly like a panel-review
  finding in `pharos-fix-workflow`: reopen the issue, flip `TODO.md` back to `[ ]`, write a
  follow-up plan, and cut the *next* patch version once it's fixed and re-verified — don't leave a
  known-broken release marked as the fix for an issue.
