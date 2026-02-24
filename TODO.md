# Pharos Project Backlog

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
- [ ] *None currently.*

