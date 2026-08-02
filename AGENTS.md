# AGENTS.md — Engineering process for AI agents working on Pharos

This file is tool-agnostic. It's read by whatever AI coding agent is working in this repo
(Claude Code via `CLAUDE.md`, Gemini-family tooling via `GEMINI.md` — both point here) and
describes *how work actually gets done* on this project, distilled from real fixes shipped
across multiple sessions. `GEMINI.md` covers persona/architecture/product context; this file
covers the engineering workflow itself.

## Skills

`.agents/skills/` is the **single real, canonical location** for every one of this project's Agent
Skills (the open `SKILL.md` frontmatter+Markdown format — the same file format works for Claude
Code, Antigravity, and Gemini CLI; only the default lookup directory differs per tool). Both
`.claude/skills` and `.gemini/skills` are directory-level symlinks pointing at `.agents/skills` —
not per-skill symlinks. That means **adding a new skill is just one step**: create
`.agents/skills/<name>/SKILL.md`, and it's automatically visible to every tool through its own
symlinked directory, with nothing else to wire up. Never create a skill directly inside
`.claude/skills/` or `.gemini/skills/` — those paths only exist as symlinks now; writing there
writes through to `.agents/skills/` anyway, so just target `.agents/skills/` directly for clarity.

## Ground rules

- **Never commit, push, tag, or close a GitHub issue without an explicit instruction each time.**
  A prior "commit and push" does not carry forward to the next change — ask or wait to be asked
  again for every distinct change.
- **Zero-Host Execution is the default, and stays absolute for implementation and pre-flight.**
  The only stage permitted to run on the host is independent review/live-verification of
  already-implemented work, conditioned on the host's Rust toolchain matching
  `rust-toolchain.toml` exactly. See `GEMINI.md` / `CONTRIBUTING.md` for the full policy and the
  "plan → builder → panel → remediate" section below for exactly which stage that is.
- Track legitimate-but-out-of-scope findings as real GitHub issues, cross-referenced in `TODO.md`
  under a `Debt #NN (Issue #NNN)` convention in "Phase 25: Fail Fast Architectural Debt." Don't let
  scope creep into an in-progress fix, but don't let real findings get lost either.

## Verify with live execution, not static reading or self-reports

Never declare a fix complete based on unit tests, a diff review, or a subagent's self-reported test
output alone. Independently re-run verification, and specifically try to break the fix with a
harder or more realistic input than whatever was already tested: a different code path, a real
environment's actual state (not a simplified mock), or the entire log/output artifact (not just the
line you expect to check).

This is not a theoretical concern — it's the exact reason two real, ship-blocking bugs were caught
before shipping in this project's history:
- A logging-redaction fix correctly fixed the exact bug reported and passed every test — but a
  second, structurally different logging call site (using string `Display` formatting rather than
  the type's `Debug` formatting) leaked the same class of secret. Only found by grepping the entire
  log file, not just the line the fix targeted.
- A cert/SAN drift-detection fix passed all tests written against simplified, mocked IPv4-only
  input. Testing against a real machine's actual network state (which, like most real hosts,
  included IPv6 addresses) surfaced a guaranteed false-positive from a case-sensitivity mismatch
  between two tools' output formats.

Both looked completely correct under the verification that was actually run. **Verification that
passes is not evidence of correctness — it's only evidence the test wasn't hard enough.** For
anything empirically uncertain (an external tool's exact output format, a library's exact
behavior), verify it directly before designing around an assumption.

For any release, the gold-standard final check is downloading the actual published GitHub release
artifact (binary or script) and testing against *that* — not a local build. CI passing and local
builds working are necessary but have not been sufficient on their own.

## The plan → builder → panel → remediate workflow

For any non-trivial fix, this is the standing process — **each stage has a specific, non-negotiable
execution location**:

1. **Plan** (host — no code execution happens here, so Zero-Host doesn't apply). Re-read the actual
   current source fresh before writing anything — don't rely on remembered line numbers or state
   from earlier in a session; they drift as other fixes land. The plan must be fully self-contained
   (a fresh, zero-context agent will execute it verbatim) and include:
   - The exact current code, quoted — not a paraphrase.
   - An **explicit non-goals section**: what NOT to touch and why. This is the single most
     effective guard against scope creep and gives the builder a clear boundary.
   - Exact verification steps with concrete example values to reproduce, not just "test it."
   - Any empirically-uncertain external behavior (tool output formats, library semantics) verified
     directly first, with the confirmed real output written into the plan.
   - Write the plan to a durable file the builder can read directly, not just inline conversation.

2. **Builder — implementation execution** (**Podman, strictly, no exception** — see
   `skill-pharos-developer` / `skill-pharos-preflight`). Dispatch a cheap/fast model to execute the
   plan exactly as written — not to redesign or improvise. A fresh agent has zero context, so the
   prompt must be fully self-contained, explaining the *why* behind the fix, not just the *what*.
   All builds and tests it runs happen inside `Containerfile.test`/`Containerfile.debug`.

3. **Panel review AND live-verification test** (**host, but only when the carve-out condition is
   met** — see `skill-pharos-auditor`). Independently re-verify: check the actual diff against the
   plan, re-run build/test/lint, and redo live verification with harder/more-real inputs than the
   builder used. Do not just read the builder's report and trust it. Before running anything on the
   host for this stage, confirm `rustc --version` matches `rust-toolchain.toml` exactly — if it
   doesn't, do this stage in Podman too instead.

4. **Remediate — back to Podman** (implementation work again, same strict rule as step 2). If
   review finds a real issue, treat it as something to fix, not just report. Send the specific
   remediation back to the *same* builder session if the tooling supports resuming it — this
   preserves already-loaded file context and is cheaper than starting over. Its tests run in Podman.

5. **Repeat 3–4 until clean**, then final re-verification (step 3's conditions again) before
   considering it done.

Report status at each transition (plan complete → builder dispatched → panel found X / clean →
remediation dispatched → final result), not only at the very end.

## Cutting a release

1. Bump `VERSION=` near the top of `scripts/install.sh`.
2. Commit that one-line change, push.
3. `git tag -a vX.Y.Z -m "..."`, push the tag.
4. Watch CI to completion.
5. Verify all release assets actually published — expect exactly 17 binaries (as of v1.9.0):
   `{ph,mdb,pharos-pulse,pharos-scan,pharos-server}` × `{linux-x86_64, linux-aarch64,
   windows-x86_64.exe}`, plus `{ph,mdb}` × `macos-aarch64` only — macOS no longer ships
   `pharos-server`/`pharos-scan`/`pharos-pulse` (client-tools-only platform), and there is no
   `macos-x86_64` (Intel Macs are unsupported by design).
6. **Non-negotiable:** download the actual published binary/script and live-test it against
   whatever the release specifically changed — never consider a release verified from a local
   build alone.
7. Update `TODO.md`'s corresponding `Debt #NN (Issue #NNN)` line to `[x]`, commit, push.

## TODO.md / GitHub Issues sync

`TODO.md`'s checkboxes and GitHub Issues' open/closed state drift out of sync silently, in both
directions:

- A commit message containing `Fixes #NNN`, once pushed to the default branch, auto-closes the
  GitHub issue — but does **not** touch `TODO.md`. That's a separate, deliberate commit.
- If a shipped fix later turns out incomplete, the issue must be explicitly reopened **and**
  `TODO.md`'s checkbox flipped back to `[ ]` in the same breath — forgetting either half
  re-creates drift that has already required a manual audit-and-reconcile pass once in this
  project's history.

When asked whether local tracking is in sync with GitHub, actually run the cross-reference
(`gh issue list --state open` against every `TODO.md` `[x]` line) — don't assume it's current.

## Triage: bug vs. feature, and what's next

Classify by what problem is solved, not by whether new code was written.
- **Bug fix**: closes a gap in an *existing* capability that causes a real, often-silent failure
  under a real-world condition the original implementation didn't account for. New code is very
  often required to fix a bug — that doesn't make it a feature.
- **New feature**: adds a capability that never existed before. Nothing was broken before; the
  capability just didn't exist.
- The repo's own `bug`/`enhancement`/`documentation` GitHub labels are a reliable secondary signal.

When asked what to tackle next, re-derive the current open-issue list fresh (`gh issue list
--state open`) rather than reusing a memorized list, then rank:
1. Security-sensitive bugs with silent real-world failure modes.
2. Correctness bugs causing silent misconfiguration or operational confusion.
3. Documentation-only defects (no functional risk).
4. Genuine net-new feature requests.

## Repo conventions worth knowing before assuming

- `scripts/install.sh` ends with an unguarded `main "$@"` — never `source` it directly for
  testing, it will run the whole installer. To test one function in isolation, extract it with
  `sed -n '/^funcname()/,/^}/p' scripts/install.sh` into a throwaway harness with stubbed
  `log()`/`warn()`/`SUDO=""`.
- `pharos-server/src/auth.rs`'s `AuthManager::reload()` is the canonical hot-reload pattern for
  this codebase: an `RwLock<T>` swapped via
  `match lock.write() { Ok(mut guard) => { *guard = new_value; info!(...) } Err(e) => error!(...) }`,
  never `.unwrap()`/`.expect()` on the lock itself, with "if the reload finds nothing usable, keep
  the previous state" as the failure-mode default. New hot-reload features should mirror this idiom
  rather than inventing a different concurrency pattern.
- `cargo clippy -p pharos-server` fails on 2 known, pre-existing, unrelated errors inside the
  `pharos-client` crate. Confirm any clippy failure is actually new (e.g. via `git stash` and
  re-running) before treating it as a regression.
- `pharos-server`'s binary-target tests (anything in `main.rs`'s own `#[cfg(test)]` module) run
  under `cargo test -p pharos-server` but are excluded by `cargo test -p pharos-server --lib`
  (library-only). Not a bug — run the full command (no `--lib`) to see both.
