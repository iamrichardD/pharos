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
echo "--- [1/4] Rust: Building and Running Unit Tests ---"
cargo test --verbose
cargo build --package pharos-server
cargo build --package pharos-pulse
cargo build --package pharos-scan
cargo build --package ph
cargo build --package mdb

# --- 2. Marketing Site Verification ---
echo "--- [2/4] Marketing Site: Build ---"
cd website
npm install
npm run build
cd ..

# --- 3. Web Console Verification (Static) ---
echo "--- [3/4] Web Console: Static Analysis & Build ---"
cd pharos-console-web
npm install
npm run check
npm run build
cd ..

# --- 4. Web Console Verification (E2E) ---
echo "--- [4/4] Web Console: Running Playwright E2E Tests ---"
cd pharos-console-web
# Generate fresh ephemeral certs for E2E backend
rm -rf /tmp/e2e-certs
mkdir -p /tmp/e2e-certs
../scripts/gen-sandbox-certs.sh /tmp/e2e-certs
cat /tmp/e2e-certs/root-ca.crt >> /tmp/e2e-certs/pharos-server.crt
export PHAROS_TLS_CERT=/tmp/e2e-certs/pharos-server.crt
export PHAROS_TLS_KEY=/tmp/e2e-certs/pharos-server.key
export PHAROS_CA_CERT=/tmp/e2e-certs/root-ca.crt
export NODE_EXTRA_CA_CERTS=/tmp/e2e-certs/root-ca.crt

# Playwright config handles startup
npm run test:e2e
cd ..

echo "===================================================="
echo "✅ PRE-FLIGHT SUCCESSFUL: All systems verified."
echo "===================================================="
