---
name: pharos-issue-triage
description: Classify a Pharos issue as bug vs. feature, and rank open issues by priority, using the heuristics validated across this project's history.
---

# Pharos issue triage

Use this when asked to classify an issue (bug vs. feature) or to identify what to work on next
among open issues.

## Bug vs. feature

Classify by what problem is solved, not by whether new code was written.

- **Bug fix**: closes a gap in an *existing* capability that causes a real, often-silent failure
  under a real-world condition the original implementation didn't account for. Examples from this
  project: secrets leaking into logs, a certificate's SAN not covering a new IP, a reload signal
  handler not covering all the state it should, documentation claiming an env var does something it
  doesn't. New code — sometimes a lot of it — is very often required to fix a bug. That doesn't
  make it a feature.
- **New feature**: adds a capability that never existed before. Nothing was broken before; the
  capability just didn't exist yet.
- The repo's own `bug` / `enhancement` / `documentation` GitHub labels are a reliable secondary
  signal — check them, but be able to justify the answer independent of the label too.

## What's next

Re-derive the current open-issue list fresh every time (`gh issue list --state open --limit 50
--json number,title,labels`) — don't reuse a memorized list from earlier in the conversation or a
prior session, it churns. Then rank:

1. Security-sensitive bugs with silent real-world failure modes (secrets exposure, silent auth
   bypass, silent data exposure).
2. Correctness bugs causing silent misconfiguration or operational confusion (a validated-but-unused
   env var, stale state with zero detection signal).
3. Documentation-only defects (diagram/prose doesn't match code, no functional risk).
4. Genuine net-new feature requests (`enhancement` label, nothing broken today).

Before recommending an issue as "next," it's worth a quick sanity check that it's still actually
valid against current source (grep for the specific claim in the issue body) — issues can go stale
if related code changed since they were filed.

## How to answer

Give the direct classification/recommendation with one or two sentences of reasoning — not an
exhaustive survey of every option. This project's maintainer has consistently preferred terse,
confident, evidence-backed answers over long write-ups for these kinds of questions.
