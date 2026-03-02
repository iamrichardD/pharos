# Pharos Verification Protocol

Pharos utilizes a **Zero-Host Execution** strategy. All verification MUST occur inside the Podman test environment to ensure parity and prevent host pollution.

## The Pre-Flight Check

Before any commit or push, you MUST run the centralized verification script. This script builds the entire ecosystem and runs automated E2E tests using Playwright.

### 1. Build the Test Image
If you've modified `Containerfile.test` or it's your first time:
```bash
podman build -t pharos-test -f Containerfile.test .
```

### 2. Run the Pre-Flight Script
This executes the Tier 1, 2, and 3 verification steps:
```bash
podman run --rm -it 
    -v $(pwd):/workspace:Z 
    pharos-test 
    /workspace/scripts/pre-flight.sh
```

## Verification Tiers

### Tier 1: Static Analysis
*   **Rust**: `cargo check` and `cargo fmt`.
*   **Web**: `npm run check` (Astro check) and `tsc` for type safety.

### Tier 2: Unit & Integration Tests
*   **Rust**: `cargo test` for protocol, storage, and middleware logic.
*   **Web**: `npm run test` (Vitest) for component and logic isolation.

### Tier 3: End-to-End (E2E) Verification
*   **Web Console**: Playwright tests (`npm run test:e2e`) verify the integrated user experience (Login, Search, Monitor) inside a headless Chromium browser.
*   **Network Protocol**: Automated `nc` (netcat) probes verify RFC 2378 adherence.

## Manual Debugging
If a test fails, you can enter the debug environment to investigate:
```bash
podman run --rm -it -v $(pwd):/workspace:Z pharos-test bash
```
