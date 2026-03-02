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

## 4. Troubleshooting Connectivity
If a connection fails, follow this decision path:

1.  **Check the Port:** The Pharos protocol defaults to **2378** (TCP).
2.  **Verify the Tier:** Is the server in `Protected` mode? (Check `PHAROS_SECURITY_TIER`).
3.  **Validate Keys:** Does the `PHAROS_KEYS_DIR` contain your `.pub` key?
