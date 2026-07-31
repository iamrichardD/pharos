---
name: skill-pharos-developer
description: Enforce Test-Driven Development (TDD) and containerized execution for all code changes. Use this skill during the implementation phase to ensure that all new code is test-covered and isolated from the host system.
---

# Pharos Developer Protocol (The Implementer)

This skill enforces the core engineering standards for feature implementation and bug fixing.
## 1. Test-Driven Development (TDD) Mandate
The agent MUST follow the Red-Green-Refactor cycle:
- **Single-Task Focus**: Work on exactly ONE feature or bug fix at a time. Do not deviate into unrelated refactoring.
- **Read-Before-Write**: Always read existing code, documentation, and tests first to prevent logic duplication.
1.  **RED**: Write a failing test that clearly defines the desired behavior or reproduces the bug.
    - **Test Rationale**: Every test function MUST be preceded by a comment explaining the importance of the test (the "Why") and what specific regression it prevents.
2.  **GREEN**: Write the simplest possible code to make the test pass.
3.  **REFACTOR**: Clean up the code while ensuring all tests continue to pass.
    - **In-Code Rationale**: Document the purpose (the "Why") of new classes, structs, methods, or complex functions. Focus on the architectural intent.

## 2. Zero-Host Execution Mandate
- **Shift-Left Security**: Identify potential attack vectors (input validation, access control) during the research phase before writing code.
- **Mandate**: All code execution, environment introspection, and testing MUST occur inside a Podman container.
- **Action**: Every command for building, testing, or running the application MUST be wrapped in `podman run` or `podman exec`.
- **Constraint**: The `run_shell_command` tool is forbidden for any non-`podman` command related to code execution.

### TDD Workflow Example
...
- **Step 1 (RED)**: Add `test_should_reject_invalid_token_when_authenticating` to `auth.rs`. 
    - *Rationale*: This test ensures that malformed or expired JWTs cannot bypass the security layer, preventing unauthorized access to the storage engine.
    - Run `podman... cargo test` and verify it fails.
- **Step 2 (GREEN)**: Implement the token validation logic in `auth.rs`. 
    - *In-Code Rationale*: Document `TokenValidator` purpose: "Handles signature verification and TTL checks to ensure only provenance-verified requests reach the business logic."
    - Run `podman... cargo test` and verify it passes.
- **Step 3 (REFACTOR)**: Refine the code, add comments, and ensure `podman... cargo test` still passes.
