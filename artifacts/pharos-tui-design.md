/* ========================================================================
 * Project: pharos
 * Component: TUI Dashboard
 * File: artifacts/pharos-tui-design.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This document outlines the architectural design and layout strategy
 * for the Pharos TUI (Terminal User Interface) dashboard. This dashboard
 * will provide real-time visibility into server health, metrics, and events.
 * * Traceability:
 * Related to Phase 12 (Issue #44)
 * ======================================================================== */

# Pharos TUI Dashboard: Architectural Design

## 1. Overview
The Pharos TUI Dashboard is a terminal-based monitoring interface designed to give Home Labbers and Enterprise Engineers an immediate, "badass" visual understanding of their `pharos-server` instance. Built with `ratatui` and `crossterm`, it provides real-time insights without requiring external monitoring tools like Prometheus or Grafana.

## 2. Core Architecture

### 2.1. UI Engine (`ratatui`)
- **Backend:** `crossterm` for cross-platform terminal manipulation.
- **Event Loop:** A dedicated thread handling terminal events (keys, resize) and application ticks.
- **State Management:** A thread-safe `AppState` struct shared between the server's core logic and the UI rendering loop.

### 2.2. Integration with `pharos-server`
The TUI will be integrated directly into the `pharos-server` binary.
- **Execution:** When `pharos-server` is started interactively (a TTY is detected) without the `--daemon` or `--no-tui` flags, it will launch the TUI.
- **Concurrency:** The TUI will run on the main thread, while the Tokio async runtime powering the server (TCP listener, API, etc.) will run in the background.

## 3. Visual Layout

The dashboard will be divided into the following layout panels:

### 3.1. Header (Top)
- Displays the Pharos logo/name, current version, server uptime, and overall status (e.g., "ONLINE", "DEGRADED").

### 3.2. Metrics Panel (Left Sidebar)
- **CPU & Memory:** Sparklines or gauges showing current host/container resource usage.
- **Network I/O:** Real-time counters for incoming requests per second and bytes transferred.

### 3.3. Database Stats (Right Sidebar)
- Displays current record counts.
- Split by category: People (`ph`) vs. Machines/Infrastructure (`mdb`).

### 3.4. Event Stream (Main Center)
- A scrolling log of live system events.
- **Inclusions:** Connections established, queries executed, authorization failures, and specifically **discovery events** (from `pharos-scan` or `pharos-pulse`).

### 3.5. Footer (Bottom)
- Quick reference for key bindings (e.g., `q` to quit, `c` to clear logs, `p` to pause updates).

## 4. Implementation Plan

### Phase 12.1: Planning (Completed)
- Define layout, technology stack, and integration strategy.

### Phase 12.2: Core Dashboard Implementation
- Add `ratatui` and `crossterm` dependencies to `pharos-server`.
- Implement the async-compatible event loop and state sharing.
- Build the static layout and populate the Header, Metrics, and Database Stats panels with live data from the existing `metrics.rs` module.

### Phase 12.3: Event Stream Integration
- Implement a broadcast channel for system events.
- Route connection logs and queries to the TUI event stream.
- Integrate discovery/pulse events for a comprehensive operational view.

## 5. Verification Strategy
- **Local:** Run `podman run -it --rm pharos-server:test` and ensure the TUI renders correctly without blocking the TCP listener.
- **Load Testing:** Bombard the server with requests while the TUI is active to ensure the UI thread remains responsive and doesn't panic.
