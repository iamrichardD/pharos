/* ========================================================================
 * Project: pharos
 * Component: Network Scanner (pharos-scan)
 * File: artifacts/pharos-scan-design.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This document outlines the architectural design and implementation strategy
 * for 'pharos-scan', a network discovery and provisioning tool. It enables
 * automated inventory of machines and infrastructure for the Pharos ecosystem.
 * * Traceability:
 * Related to Phase 10 (Issue #39, #40)
 * ======================================================================== */

# Pharos Scan: Architectural Design

## 1. Overview
`pharos-scan` is a CLI tool designed to simplify the population of the `pharos` server. It automates the discovery of network assets and provides an interactive workflow for provisioning them into the machine/infrastructure (mdb) database.

## 2. Core Components

### 2.1. Discovery Engine (Discovery Sub-Agent)
- **mDNS/DNS-SD:** Detects devices broadcasting services (SSH, HTTP, etc.) on the local network.
- **ARP/ICMP Scanner:** Performs fast sweeps of the local subnet to identify silent or non-broadcasting hosts.
- **OUI Resolver:** Uses MAC address prefixes to identify hardware manufacturers.
- **Port Fingerprinting:** Probes common infrastructure ports (22, 80, 443, 8006, 32400) to guess device roles.

### 2.2. Interactive CLI (Advocate Sub-Agent UX)
- **TUI Selection:** Uses `inquire` or `ratatui` to allow users to multi-select discovered devices.
- **Pharos Context:** Labels devices as `[EXISTING]` or `[NEW]` based on live queries to the Pharos server.
- **Template Provisioning:** Allows bulk assignment of fields (e.g., `type=server`, `owner=admin`).

### 2.3. Provisioning Layer (Developer Sub-Agent)
- **RFC 2378 Integration:** Communicates with `pharos-server` via the standard protocol.
- **SSH-Auth Support:** Uses the shared authentication logic to perform `add` operations.

## 3. Implementation Plan

### Phase 10.1: Shared Client Library
Refactor `mdb` and `ph` logic into a shared internal crate (`crates/pharos-client`) to provide:
- Async TCP communication.
- RFC 2378 parsing and response handling.
- SSH-key signing and challenge/response authentication.

### Phase 10.2: Scanner Implementation
- Implement the async scanning engine.
- Integrate mDNS and ARP scanning.
- Implement fingerprinting logic.

### Phase 10.3: Provisioning Workflow
- Implement the interactive TUI.
- Connect the scanner results to the Pharos `add` command.

## 4. Verification Strategy
- **Unit Tests:** Mock network interfaces to verify scanning logic.
- **Integration Tests:** Run scanner against a controlled Podman network with simulated services.
- **Manual Verification:** Verify that discovered devices are correctly added to a live `pharos-server` instance.
