# Pharos Project Backlog

## Phase 1: Zero-Host & DevSecOps Foundation
- [ ] **Task 1.1 (Issue #TBD):** Initialize internal project directory structure within the current root (create workspaces for `pharos-server` backend, `ph` CLI, and `mdb` CLI).
- [ ] **Task 1.2 (Issue #TBD):** Create `Containerfile.test` and `Containerfile.debug` for Podman-based Zero-Host execution.
- [ ] **Task 1.3 (Issue #TBD):** Select primary programming language and initialize dependency management inside the `Containerfile.debug` environment.
- [ ] **Task 1.4 (Issue #TBD):** Create `SECURITY.md` detailing DevSecOps practices.
- [ ] **Task 1.5 (Issue #TBD):** Scaffold initial GitHub Actions workflow for cross-compiling target triples (`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`).

## Phase 2: Core Server (`pharos`) MVP
- [ ] **Task 2.1 (Issue #TBD):** Implement core TCP listener and base RFC 2378 (Ph) syntax parser.
- [ ] **Task 2.2 (Issue #TBD):** Implement the in-memory storage engine (Development Tier).
- [ ] **Task 2.3 (Issue #TBD):** Implement the "Discriminator" logic to route requests as either `people` or `machine` records.
- [ ] **Task 2.4 (Issue #TBD):** Implement standard application metrics (Push/Pull) and the "Health Monitor" threshold warnings.

## Phase 3: CLI Clients MVP
- [ ] **Task 3.1 (Issue #TBD):** Implement read-only `ph` CLI client with basic query formatting.
- [ ] **Task 3.2 (Issue #TBD):** Implement read-only `mdb` CLI client with basic query formatting.

## Phase 4: Advanced Storage & Authentication
- [ ] **Task 4.1 (Issue #TBD):** Implement file-level, restart-survivable storage engine (Home Lab Tier).
- [ ] **Task 4.2 (Issue #TBD):** Implement LDAP-backed storage engine and standard schema (Enterprise Tier).
- [ ] **Task 4.3 (Issue #TBD):** Implement SSH-key-based authentication for Write operations on the server.
- [ ] **Task 4.4 (Issue #TBD):** Update `ph` and `mdb` CLIs to support authenticated write/update commands.

## Bug Tracker & Unplanned Work
*(Log any bugs discovered during feature development here. Do not fix them until the current task is complete. Ensure each gets a GitHub issue created via `gh`).*
- [ ] *None currently.*

