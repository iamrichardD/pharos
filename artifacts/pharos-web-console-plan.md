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
 * the mdb CLI, serving non-technical users, mobile home-labbers, and AI agents.
 * * Traceability:
 * Related to Phase 14 (Pulse & Multi-Tenant Architecture) and Phase 16.
 * ======================================================================== */
-->

# Pharos Web Console: Human/AI/Mobile Interface Plan

## 1. Problem Statement
The Pharos ecosystem requires a unified, dynamic web interface for:
- **Home Labbers**: Mobile/Tablet access within the home network (IP search, IoT management).
- **Enterprise Staff**: Non-technical users performing one-off searches/additions.
- **AI Agents**: Native WebMCP capabilities for automated management.

The Web Console must be **Self-Hosted** (deployed alongside `pharos-server`) and strictly separate from the **Static Marketing Site**.

## 2. Technical Stack: Astro (SSR)
Astro (SSR Mode with Node.js adapter) is selected over Next.js for:
- **Security**: Explicit boundaries between server and client; prevents "accidental hydration" of sensitive data.
- **Performance**: "Islands Architecture" ensures mobile devices only load JS for interactive components (Search/Dashboards).
- **Consistency**: Shares component logic, Tailwind configs, and design tokens with the existing documentation site.
- **Portability**: Lightweight Node.js container ideal for Proxmox/LXC environments.

## 3. Dual-Posture Security Model

### A. Home Lab Posture (Mobile-First)
- **Auth Handshake**: 
    - **Desktop**: "CLI-to-Web" Handshake. User signs a challenge in the terminal (`ph auth sign`) to authorize the browser.
    - **Mobile/Tablet**: "QR Code Handshake". Desktop console generates a secure QR code; mobile device scans it to establish a session.
    - **WebAuthn**: Native Passkey (FaceID/TouchID) support for frictionless mobile access.
- **Trust Model**: Optimized for speed and responsiveness on low-power mobile hardware.

### B. Enterprise Intranet Posture (Hardened)
- **Identity**: OIDC / LDAP integration via Astro Middleware.
- **Hardening**: Mandatory TLS, CSRF protection (Astro Actions), and strict HSTS headers.
- **Auditability**: Every write operation (MDB/PH) includes `Original-Signer-Identity` provenance metadata.

## 4. Product Roadmap (Prioritized)

### Phase 1: Interactive MDB (Highest Priority)
- **Feature**: Mobile-responsive search interface for machine records.
- **Feature**: "One-off Addition" forms using **Astro Actions** (Type-safe, POST-only).
- **Target**: The "Home Labber" on a tablet in the server closet or the "Office Manager" adding a laptop.

### Phase 2: Pulse Monitoring & Identity
- **Feature**: Real-time "ONLINE/OFFLINE" dashboard for all nodes.
- **Feature**: Handshake UI for Desktop-to-Mobile session transfer.

### Phase 3: Network Discovery & Automation
- **Feature**: Trigger `pharos-scan` jobs and bulk-provision discovered devices.
- **AI Enablement**: Native WebMCP tool definitions (Search/Update/Provision).

## 5. Architectural Principles
- **Clean Architecture**: The Console is a *client* of the `pharos-server` API.
- **Mobile-First UI**: Tailwind-based responsive design optimized for touch targets and high-latency mobile networks.
- **Zero-Trust Ready**: Middleware-driven auth logic that scales from LAN-only to Zero-Trust Intranets.
