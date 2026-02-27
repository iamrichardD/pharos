# Pharos Project Backlog

## Definition of Done (DoD)
A task is considered complete and may be marked `[x]` only when:
1. **Local Verification:** All tests/builds pass inside the `Containerfile.test` Podman environment.
2. **Standardization:** All new files include the Standardized File Prologue.
3. **CI Validation:** GitHub Actions (CI) pass successfully on all target platforms (`linux`, `macos`, `windows`).
4. **Production Verification:** The change is verified to be live and functional on the production environment (e.g., `https://iamrichardd.com/pharos/`) using `web_fetch` or manual verification.
5. **AI-Handover:** The GitHub Issue is closed with a structured "Fix Summary" and an "AI-Ready Verification Prompt."
6. **State Sync:** `@PROGRESS.md` and `@TODO.md` are updated.

## Phase 1: Zero-Host & DevSecOps Foundation
- [x] **Task 1.1 (Issue #1):** Initialize internal project directory structure within the current root (create workspaces for `pharos-server` backend, `ph` CLI, and `mdb` CLI).
- [x] **Task 1.2 (Issue #2):** Create `Containerfile.test` and `Containerfile.debug` for Podman-based Zero-Host execution.
- [x] **Task 1.3 (Issue #3):** Select primary programming language and initialize dependency management inside the `Containerfile.debug` environment.
- [x] **Task 1.4 (Issue #4):** Create `SECURITY.md` detailing DevSecOps practices.
- [x] **Task 1.5 (Issue #5):** Scaffold initial GitHub Actions workflow for cross-compiling target triples (`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`).
- [x] **Task 1.6 (Issue #7):** Refine the release process in `GEMINI.md` to mandate Semantic Versioning and annotated Git tags.

## Phase 2: Core Server (`pharos`) MVP
- [x] **Task 2.1 (Issue #6):** Implement core TCP listener and base RFC 2378 (Ph) syntax parser.
- [x] **Task 2.2 (Issue #8):** Implement the in-memory storage engine (Development Tier).
- [x] **Task 2.3 (Issue #9):** Implement the "Discriminator" logic to route requests as either `people` or `machine` records.
- [x] **Task 2.4 (Issue #10):** Implement standard application metrics (Push/Pull) and the "Health Monitor" threshold warnings.

## Phase 3: CLI Clients MVP
- [x] **Task 3.1 (Issue #11):** Implement read-only `ph` CLI client with basic query formatting.
- [x] **Task 3.2 (Issue #12):** Implement read-only `mdb` CLI client with basic query formatting.

## Phase 4: Advanced Storage & Authentication
- [x] **Task 4.1 (Issue #13):** Implement file-level, restart-survivable storage engine (Home Lab Tier).
- [x] **Task 4.2 (Issue #14):** Implement LDAP-backed storage engine and standard schema (Enterprise Tier).
- [x] **Task 4.3 (Issue #15):** Implement SSH-key-based authentication for Write operations on the server.
- [x] **Task 4.4 (Issue #16):** Update `ph` and `mdb` CLIs to support authenticated write/update commands.

## Phase 5: Release & Documentation
- [x] **Task 5.1 (Issue #17):** Finalize AGPL-3.0 License enforcement and headers in CI/CD.
- [x] **Task 5.2 (Issue #18):** Generate high-quality architecture diagrams and "How-To" guides.
- [x] **Task 5.3 (Issue #19):** Prepare v1.0.0 release with annotated Git tags and GitHub Release.

## Bug Tracker & Unplanned Work
*(Log any bugs discovered during feature development here. Do not fix them until the current task is complete. Ensure each gets a GitHub issue created via `gh`).*

**Verification Protocol (Strict Requirement):**
- **Issue Creation:** Include `**Failure Specifics**`, `**Proposed Fix**`, and `**Verification Strategy**`.
- **Issue Closure:** Include an **AI-Ready Verification Prompt** (Podman command) and a human-readable **Fix Summary**.

- [x] **Task 6.1 (Issue #20):** CI: Verify standardized headers fails on Windows runner (bash syntax in pwsh).
- [x] **Task 6.2 (Issue #21):** Build: `pharos-server` compilation failure due to dependencies and type inference.
- [x] **Task 6.6 (Issue #32):** Deploy: GitHub Actions website deployment fails due to environment branch policy.
- [x] **Task 6.7 (Issue #36):** Bug: Mermaid diagrams not rendering on Architecture documentation page.

## Phase 6: Marketing & Open Source Advocacy
- [x] **Task 6.3 (Issue #24):** Advocacy: Scaffold GitHub Pages site with Sierra-inspired UX (iamrichardd.com/pharos).
- [x] **Task 6.4 (Issue #25):** Advocacy: Create high-quality architecture diagrams and "How-To" guides for Home Labbers and Enterprise Engineers.
- [x] **Task 6.5 (Issue #26):** Advocacy: Integrate DORA Metrics & Project Velocity Stats into the marketing site.

## Phase 7: Personal Website Integration (iamrichardd.com)
- [x] **Task 7.1 (Issue #27):** Advocacy: Integrate Pharos Landing Page into iamrichardd.com using existing Astro design system.
- [x] **Task 7.2 (Issue #28):** Advocacy: Port Pharos technical documentation to Astro MDX with Shiki highlighting.
- [x] **Task 7.3 (Issue #29):** Advocacy: Implement visual DORA Metrics & Project Velocity dashboard on the marketing site.
- [x] **Task 7.4 (Issue #30):** Advocacy: Align Pharos marketing site UX/UI with iamrichardd.com personal branding.

## Phase 8: Community & Ecosystem Expansion
- [x] **Task 8.1 (Issue #31):** Advocacy: Create a comprehensive `CONTRIBUTING.md` guide and project governance model.
- [x] **Task 8.2 (Issue #33):** Engineering: Implement a middleware/hook system in `pharos-server` for custom request processing.
- [x] **Task 8.3 (Issue #34):** Advocacy: Expand the marketing site with a community "Showcase" and integration guide.
- [x] **Task 8.4 (Issue #35):** Release: Prepare v1.1.0 release with the new hook system and community features.

## Phase 9: Documentation & UX Refinement
- [x] **Task 9.1 (Issue #37):** Advocacy: Refine documentation headers to remove "Pharos" prefix across all MDX pages.
- [x] **Task 9.2 (Issue #38):** Advocacy: Implement a tiered tabbed interface (Home Lab vs. Enterprise) in `howto.mdx` to reduce cognitive load.

## Phase 10: Network Discovery & Automation
- [x] **Task 10.1 (Issue #39):** Engineering: Refactor client logic into a shared `pharos-client` library with async support and SSH-auth.
- [x] **Task 10.2 (Issue #40):** Engineering: Implement the `pharos-scan` engine (mDNS, ARP, and Port Fingerprinting).
- [x] **Task 10.3 (Issue #41):** Engineering: Implement the interactive TUI and provisioning workflow for `pharos-scan`.
- [x] **Task 10.4 (Issue #42):** Release: Prepare v1.2.0 release including the new network scanner tool.

## Phase 11: Marketing & Advocacy (v1.2.0)
- [x] **Task 11.1 (Issue #43):** Advocacy: Update marketing site with user-centric success messaging and the pharos-scan automated discovery feature.

## Phase 12: Advanced Visualization & Management (TUI)
- [x] **Task 12.1 (Issue #44):** Planning: Design a real-time TUI dashboard for \`pharos-server\` using \`ratatui\`.
- [x] **Task 12.2 (Issue #45):** Engineering: Implement the core dashboard logic and event stream.
- [x] **Task 12.3 (Issue #46):** Engineering: Integrate live \`pharos-scan\` events into the dashboard view.

## Phase 13: Future Expansion

## Phase 14: Pulse & Multi-Tenant Architecture
- [x] **Task 14.1 (Issue #47):** Engineering: Implement `pharos-pulse` heartbeat agent in Rust for cross-platform system manager integration (Systemd, SCM, launchd).
- [x] **Task 14.2 (Issue #49):** Engineering: Extend `pharos-server` security model to support Triple-Tier Security (open, protected, scoped) and Provenance Metadata.
- [x] **Task 14.3 (Issue #51):** Engineering: Develop the `pharos-console` (Dynamic Dashboard & WebMCP/MCP Server) to allow AI-agent management of SSH keys and enrollment tokens. Note: This is separate from the marketing site.
- [x] **Task 14.4 (Issue #53):** Advocacy: Create multi-tenant documentation, "The YOLO Lab" vs "Locking down the Lab" guides, and `pharos-pulse` installation guides. Include docs on the separation of Marketing vs. Console sites.
- [x] **Task 14.5 (Issue #55):** Advocacy: Apply tiered tab UX (Home Lab vs. Enterprise) to `integrations.mdx`, `architecture.mdx`, and `showcase.mdx` to reduce cognitive load across the documentation suite.
- [x] **Task 14.6 (Issue #56):** Advocacy: Perform Cognitive UX Audit, generate high-quality CLI animations, and implement dynamic "Vision vs Quick Start" landing page.
- [x] **Task 14.7 (Issue #57):** Advocacy: Rearrange documentation information architecture to be tool-centric (Clients -> Console -> Automation -> Server Setup -> Scan).

## Phase 15: Global Resilience & Advanced Connectivity
- [ ] **Task 15.1 (Issue #59):** Engineering: Implement Multi-Server Synchronization for high-availability (HA) clusters using background replication.
- [ ] **Task 15.2 (Issue #60):** Engineering: Implement Webhook Notification Engine for real-time Slack, Discord, and Custom API alerts on record modifications.
- [ ] **Task 15.3 (Issue #61):** Engineering: Implement Advanced Pulse Alerting with configurable "Dead Man's Switch" logic for node failures.
