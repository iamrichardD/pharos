---
name: skill-pharos-auditor
description: Automated SAST/DAST security auditing and vulnerability reproduction within Podman. Use this skill when Gemini CLI needs to perform a security audit, reproduce a reported vulnerability, or validate structural integrity against malicious inputs.
---

# Skill Pharos Auditor

## Overview
This skill empowers Gemini CLI to act as a **Security Gatekeeper** within the Pharos project. It enforces the **Zero-Host Execution Mandate** by performing all security analysis inside Podman containers and follows a **Break-to-Fix TDD** workflow to ensure every discovered flaw is mitigated and verified with code.

## Hybrid Audit Mandate

### 1. Static Structural Analysis (SAST)
Analyze the codebase for vulnerabilities without executing the software.
- **Goal**: Identify leaked secrets, insecure dependencies, and vulnerable code patterns.
- **Clean-State Scanning Mandate**: To avoid finding fatigue, you MUST filter audit results to exclude files listed in `.gitignore` or `git ls-files --others`. Use the `--git` flag in supported tools or cross-reference findings with the Git index. An audit should only be "High-Signal."
- **Workflow**: 
  1. Select a tool from `references/tools.md#static-analysis-sast`.
  2. Execute the tool via Podman: `podman run --rm -v .:/app:z [IMAGE] [COMMAND]`.
  3. Interpret the output and identify "Structural Vulnerabilities."

### 2. Dynamic Structural Analysis (DAST)
Analyze the running system by interacting with it under test conditions.
- **Goal**: Identify protocol-level flaws (RFC 2378), network vulnerabilities, and Web Console regressions.
- **Workflow**:
  1. Spin up the target service (e.g., `pharos-server`) in a Podman container.
  2. Select an auditing/fuzzing tool from `references/tools.md#dynamic-analysis-dast`.
  3. Run the auditor in a separate Podman container on the same network:
     `podman run --rm --network container:[TARGET_CONTAINER] -v .:/app:z [IMAGE] [COMMAND]`.

## The "Break-to-Fix" TDD Workflow
When a vulnerability is identified, do not fix it immediately. Follow these steps:

1. **Reproduction (RED)**: Write a failing (RED) test case that reproduces the vulnerability (e.g., a fuzzer script that crashes the server or a unit test that fails for a specific input).
2. **Mitigation (GREEN)**: Implement the minimal code change to fix the vulnerability and pass the test.
3. **Validation (REFACTOR)**: Ensure the fix doesn't break existing functionality and adheres to Clean Architecture principles.

## Tool Selection Philosophy
Avoid hardcoding specific tools. Instead, evaluate the task and select the best open-source, container-based tool. 
- Refer to [references/tools.md](references/tools.md) for a list of pre-vetted images and usage patterns.
- **Registry Governance**: You MUST NOT pull images from `docker.io`. All images MUST be from `ghcr.io` or `public.ecr.aws`.
- Prefer lightweight, specialized images over large generic ones.

## Security Constraints
- **Zero-Host**: NO code execution or auditing tools on the host OS.
- **Traceability**: All audit findings MUST be logged in the corresponding GitHub Issue via `skill-pharos-sync`.
- **Non-Literal**: Use professional terminology. Avoid "Hacker" language; prefer "Structural Vulnerability Analysis" and "Exploitation Validation."

## Execution Template
Always use this standardized template for Podman execution:
```bash
podman run --rm \
  --security-opt seccomp=unconfined \
  -v .:/app:z \
  -w /app \
  [SECURITY_TOOL_IMAGE] \
  [COMMAND]
```

## Resources
- [references/tools.md](references/tools.md): Pre-vetted security tool list and usage patterns.
