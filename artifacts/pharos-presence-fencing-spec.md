<!--
/* ========================================================================
 * Project: pharos
 * Component: Documentation / Architecture
 * File: pharos-presence-fencing-spec.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Defines the "Tri-State Presence" model to prevent split-brain automation
 * and distinguish between a graceful shutdown (OFFLINE) and a network
 * partition or power failure (UNREACHABLE).
 * * Traceability:
 * Related to RFC 2378 and pharos-pulse-spec.md.
 * ======================================================================== */
-->

# Pharos Presence & Fencing Specification

## 1. The Tri-State Model
To ensure operational safety in Home Lab and Enterprise environments, Pharos distinguishes between reported state and inferred state.

| State | Source | Definition | Automation Impact |
| :--- | :--- | :--- | :--- |
| **`ONLINE`** | `pharos-pulse` | Node explicitly sent an `ONLINE` or `HEARTBEAT` event. | Safe to route traffic. |
| **`OFFLINE`** | `pharos-pulse` | Node explicitly sent a `SIGTERM` / `OFFLINE` signal. | Safe to decommission/replace. |
| **`UNREACHABLE`**| `pharos-server` | (Inferred) No heartbeat received for > 70 minutes. | **FENCED**: Do not replace; check network/power. |

## 2. Server-Side "Dead Man's Switch" Sweep
The `pharos-server` maintains a background process (the "Presence Monitor") that runs every 10 minutes.

### 2.1 Inference Logic
For every `Machine` record in the `mdb`:
1.  Calculate `Delta = CurrentTime - last_seen`.
2.  If `presence == ONLINE` AND `Delta > 70 minutes`:
    *   Update `presence` to `UNREACHABLE`.
    *   Emit a `SystemAlert` event for the Web Console and Pulse Alerting (Phase 15.3).
3.  If a new `HEARTBEAT` or `ONLINE` event arrives for an `UNREACHABLE` node:
    *   Immediately restore `presence` to `ONLINE`.

## 3. Automation Fencing
Any automation tool (e.g., `pharos-scan`, Ansible plugins) integrating with Pharos MUST treat `UNREACHABLE` as a blocking state. 
- **Replacement Rule**: A node marked `UNREACHABLE` cannot be auto-replaced. It requires a Human Asset Manager to change the administrative `status` (e.g., to `RMA` or `DECOMMISSIONED`) or a verified `OFFLINE` event.
