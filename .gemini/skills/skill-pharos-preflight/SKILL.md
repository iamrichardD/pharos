---
name: skill-pharos-preflight
description: Enforce the "Pre-Flight Mandate" by running all CI-critical tests (Rust, Astro, Playwright E2E) inside Podman before proposing a commit. Use this skill after completing any code change to ensure it is regression-free.
---

# Pharos Pre-Flight Protocol (The Gatekeeper)

This skill enforces the structural integrity and behavioral correctness of the project before any changes are committed.

## The Pre-Flight Mandate
Before proposing a `git commit` message:
- **Mandate**: The centralized `scripts/pre-flight.sh` script MUST pass successfully inside the `Containerfile.test` environment.
- **Action**: Run the pre-flight check using the Podman-wrapped command.
- **Constraint**: The `git commit` tool is forbidden if this check fails. The agent must analyze the failure, fix the issue, and re-run the check until it passes.

### Pre-Flight Command

```bash
podman run --rm --security-opt seccomp=unconfined -v .:/app:z --workdir /app --env-file .env.example -it pharos-test scripts/pre-flight.sh
```

### Pre-Flight Components
The script validates:
- **Rust Unit Tests**: `cargo test` for all crates.
- **Astro Static Analysis**: `npm run build` in `pharos-console-web`.
- **Headless E2E Verification**: `playwright test` against an ephemeral backend.
