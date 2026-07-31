---
name: pharos-fix-workflow
description: Plan -> cheap-builder -> panel-review -> remediate workflow for implementing a non-trivial Pharos bug fix or GitHub issue end-to-end, with independent live-verification and a strict per-stage Zero-Host/carve-out split.
---

# Pharos fix workflow

Use this when asked to implement a non-trivial fix or GitHub issue on Pharos — not for trivial
one-line changes, and not when the user has already specified a different process.

Full background and rationale: `AGENTS.md` at the repo root. This skill is the actionable version
of that document's "plan → builder → panel → remediate" section. Related: `skill-pharos-developer`
(implementation), `skill-pharos-preflight` (pre-commit gate), `skill-pharos-auditor` (review
carve-out), `skill-pharos-sync` (issue/TODO tracking).

## Each stage has a specific, non-negotiable execution location

1. **Plan** — host. No code executes here, so Zero-Host doesn't apply.
2. **Builder / implementation execution** — **Podman, strictly, no exception.**
3. **Panel review AND live-verification test** — **host, but only when the carve-out condition is
   met** (host `rustc --version` matches `rust-toolchain.toml` exactly). If it doesn't match, this
   stage runs in Podman too.
4. **Remediation** — back to Podman (implementation work again, same rule as step 2).
5. Repeat 3–4 until clean.

## Steps

1. **Plan (host).** Re-read the actual current source fresh — don't rely on remembered line
   numbers or file state from earlier in the conversation, other fixes may have shifted them. Grep
   broadly for every place the bug's root cause could also manifest (a similar-looking but
   structurally different code path is exactly the kind of thing that hides a second instance of
   the same bug — this has happened twice on this project already). Write a fully self-contained
   plan to a durable file. It must include:
   - The exact current code, quoted verbatim — not paraphrased.
   - An explicit **non-goals** section: what NOT to touch and why. This is the single most
     effective guard against a cheap builder over-reaching or under-scoping.
   - Concrete verification steps with real example values to reproduce — not just "test it."
   - Any empirically uncertain external behavior (a CLI tool's exact output format, a library's
     exact semantics) verified directly *before* writing the plan, with the confirmed real output
     included in the plan.

2. **Builder (Podman only).** Dispatch a cheap/fast model as a fresh subagent with the plan,
   instructed to execute exactly as written — not redesign or improvise — and to run every
   build/test inside `Containerfile.test`/`Containerfile.debug`, never on the host. Since it starts
   with zero context, explain the *why* behind the fix in the prompt, not just the *what*, and tell
   it explicitly not to commit or push.

3. **Panel review (host, carve-out gated) — done independently, don't just trust the builder's
   report:**
   - Before running anything on the host for this stage, confirm `rustc --version` matches
     `rust-toolchain.toml`. If it doesn't, do this stage in Podman instead.
   - Check the actual diff against the plan.
   - Re-run build/test/lint yourself.
   - Redo live verification yourself, specifically trying an input class harder or more realistic
     than whatever the builder tested — e.g. this machine's *real* environment state instead of a
     simplified mock, the *entire* output/log artifact instead of just the expected line, a
     structurally different code path that does something similar.

4. **Remediate (Podman only) — don't just report a finding, fix it.** Resume the same builder
   session (not a fresh one) with the specific fix needed, so it keeps its already-loaded file
   context, and keep its tests in Podman.

5. **Final independent re-verification** (step 3's conditions again) after remediation, before
   considering it done.

6. **Report status at each transition** (plan complete → builder dispatched → panel found X /
   clean → remediation dispatched → final result) — not only once at the very end.

## Non-negotiable

Never declare a fix complete based on unit tests or a subagent's self-report alone. See
`AGENTS.md`'s "Verify with live execution" section — this exact gap shipped two real bugs on this
project (a second logging leak via a different code path; a false-positive from IPv6 case handling
that only surfaced against real network state) before being caught by independent live
re-verification.

Never commit, push, tag, or close an issue without an explicit instruction — a prior approval does
not carry forward to the next fix.
