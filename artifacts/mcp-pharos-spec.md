<!--
/* ========================================================================
 * Project: pharos
 * Component: Documentation / Architecture
 * File: mcp-pharos-spec.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Defines the Model Context Protocol (MCP) and WebMCP server implementation
 * for the Pharos Console (Dynamic Dashboard), enabling AI Agents to manage
 * SSH keys, multitenant enrollments, and infrastructure provisioning securely.
 * * Traceability:
 * Related to "Pulse Agent" Architecture definition and WebMCP Interview.
 * ======================================================================== */
-->

# Model Context Protocol (MCP) & WebMCP Specification

## 1. Context & Rationale
To support Home Labbers using LLMs to manage infrastructure (e.g., "Add my new proxmox node to my home lab"), Pharos exposes an MCP and **WebMCP** Server via the **Pharos Console**. 

**Note on Separation of Concerns**: 
- **Marketing Site** (`iamrichardd.com/pharos/`): Static documentation and architecture.
- **Pharos Console** (Dynamic Dashboard): Co-hosted with `pharos-server` or as a standalone LXC service. This site implements **WebMCP** patterns, allowing the browser to act as a secure bridge for AI agents.

## 2. Server & Web Architecture
- **Location**: Runs as an API module and client-side bridge within the **Pharos Console** (Node.js/TypeScript).
- **Communication**: 
    - **Stdio/SSE**: For backend-to-agent communication.
    - **WebMCP**: For browser-to-agent communication, replacing DOM scraping with structured JavaScript tool calls.
- **Authentication & Security**: 
    - Leverages existing **SSO/Session Cookies** from the Pharos Console login.
    - **Human-in-the-Loop (HitL)**: Destructive actions (e.g., `revoke_ssh_key`) MUST require a UI confirmation button in the Pharos Console browser tab before execution.
    - **Narrow Scoping**: Agents only see specific, hashed functions for tool discovery, preventing full DOM access.

## 3. WebMCP & MCP Tools Definition

### 3.1 `add_ssh_key`
Enrolls a new public SSH key into the Pharos server's authorization store.
- **Input parameters**:
  - `pub_key` (string, required): The public SSH key content (e.g., `ssh-ed25519 ...`).
  - `team_scope` (string, optional): The LDAP or internal group this key should be mapped to (for `scoped` tier). Defaults to the user's primary team.
  - `alias` (string, optional): A human-readable name for this key (e.g., "macbook-pro").
- **Output**: Success confirmation or permission denial error.

### 3.2 `revoke_ssh_key` (Requires HitL)
Removes a public key to immediately cut off write access. 
- **Verification**: Pharos Console will trigger a browser-level notification/modal for user confirmation.
- **Input parameters**:
  - `pub_key_fingerprint` (string, required): The SHA256 fingerprint or exact public key.
- **Output**: Confirmation of revocation.

### 3.3 `create_provisioning_token` (Requires HitL)
Generates a short-lived, single-use token that `pharos-pulse` can use to auto-enroll itself.
- **Input parameters**:
  - `team_scope` (string, required): The team to map the generated identity to.
  - `expires_in` (integer, optional): Minutes until expiry (default: 60).
- **Output**:
  - `token` (string): The one-time secure token.
  - `instructions` (string): A short snippet showing how to use the token with `pharos-pulse install`.

### 3.4 `query_telemetry`
Retrieves recent telemetry metrics from a specific node or group.
- **Input parameters**:
  - `target` (string, required): UUID or Hostname of the node.
- **Output**: JSON representation of the latest `pharos-pulse` payload.

## 4. Security Enforcement
The WebMCP/MCP layer does NOT execute commands directly. It acts as an API gateway that translates agent intents into structured internal RPC calls to `pharos-server`. The backend server still enforces the Triple-Tier Security Model (`open`, `protected`, `scoped`), ensuring AI Agents cannot bypass multi-tenant isolation.

