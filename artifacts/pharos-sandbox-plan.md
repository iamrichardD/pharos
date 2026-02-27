/* ========================================================================
 * Project: pharos
 * Component: Architecture Artifacts
 * File: pharos-sandbox-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This document outlines the architectural strategy for the "Pharos Sandbox,"
 * a one-click, ephemeral test environment designed for Zero-Host execution.
 * It ensures developers, home labbers, and enterprise engineers can stand up
 * the entire Pharos ecosystem with a single command and no host pollution.
 * * Traceability:
 * Related to Phase 16 planning, supports WebMCP integration and DORA metrics.
 * ======================================================================== */

# Pharos Sandbox: Ephemeral "Lab-in-a-Box" Strategy

## 1. Vision & Goals
The Pharos Sandbox provides a standardized, isolated environment for testing and developing the Pharos ecosystem. It adheres to the **Zero-Host Execution** mandate by containerizing all components and ensuring zero persistence on the host machine.

- **One-Click Startup:** A single command (`podman-compose up`) to stand up the full stack.
- **Zero Host Impact:** No modifications to host files, libraries, or network configurations.
- **Ephemeral Storage:** All data resides in `tmpfs` mounts, disappearing upon teardown.
- **Development Feedback Loop:** Integrated with the WebMCP site for live interactive documentation.

## 2. Technical Architecture

### 2.1 Service Stack (Compose)
The sandbox is orchestrated via `podman-compose` and includes:

1.  **`pharos-server`**: The core RFC 2378 engine.
    - Port: `2378` (mapped to host).
    - Storage: `tmpfs` mounted at `/var/lib/pharos`.
2.  **`pharos-pulse`**: Monitoring agent.
    - Monitors `pharos-server` health.
3.  **`pharos-scan`**: Network discovery engine.
    - Scans the internal container network (`pharos-net`).
4.  **`pharos-web`**: The Astro-based documentation/console site.
    - Port: `3000` (mapped to host).
    - Connects to `pharos-server` and `pharos-pulse` via internal DNS.
5.  **`pharos-shell`**: A pre-configured debugging environment.
    - Includes `ph`, `mdb`, and `gh` tools.

### 2.2 Network Topology
- **Network Name:** `pharos-net` (Bridge).
- **Service Discovery:** Standard container hostnames (e.g., `http://pharos-server`).

### 2.3 Ephemeral Data Strategy
To ensure a clean state every time:
- Use `tmpfs` for the server database.
- Use Docker/Podman named volumes for caches (e.g., `node_modules` for the website) to speed up restarts without polluting the host's project directory.

## 3. WebMCP & Console Integration
The sandbox serves as the primary development environment for the **WebMCP** initiative:

- **Live Docs:** The documentation site can detect the sandbox environment and enable "Interactive Shells" that query the local `pharos-server`.
- **MCP Host:** Developers can test LLM tool-calling by pointing their agents at the sandbox's MCP-compliant API.
- **Pulse Visuals:** Real-time system health metrics from `pharos-pulse` are rendered directly in the WebMCP dashboard.

## 4. Implementation Tasks (Phase 16)

1.  **Task 16.1:** Design `deploy/compose.yml` with `tmpfs` and network isolation.
2.  **Task 16.2:** Create a `Makefile` with `make lab` as the primary entry point.
3.  **Task 16.3:** Update `pharos-web` to support "Sandbox Mode" (interactive components).
4.  **Task 16.4:** Implement "Lab Guide" documentation for the one-click stand-up.
5.  **Task 16.5:** Verify teardown reliability (ensuring no orphaned volumes/networks).
