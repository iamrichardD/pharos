---
name: skill-pharos-sync
description: Synchronize Pharos project state between @TODO.md, @PROGRESS.md, and GitHub Issues. Use this skill when starting new tasks, completing work, performing end-of-session state reconciliation, or when asked whether local tracking is in sync with GitHub. Also use this skill as a GATEKEEPER when a prompt starts with "bug report", "feature request", or "feature update request" to enforce documentation before implementation.
---

# Pharos Synchronization Protocol

This skill enforces the "Single Source of Truth" for the Pharos project by reconciling the local tracking files with the GitHub issue tracker.

## 🛑 STRICT MANDATE: THE GATEKEEPER
When a user starts a prompt with **"bug report"**, **"feature request"**, **"feature update request"**, or when the agent identifies a **tracking intent** (creating a log or issue), you MUST follow the **Document then Stop** workflow:
1. **Document**: 
    - **Check Existing**: Search GitHub Issues and local tracking files for a related open issue.
    - **Scope Expansion**: If a related issue exists, document the new findings or requirements as a detailed comment on the existing issue to expand its scope.
    - **New Task**: If no related issue exists, execute the `sync-init` workflow (Create GH Issue, update `@TODO.md` and `@PROGRESS.md`).
2. **Summarize**: Provide a concise summary of the documented task (or scope expansion) and the verification strategy.
3. **STOP**: Do NOT proceed to implementation. Inform the user that the task is documented and you are stopping to allow for a clean session transition.

## Core Workflows

### 1. Task Initialization (`sync-init`)
When starting a new task from the backlog:
- **Pre-check Naming Protocol**: You MUST read `@TODO.md` before calling `gh issue create` to ensure the **Task ID** (e.g., `16.4`) and title alignment follow existing patterns.
- **Search Mandate**: Search BOTH `@TODO.md` and `@PROGRESS.md` for the proposed Task ID and Issue ID to ensure they are not already in use.
- **ID Assignment**: If a task is new and not in the backlog, assign the next available incremental ID for that phase.
- **Label Validation**: Always run `gh label list` before creating an issue. If a required label is missing, you MUST create it using `gh label create [NAME] --color [HEX]`.
- **Mandate**: Use `gh issue create` with the prefix `Task X.Y: [Title]`, `Bug #Z: [Title]`, or `Debt #A: [Title]`.
- **Update**: Immediately add the resulting `(Issue #ID)` to the corresponding line in `@TODO.md`.
- **Assignment**: Ensure the issue is assigned to the current agent and tagged with the correct `phase-X` label.

### 2. Progress Documentation (`sync-update`)
During active development:
- **Commentary**: Periodically update the GitHub issue with progress comments to ensure "Human/AI Handover" continuity.
- **Traceability**: Ensure every commit message references the Task ID (e.g., `feat(auth): add login logic (Task 16.4)`).

### 3. Task Closure (`sync-close`)
When a task meets the "Definition of Done":
- **Verification**: MUST perform **Production Verification**. Use `web_fetch` to confirm changes are live and functional on `https://iamrichardd.com/pharos/`.
- **Summary**: Extract the "Fix Summary" and "Verification Prompt" from the implemented changes.
- **GitHub**: Post a final comment on the GitHub issue containing:
  - **Fix Summary**: High-level description.
  - **Security Review**: Explicit confirmation that the implementation adheres to `SECURITY.md`.
  - **Production Verification**: Confirmation that the live site was inspected and is correct.
  - **AI-Ready Verification Prompt**: The exact Podman command for local verification.
- **Close**: Close the GitHub issue.
- **TODO**: Mark the checkbox `[x]` in `@TODO.md`.

### 4. Reconciliation Sweep (`sync-audit`)
Before concluding a session, or whenever asked whether local tracking is in sync with GitHub —
don't assume `@TODO.md` is current just because it's usually kept up to date:
- **Compare**: `gh issue list --state open --limit 50 --json number,title,labels`, then for each
  open issue number grep `@TODO.md` for it (`grep -n "Issue #NNN" @TODO.md`) to check its checkbox
  state. Present findings as a table (issue #, title, TODO status, GH status), not prose.
- **Repair**:
  - If GH is closed but TODO is `[ ]`, mark TODO complete.
  - If TODO is `[x]` but GH is open, **do not blindly close the GH issue to match TODO** — first
    check *why* it's open. Two different situations look identical at a glance but need opposite
    handling:
    - The work was genuinely done and the issue was just never explicitly closed (e.g. no `Fixes
      #NNN` commit ever landed) → close it now with a resolution comment citing the actual
      evidence (commit hash, verified behavior) — don't close blind off the checkbox alone, check
      the real code/behavior first.
    - The issue was reopened on purpose because review/live-verification found the original fix
      incomplete (check the issue's comment history for this) → leave it open; this is a known,
      legitimate state, not drift. Confirm a follow-up fix is tracked instead.
  - If prefixes are missing from GH titles, add them using `gh issue edit`.
- **When a fix is found incomplete during review** (not just during the periodic sweep above): do
  both of the following immediately, in the same breath, not just one — `gh issue reopen NNN` with
  a comment explaining what the follow-up gap is, **and** flip `@TODO.md`'s checkbox for that item
  back to `[ ]`. Forgetting either half is exactly the drift this sweep exists to catch.

## Standards & Formatting

- **GitHub Titles**: MUST start with `Task X.Y: `, `Bug #Z: `, or `Debt #A: `.
- **Issue Labels**: `enhancement`, `bug`, `documentation`, `phase-X`.
- **Closure Comment**:
  ```markdown
  **Fix Summary**
  [Clear, high-level description of what changed]

  **AI-Ready Verification Prompt**
  `podman run --rm ... [exact command]`
  ```
