---
name: skill-pharos-sync
description: Synchronize Pharos project state between @TODO.md, @PROGRESS.md, and GitHub Issues. Use this skill when starting new tasks, completing work, or performing end-of-session state reconciliation to ensure DORA metric integrity and traceability.
---

# Pharos Synchronization Protocol

This skill enforces the "Single Source of Truth" for the Pharos project by reconciling the local tracking files with the GitHub issue tracker.

## Core Workflows

### 1. Task Initialization (`sync-init`)
When starting a new task from the backlog:
- **Mandate**: Use `gh issue create` with the prefix `Task X.Y: [Title]` or `Bug #Z: [Title]`.
- **Update**: Immediately add the resulting `(Issue #ID)` to the corresponding line in `@TODO.md`.
- **Assignment**: Ensure the issue is assigned to the current agent and tagged with the correct `phase-X` label.

### 2. Progress Documentation (`sync-update`)
During active development:
- **Commentary**: Periodically update the GitHub issue with progress comments to ensure "Human/AI Handover" continuity.
- **Traceability**: Ensure every commit message references the Task ID (e.g., `feat(auth): add login logic (Task 16.4)`).

### 3. Task Closure (`sync-close`)
When a task meets the "Definition of Done":
- **Summary**: Extract the "Fix Summary" and "Verification Prompt" from the implemented changes.
- **GitHub**: Post a final comment on the GitHub issue containing the summary and the exact Podman command needed for verification.
- **Close**: Close the GitHub issue.
- **TODO**: Mark the checkbox `[x]` in `@TODO.md`.

### 4. Reconciliation Sweep (`sync-audit`)
Before concluding a session:
- **Compare**: List all open/closed issues on GitHub and compare them against `@TODO.md` and `@PROGRESS.md`.
- **Repair**:
  - If GH is closed but TODO is `[ ]`, mark TODO complete.
  - If TODO is `[x]` but GH is open, close the GH issue with a summary.
  - If prefixes are missing from GH titles, add them using `gh issue edit`.

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
