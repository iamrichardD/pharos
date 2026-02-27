<!--
/* ========================================================================
 * Project: pharos
 * Component: Documentation / Planning
 * File: pharos-web-console-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Resolves the conflation between the static marketing site and the dynamic 
 * Web Console. Defines the Roadmap for the Human/AI Web-enabled version of 
 * the mdb CLI, serving non-technical users and AI agents.
 * * Traceability:
 * Related to Phase 14 (Pulse & Multi-Tenant Architecture) and Phase 16.
 * ======================================================================== */
-->

# Pharos Web Console: Human/AI Interface Plan

## 1. Problem Statement
The current Pharos ecosystem lacks a unified, dynamic web interface for human users (non-technical staff) and AI Agents to interact with the system. Previous efforts conflated the **Static Marketing/Documentation Site** (iamrichardd.com/pharos) with the **Dynamic Console**. 

The missing component is a Web-based version of the `mdb` CLI that allows for:
- One-off device searches (for Home Labbers/Enterprise staff).
- One-off device additions/modifications.
- SSH Key management.
- Native WebMCP capabilities for AI Agents.

## 2. Strategic Remediation
The project will formally bifurcate web efforts into two distinct products:

### A. Pharos Documentation & Marketing (Static)
- **URL**: `https://iamrichardd.com/pharos/`
- **Tech Stack**: Astro (Static Site Generation).
- **Goal**: High-value technical documentation, architecture diagrams, and DORA metrics.

### B. Pharos Web Console (Dynamic)
- **Host**: Deployed alongside `pharos-server` (e.g., `https://pharos.internal/` or `localhost:3000`).
- **Tech Stack**: Next.js / React with Tailwind CSS.
- **Goal**: Functional parity with `mdb` and `ph` CLIs, plus WebMCP integration.

## 3. Product Roadmap (Prioritized)

### Phase 1: Interactive MDB (Highest Priority)
- **Feature**: Search interface for machine/infrastructure records.
- **Feature**: CRUD forms for adding/editing devices (one-off additions).
- **Target Persona**: The "Office Manager" or "Home Labber" who needs to quickly find an IP or add a new IoT device without touching a terminal.
- **AI Enablement**: Expose `search_mdb` and `update_record` as WebMCP tools.

### Phase 2: Pulse Monitoring & Identity
- **Feature**: Visual dashboard showing "ONLINE/OFFLINE" status of all nodes (via `pharos-pulse`).
- **Feature**: SSH Public Key management and provisioning token generation.
- **AI Enablement**: Expose `generate_provisioning_token` as a WebMCP tool.

### Phase 3: Network Discovery Integration
- **Feature**: Trigger `pharos-scan` jobs from the UI.
- **Feature**: Bulk-provision discovered devices into `mdb`.

## 4. Architectural Principles
- **Clean Architecture**: The Web Console is a *client* of the `pharos-server` API. No direct database access.
- **Security**: Must leverage the existing Triple-Tier Security model. All write operations from the Web Console require an authenticated session (mapped to an SSH key or LDAP credential).
- **AI Agent Native**: Tools defined in `artifacts/mcp-pharos-spec.md` must be natively available in the browser via WebMCP.

## 5. Implementation Schedule
- **Week 1**: Scaffold Next.js "Pharos Console" inside the `crates/pharos-console` or a new `web/console` workspace.
- **Week 2**: Implement read-only `mdb` view (Search/Filter).
- **Week 3**: Implement write forms for device management (Human-in-the-Loop required).
- **Week 4**: Finalize WebMCP bridge for AI Agent interaction.
