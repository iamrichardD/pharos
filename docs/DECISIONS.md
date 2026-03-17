/* ========================================================================
 * Project: pharos
 * Component: Documentation
 * File: docs/DECISIONS.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This file codifies the "Why" behind the architectural and user-facing
 * decision paths. It serves as a bridge for both human operators and
 * AI Agents to understand the system's intent and boundaries.
 * * Traceability:
 * Related to Phase 17 Gap Analysis.
 * ======================================================================== */

# Pharos Decision Matrix

This guide helps you navigate the Pharos ecosystem by mapping your intent to the most effective tool and configuration.

## 1. Choosing Your Interface
We provide multiple ways to interact with Pharos. Choose the one that best fits your current context.

| If your goal is... | Use this tool... | Because... |
| :--- | :--- | :--- |
| **Rapid searching** | `ph` or `mdb` CLI | It's local, pipeable, and requires zero context switching. |
| **Bulk automation** | `pharos-client` lib | It provides programmatic access with built-in SSH-signing. |
| **Visual oversight** | **Web Console** | It maps infrastructure relationships (IP/Hostname) visually. |
| **AI Management** | **MCP Server** | It provides a secure "Human-in-the-Loop" bridge for agents. |

## 2. Selecting a Storage Tier
Pharos is designed to grow with your environment, from a single laptop to a global enterprise.

- **IF** You are developing or testing **THEN** use **MemoryStorage** (`PHAROS_STORAGE_PATH` is unset).
    - *Success Factor:* Zero-configuration, sub-millisecond latency.
- **IF** You are a Home Labber (Single-Node) **THEN** use **FileStorage** (`PHAROS_STORAGE_PATH=/path/to/pharos.json`).
    - *Success Factor:* Simple backups, restart-survivable, no database to manage.
- **IF** You are an Enterprise Engineer **THEN** use **LdapStorage** (`PHAROS_LDAP_URL=...`).
    - *Success Factor:* Centralized identity, scales with your existing directory service.

## 3. Navigating the Security Boundary
Pharos balances "Read-Optimized" openness with "Write-Authenticated" integrity.

```mermaid
flowchart TD
    Start([Request Received]) --> Command{Is it a Write command?}
    Command -- No (Query/Status) --> Tier{Security Tier?}
    
    Tier -- Open --> Allow([Allow Unauthenticated])
    Tier -- Protected/Scoped --> AuthCheck{Is User Authenticated?}
    
    Command -- Yes (Add/Change/Delete) --> AuthCheck
    
    AuthCheck -- No --> Challenge([Issue SSH Challenge])
    AuthCheck -- Yes --> Role{Role Check?}
    
    Role -- Admin Required --> IsAdmin{Is User Admin?}
    IsAdmin -- No --> Deny([403: Forbidden])
    IsAdmin -- Yes --> Execute([Execute Operation])
    
    Role -- None --> Execute
```

## 4. Mandatory Password Rotation (Home Lab)
To balance "Frictionless Setup" with "Day 1 Security," Pharos enforces a mandatory password update policy for the Web Console.

- **IF** The user logs in with the default password (`admin:admin`) **THEN** The session is flagged with `mustChangePassword: true`.
- **IF** The session is flagged **THEN** Middleware intercepts all requests and redirects the user to `/change-password`.
- **IF** The user submits a new password **THEN** The hash is saved to `data/auth_store.json` and the flag is cleared.

## 5. Machine Presence Metadata (Temporal Context)
To enable the **Tri-State Presence Model** (ONLINE, OFFLINE, UNREACHABLE), Pharos automatically injects temporal metadata into every machine record.

- **`created_at`**: Captured during the initial registration (first `ONLINE` signal).
- **`last_seen_at`**: Updated on every interaction (HEARTBEAT, ONLINE, manual `add`).
- **Standardization**: All timestamps are stored as **ISO8601 UTC strings**.
- **Naming**: We use `_at` suffix (e.g., `created_at`) rather than `_datetime` to align with modern REST and Graph APIs.

## 7. Human-Readable CLI Output (MDB)
To balance "Script-First" interoperability with "Human-First" readability, the `mdb` CLI implements a transform layer for display.

- **IF** The `--human` (or `-H`) flag is used **THEN** Raw values are transformed into human-friendly formats.
- **IF** The flag is omitted **THEN** Values are displayed in their raw, machine-readable protocol format (e.g., ISO8601, KB).
- **Success Factor:** Use `clap` for standardized argument parsing and `chrono` for precise temporal transformations.


## 6. Troubleshooting Connectivity
If a connection fails, follow this decision path:

3.  **Validate Keys:** Does the `PHAROS_KEYS_DIR` contain your `.pub` key?

## 8. Regional Date Formatting (Web Console)
To balance "Protocol Consistency" (UTC) with "Human Readability" (Regional), the Web Console implements a deferred formatting strategy.

- **IF** A machine record contains temporal fields (`created_at`, `last_seen_at`) **THEN** The server-side (SSR) renders them as raw ISO8601 strings within a `<time>` element.
- **IF** The page loads in a browser **THEN** A client-side script transforms these strings into the user's regional format using `Intl.DateTimeFormat`.
- **Rationale:** This preserves the "Single Source of Truth" in the protocol while providing a localized UX without requiring server-side locale tracking.

## 9. Agent-Native Infrastructure (Standard)
To align with modern DevSecOps and "Cloud-Native" paradigms, Pharos is designed as **Agent-Native Infrastructure**.

- **IF** An AI Agent needs to manage resources **THEN** It must use the **WebMCP JSON-RPC 2.0 gateway** (`/mcp`) rather than scraping the UI or using legacy terminals.
- **IF** The term "Sovereign" is used **THEN** It refers to the *territory* of the Sandbox, while "Native" refers to the *engineering standard* of the entire infrastructure.
- **Rationale:** "Native" implies that the agent is a first-class citizen with its own optimized, deterministic instruction set (tools), reducing hallucinations and operational friction.

## 10. Deterministic Infrastructure & Unified Source of Truth
To align with the "Collaborative Force Multiplier" role and the "Deterministic Infrastructure" narrative, Pharos serves as the high-rigor grounding layer for all network assets.

- **IF** There is a discrepancy between manual documentation and the Pharos registry **THEN** The Pharos registry is the **Unified Source of Truth**.
- **IF** An AI Agent or Human Actor requires infrastructure discovery **THEN** Pharos provides the deterministic data needed to eliminate the **"Hallucination Gap."**
- **Rationale:** High-rigor systems design reduces engineering toil and ensures that infrastructure state is observable, verifiable, and clinical.


