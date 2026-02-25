<!--
/* ========================================================================
 * Project: pharos
 * Component: Documentation / Architecture
 * File: pharos-pulse-spec.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Defines the technical specification for the pharos-pulse agent, its 
 * telemetry payload, and system manager integration lifecycle across
 * Linux, macOS, and Windows platforms.
 * * Traceability:
 * Related to "Pulse Agent" Architecture definition.
 * ======================================================================== */
-->

# Pharos Pulse (`pharos-pulse`) Technical Specification

## 1. Overview
The `pharos-pulse` agent is a lightweight, zero-dependency, statically linked Rust binary deployed on nodes managed by the Pharos server. It acts as a "heartbeat" service, providing constant telemetry, state synchronization, and identity assertion to enable autonomous infrastructure management.

## 2. Core Constraints
- **Language**: Rust
- **Linking**: Fully static (musl for Linux).
- **Execution Context**: Runs as a background service/daemon with the lowest privileges necessary to gather metrics.
- **Resource Footprint**: Must consume less than 15MB RAM and minimal CPU.

## 3. Platform Integrations (System Managers)
A unified `pharos-pulse install` command must handle native service registration.

### 3.1 Ubuntu / Linux (Systemd)
- **Path**: `/etc/systemd/system/pharos-pulse.service`
- **Type**: `notify` (using `sd-notify`) for precise lifecycle tracking.
- **Security**: Bound by `ProtectSystem=full`, `PrivateTmp=true`, `CapabilityBoundingSet=`.

### 3.2 macOS (launchd)
- **Path**: `/Library/LaunchDaemons/com.pharos.pulse.plist`
- **Lifecycle**: Managed by `launchctl load/unload`.
- **Behavior**: Persistent across reboots, automatic restart on crash (`KeepAlive`).

### 3.3 Windows (Service Control Manager)
- **Tooling**: Uses the `windows-service` Rust crate.
- **Context**: Runs under `LocalService` account to restrict access to user data while allowing system metrics access.

## 4. Telemetry Payload Schema
Sent via JSON to the Pharos server's "Telemetry Write" endpoint on start/restart, and every 24 hours.

```json
{
  "identity": {
    "hw_uuid": "string",
    "hostname": "string",
    "ssh_pubkey_fingerprint": "string"
  },
  "environment": {
    "platform": "string (e.g., proxmox-lxc, aws-ec2, bare-metal)",
    "os_family": "string",
    "kernel_version": "string",
    "last_update_timestamp": "string (ISO8601)"
  },
  "resources": {
    "mem_total_mb": "integer",
    "mem_used_mb": "integer",
    "cpu_model": "string",
    "cpu_load_avg": "float",
    "disk_utilization_pct": "integer"
  },
  "capabilities": {
    "installed_tools": ["docker", "python3", "zfs", "..."]
  },
  "health": {
    "uptime_seconds": "integer",
    "thermal_status": "string (optional)"
  }
}
```

## 5. Security & Authentication
- **Local Key Pair**: `pharos-pulse` generates or utilizes an existing SSH key pair (typically `/etc/pharos/pulse_id`).
- **Signature**: The telemetry payload is signed with the private key. The Pharos server validates this against the agent's mapped team/identity.
- **Enrollment**: Manual (via placing the public key in authorized keys) or Automatic (via short-lived Provisioning Token sent to the server for initial bootstrapping).
