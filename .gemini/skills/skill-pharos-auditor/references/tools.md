# Structural Vulnerability Analysis Toolset (Containerized)

## Registry Governance
**STRICT MANDATE**: The use of `docker.io` is FORBIDDEN. All container images MUST be sourced from:
- `ghcr.io` (GitHub Container Registry)
- `public.ecr.aws` (Amazon ECR Public Gallery)

## Static Analysis (SAST)





   dependencies, and configuration for vulnerabilities without execution.

- **Secret Scanning**:
  - `trufflehog`: `ghcr.io/trufflesecurity/trufflehog:latest`
  - `gitleaks`: `ghcr.io/gitleaks/gitleaks:latest`
- **Dependency Auditing (Rust)**:
  - `cargo-audit`: `public.ecr.aws/docker/library/rust:latest` (requires `cargo install cargo-audit`) or specialized images.
- **Insecure Patterns (Rust/JS)**:
  - `semgrep`: `ghcr.io/semgrep/semgrep:latest`
  - `eslint`: `public.ecr.aws/docker/library/node:latest` (with security plugins)

## Dynamic Analysis (DAST)
Analyze running systems through protocol interaction and fuzzing.

- **Protocol Validation (RFC 2378)**:
  - Custom Python/Rust fuzzer: `public.ecr.aws/docker/library/python:3.12-slim` or `public.ecr.aws/docker/library/rust:latest`.
- **Port Scanning / Network Mapping**:
  - `nmap`: `ghcr.io/instrumenta/nmap:latest`
- **Web Console Auditing**:
  - `zap-baseline`: `ghcr.io/zaproxy/zaproxy:stable`
  - `nuclei`: `ghcr.io/projectdiscovery/nuclei:latest`

## Generic Fuzzing & Exploration
- **AFL++**: `ghcr.io/aflplusplus/aflplusplus:latest`
- **LibFuzzer**: Integrated into Rust/LLVM toolchains.

## Usage Pattern
Always use the `--rm` and `-v .:/app:z` flags to ensure a clean state and proper volume mounting within Podman.
