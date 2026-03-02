#!/bin/bash
# ========================================================================
# Project: pharos
# Component: DevSecOps Automation
# File: scripts/pre-flight.sh
# Author: Richard D. (https://github.com/iamrichardd)
# License: AGPL-3.0 (See LICENSE file for details)
# * Purpose (The "Why"):
# Centralized verification script to be run inside the Podman test container.
# Ensures Rust builds, Astro builds, and Headless E2E tests pass before commit.
# * Traceability:
# Related to Phase 17 Sandbox verification.
# ========================================================================

set -e

# --- 1. Rust Verification ---
echo "--- [1/3] Rust: Building and Running Unit Tests ---"
cargo test --verbose

# --- 2. Web Console Verification (Static) ---
echo "--- [2/3] Web Console: Static Analysis & Build ---"
cd pharos-console-web
npm install
npm run check
npm run build

# --- 3. Web Console Verification (E2E) ---
echo "--- [3/3] Web Console: Running Playwright E2E Tests ---"
# Playwright config handles startup
npm run test:e2e

echo "===================================================="
echo "✅ PRE-FLIGHT SUCCESSFUL: All systems verified."
echo "===================================================="
