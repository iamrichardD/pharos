/* ========================================================================
 * Project: pharos
 * Component: Documentation & UX
 * File: pharos-installation-ux-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This document outlines the strategy for creating a high-signal, 
 * frictionless installation experience for the Pharos ecosystem, 
 * inspired by industry standards like Pi-hole.
 * * Traceability:
 * Related to Phase 21 (Installation UX Refinement), inspired by Pi-hole.
 * ======================================================================== */

# Pharos Installation UX & Toolbelt Plan

## 1. Vision
Transform the Pharos installation process from a series of manual steps into a "60-second success" workflow. The goal is to empower Home Labbers and Enterprise Engineers with an automated, reliable, and visually guided installation path.

## 2. Key Components

### 2.1. The "One-Liner" Automated Installer
Implement a unified shell script that detects the environment (OS, Architecture) and installs the requested component.
- **Command:** `curl -sSL https://install.pharos.sh | bash -s -- [server|toolbelt|pulse]`
- **Features:**
    - OS Detection (Ubuntu, Debian, macOS, Windows/WSL).
    - Architecture Detection (x86_64, aarch64).
    - Automatic binary download from GitHub Releases.
    - Systemd service configuration (for Server and Pulse).
    - Path injection for CLI toolbelt.

### 2.2. Unified Installation Documentation (`docs/INSTALL.md` & `/docs/install`)
Consolidate installation information into a single, high-priority page on the marketing site.
- **Structure:**
    - **Quick Start:** The One-Liner.
    - **Step-by-Step:** Manual installation for air-gapped or restricted environments.
    - **Toolbelt Only:** Lightweight workstation setup for `ph` and `mdb`.
    - **Pulse Agent:** Mass-deployment guide for managing hundreds of nodes.
- **UX Elements:**
    - Tiered Tabs (Home Lab vs. Enterprise).
    - "Copy to Clipboard" buttons for all commands.
    - Visual "Post-Install Verification" checklist.

### 2.3. Web Console Integration
The Web Console should serve as a living guide for expansion.
- **"Add Node" Workflow:** A modal or page that provides the exact `podman` or `curl` command to install `pharos-pulse` on a new machine.
- **"Download Toolbelt" Section:** Quick links to binaries for all platforms.

### 2.4. Contextual Guidance & CLI Bridging
Bridge the gap between the Web Console (GUI) and the specialized CLI toolbelt.
- **Search Examples**: Add clickable search examples (e.g., `hostname=pharos-*`, `status=up`, `return all`) below the MDB search bar.
- **CLI References**: Provide direct links to the marketing site's CLI documentation on every relevant page (e.g., Search, Node Details).
- **Toolbelt Hints**: When a user performs an action in the Web Console, provide the equivalent CLI command as a "Power User Hint."

## 3. Implementation Phases

### Phase 1: Planning & Tracking (Current)
- [ ] Create `artifacts/pharos-installation-ux-plan.md`.
- [ ] Open GitHub Issue for "Installation UX Refinement".
- [ ] Update `TODO.md` and `PROGRESS.md`.

### Phase 2: The Automated Installer
- [ ] Develop `scripts/install.sh` with environment detection.
- [ ] Implement component-specific logic (Server vs. Toolbelt vs. Pulse).
- [ ] Test in Podman `Containerfile.test` across multiple base images.

### Phase 3: Documentation Revamp
- [ ] Create `website/src/content/docs/install.mdx`.
- [ ] Update `server-setup.mdx` and `cli-clients.mdx` to link to the unified installer.
- [ ] Add "Installation" to the primary navigation on the marketing site.

### Phase 4: Web Console Enhancements
- [ ] Implement the "Add Node" UI component in the Web Console.
- [ ] Add a "Quick Setup" card to the Web Console dashboard for new users.

## 4. Inspiration: The Pi-hole Standard
Pi-hole succeeded by making a complex network-level installation feel trivial. Pharos will adopt:
- **Zero-Friction Entry:** The one-liner is the primary recommendation.
- **Clear Prerequisites:** Explicitly state supported OS versions (Ubuntu LTS, etc.).
- **Immediate Feedback:** The installer should provide a "Success" summary with the server IP and port.

## 5. Verification Strategy
- **Automated:** CI/CD tests for the installer script in various container environments.
- **Manual:** Live verification of the one-liner on a clean Ubuntu 24.04 VM.
- **Documentation:** Review by a "Home Labber" persona for clarity and cognitive load.
