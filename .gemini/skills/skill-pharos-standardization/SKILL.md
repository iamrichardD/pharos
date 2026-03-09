---
name: skill-pharos-standardization
description: Enforce the Pharos Standardized File Prologue and project headers across all source files. Use this skill when creating new files, modifying existing code, or performing workspace-wide compliance audits to ensure AGPL-3.0 attribution and traceability.
---

# Pharos Standardization Protocol (The Janitor)

This skill enforces the structural and legal standards defined in `GEMINI.md`. It ensures that every source file reflects the project's identity, licensing, and purpose.

## Core Workflows

### 1. Header Injection (`std-inject`)
When creating or modifying a file:
- **Mandate**: Every source file MUST begin with the Standardized File Prologue block.
- **Action**: Use the `prologue_injector.js` script (or manual implementation) to prepend the block.
- **Mapping**:
  - `pharos-server/` -> Component: Server Core
  - `ph/` -> Component: CLI-ph
  - `mdb/` -> Component: CLI-mdb
  - `pharos-console-web/` -> Component: Web Console
  - `pharos-pulse/` -> Component: Pulse Agent
  - `pharos-scan/` -> Component: Pharos-Scan

### 2. Header Auditing (`std-audit`)
Before any commit:
- **Mandate**: Verify that all files staged for commit contain the prologue.
- **Action**: Run a workspace-wide grep to identify missing or malformed headers.

### 3. Vertical Slice Alignment (`std-vsa`)
When implementing features:
- **Mandate**: Group logic by feature (Vertical Slice) rather than technical layer.
- **Action**: Ensure files are located in the appropriate `features/` directory within the Web Console or appropriate crate in the server.

## The Standardized File Prologue

```javascript
/* ========================================================================
 * Project: pharos
 * Component: [Component Name]
 * File: [filename]
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * [1-3 sentences explaining exactly why this file exists.]
 * * Traceability:
 * [Related to GitHub Issue #X, implements RFC 2378 Section Y]
 * ======================================================================== */
```

## Implementation Standards

- **TDD First**: Every change MUST have a corresponding test.
- **Zero-Host**: All execution (test/build) MUST be inside Podman.
- **Clean Code**: Favor simplicity and semantic naming over clever abstractions.
