/* ========================================================================
 * Project: pharos
 * Component: Documentation / Planning
 * File: artifacts/web-mcp-alignment-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Resolves the "WebMCP Mirage" by bridging the gap between the RFC 2378 
 * legacy foundation and the AI-native WebMCP vision. Provides a roadmap
 * for the Agent-Sovereign implementation in the Web Console.
 * * Traceability:
 * Related to Phase 17 (Sandbox & WebMCP Integration) and Phase 22.
 * ======================================================================== */

# Strategic Alignment: WebMCP & Resource-First Console

## 1. Executive Summary
The Pharos ecosystem currently suffers from a "Persona Conflict" where the marketing site promises an AI-native WebMCP experience that is not yet fully implemented in the Web Console. This plan realigns the technical architecture with the marketing vision, transitioning from a "Fake Terminal" UX to a "Resource-First" Agent-Sovereign model.

## 2. Research Findings (The Discrepancy)
- **Endpoint Missing:** The promised `/mcp` web endpoint does not exist in the Astro Console.
- **Component Mismatch:** The only functional MCP server is a standalone Rust binary (`pharos-console`) operating over STDIO.
- **UX Redundancy:** The "Sandbox Terminal" mimics a CLI but provides less value than the structured `/mdb` search interface.
- **Agentic Gap:** AI agents are forced to scrape raw text instead of interacting with structured JSON-RPC tools.

## 3. Implementation Roadmap (Phase 22)

### 3.1 The WebMCP Sovereign Bridge
- **Endpoint Implementation:** Create `pharos-console-web/src/pages/mcp.ts` as a JSON-RPC 2.0 gateway.
- **Tool Unification:** Standardize the toolset (e.g., `query_mdb`, `provision_node`) between the Rust server and the Web Console.
- **Auth Handshake:** Ensure the `/mcp` route utilizes the existing JWT session logic to authorize agent actions on behalf of the user.

### 3.2 Resource-First Realignment
- **Deprecate "Fake Terminal":** Replace the simulated terminal in the Sandbox with a "Live Resource Preview" of the actual `/mdb` UI.
- **Metadata Injection:** Add machine-optimized metadata blocks to `[hostname].astro` to allow agents to "glance" at records without complex scraping.
- **HitL Integration:** Implement mandatory browser-level confirmation for destructive tool calls issued via WebMCP.

### 3.3 Marketing & Docs Surgery
- **Authority Pivot:** Update `index.astro` and `console.mdx` to emphasize the **Web Console** as the primary Human/AI interface.
- **Backplane Story:** Position **RFC 2378** as the "Invisible High-Performance Engine" and WebMCP as the "Sovereign Cockpit."
- **Transparency:** Explicitly document the "Read-Only" nature of the current LDAP Enterprise Tier to maintain engineering integrity.

## 4. Verification Strategy (TDD-First)
- **E2E Tests:** Create `tests-e2e/mcp.test.ts` to verify the JSON-RPC 2.0 handshake at the `/mcp` endpoint.
- **Schema Validation:** Use `vitest` to ensure the `query_mdb` tool returns standardized `snake_case` JSON matching the `pharos-pulse` inventory spec.
- **Pre-Flight:** Execute `scripts/pre-flight.sh` inside the Podman test environment before finalization.

## 5. Definition of Done (DoD)
- [ ] `/mcp` endpoint returns valid JSON-RPC 2.0 responses.
- [ ] AI Agents can successfully call `query_mdb` via the Web Console.
- [ ] Sandbox Terminal is replaced with a "Live Resource-First" preview.
- [ ] Marketing site and documentation are 100% synchronized with the implementation.
- [ ] DORA metrics reflect the completion of the WebMCP modernization.
