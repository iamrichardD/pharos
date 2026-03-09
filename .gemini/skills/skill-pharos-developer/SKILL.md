---
name: skill-pharos-developer
description: Enforce Test-Driven Development (TDD) and containerized execution for all code changes. Use this skill during the implementation phase to ensure that all new code is test-covered and isolated from the host system.
---

# Pharos Developer Protocol (The Implementer)

This skill enforces the core engineering standards for feature implementation and bug fixing.

## 1. Test-Driven Development (TDD) Mandate
The agent MUST follow the Red-Green-Refactor cycle:
1.  **RED**: Write a failing test that clearly defines the desired behavior or reproduces the bug.
2.  **GREEN**: Write the simplest possible code to make the test pass.
3.  **REFACTOR**: Clean up the code while ensuring all tests continue to pass.

## 2. Zero-Host Execution Mandate
- **Mandate**: All code execution, environment introspection, and testing MUST occur inside a Podman container.
- **Action**: Every command for building, testing, or running the application MUST be wrapped in `podman run` or `podman exec`.
- **Constraint**: The `run_shell_command` tool is forbidden for any non-`podman` command related to code execution.

### TDD Workflow Example
- **Step 1 (RED)**: Add `test_should_reject_invalid_token_when_authenticating` to `auth.rs`. Run `podman... cargo test` and verify it fails.
- **Step 2 (GREEN)**: Implement the token validation logic in `auth.rs`. Run `podman... cargo test` and verify it passes.
- **Step 3 (REFACTOR)**: Refine the code, add comments, and ensure `podman... cargo test` still passes.
