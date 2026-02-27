<!--
/* ========================================================================
 * Project: pharos
 * Component: Documentation / Architecture
 * File: mcp-pharos-spec.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Defines the Model Context Protocol (MCP) and WebMCP server implementation
 * for the Pharos Web Console, enabling AI Agents and non-technical human 
 * users to manage infrastructure, SSH keys, and provisioning securely.
 * * Traceability:
 * Related to the "Pharos Web Console" Architecture and Pulse Agent definition.
 * ======================================================================== */
-->

# Model Context Protocol (MCP) & WebMCP Specification

## 1. Context & Rationale
To support Home Labbers and non-technical staff using LLMs (AI Agents) to manage infrastructure, Pharos exposes an MCP and **WebMCP** Server via the **Pharos Web Console**. 

**Note on Separation of Concerns (Critical)**: 
- **Marketing & Documentation Site** (`iamrichardd.com/pharos/`): Static technical documentation and architecture. **No dynamic state.**
- **Pharos Web Console** (`pharos.internal`): Dynamic, authenticated interface co-hosted with `pharos-server` or as a standalone service. This site implements **WebMCP** patterns, allowing the browser to act as a secure bridge for AI agents.

## 2. Server & Web Architecture
- **Location**: Runs as an API module and client-side bridge within the **Pharos Web Console** (Next.js/React).
- **Communication**: 
    - **Stdio/SSE**: For backend-to-agent communication.
    - **WebMCP**: For browser-to-agent communication, replacing DOM scraping with structured JavaScript tool calls.
- **Human/AI MDB Interface**: Provides a searchable, editable view of machine/infrastructure records (the "Web version of `mdb`").

## 3. WebMCP & MCP Tools Definition

### 3.1 `query_mdb` (High Priority)
Enables searching and filtering of machine/infrastructure records.
- **Input parameters**:
  - `query` (string, required): Search term (e.g., "proxmox-01", "192.168.1.50").
- **Output**: JSON representation of matching `mdb` records.

### 3.2 `upsert_record` (High Priority, Requires HitL)
Adds or updates a record in the Pharos server. 
- **Verification**: The Web Console will trigger a browser-level modal for user confirmation.
- **Input parameters**:
  - `type` (string, required): `people` | `machine`.
  - `data` (object, required): The record fields (hostname, IP, owner, etc.).
- **Output**: Success confirmation or permission denial.

### 3.3 `add_ssh_key`
Enrolls a new public SSH key into the Pharos server's authorization store.
- **Input parameters**:
  - `pub_key` (string, required): The public SSH key content (e.g., `ssh-ed25519 ...`).
  - `team_scope` (string, optional): The LDAP or internal group this key should be mapped to.
- **Output**: Success confirmation.

### 3.4 `revoke_ssh_key` (Requires HitL)
Removes a public key to immediately cut off write access. 
- **Input parameters**:
  - `pub_key_fingerprint` (string, required): The SHA256 fingerprint.
- **Output**: Confirmation of revocation.

### 3.5 `create_provisioning_token` (Requires HitL)
Generates a short-lived token for `pharos-pulse` auto-enrollment.
- **Input parameters**:
  - `team_scope` (string, required): The team to map the identity to.
- **Output**: Token and instruction snippet.

## 4. Security Enforcement
The WebMCP/MCP layer does NOT execute commands directly. It acts as an API gateway that translates agent intents into structured internal RPC calls to `pharos-server`. The backend server still enforces the Triple-Tier Security Model (`open`, `protected`, `scoped`), ensuring AI Agents cannot bypass multi-tenant isolation.

