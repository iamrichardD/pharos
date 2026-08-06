#!/bin/bash
# ========================================================================
# Project: pharos
# Component: Live Verification Harness
# File: scripts/live-verify/setup.sh
# Author: Richard D. (https://github.com/iamrichardd)
# License: AGPL-3.0 (See LICENSE file for details)
# * Purpose (The "Why"):
# Prepares a fresh disposable workspace (certs, admin key, empty data dir)
# for scripts/live-verify/docker-compose.yml. DEV/TEST ONLY - throwaway
# certs and keys, generated fresh every run, never reused across runs and
# never meant to be committed. See README.md in this directory.
# ========================================================================

set -euo pipefail
cd "$(dirname "$0")"

WORKSPACE="./workspace"
rm -rf "${WORKSPACE}"
mkdir -p "${WORKSPACE}/certs" "${WORKSPACE}/keys" "${WORKSPACE}/data" "${WORKSPACE}/bin"

# Reuse the repo's existing CA-signed cert generator rather than hand-rolling
# a self-signed one (a self-signed leaf trips "CA used as end entity" against
# a strict TLS stack) - CN/SAN "pharos-server" matches the compose service
# name so mdb/pharos-pulse can reach it by that name over the compose network.
../gen-sandbox-certs.sh "${WORKSPACE}/certs" >/dev/null
mv "${WORKSPACE}/certs/pharos-server.crt" "${WORKSPACE}/certs/server.crt"
mv "${WORKSPACE}/certs/pharos-server.key" "${WORKSPACE}/certs/server.key"

# A single pre-enrolled admin identity, used by every service in the compose
# file (pharos-pulse's writes, mdb/ph's reads and writes) - avoids the
# "which key actually got auto-generated inside the server container" chase
# that ate a lot of time in manual ad-hoc verification before this harness
# existed. Enrolling it as the server's own admin key at startup (Open tier
# would otherwise self-generate one and this pre-enrolled key would go
# unrecognized) is handled by docker-compose.yml copying this .pub into the
# server's keys dir before pharos-server starts.
ssh-keygen -t ed25519 -N "" -f "${WORKSPACE}/keys/admin_id_ed25519" -C "live-verify-admin" >/dev/null

echo "Workspace ready at scripts/live-verify/workspace/"
echo "Next: copy built release binaries (pharos-server, pharos-pulse, mdb, ph as needed) into"
echo "  scripts/live-verify/workspace/bin/  before running 'podman compose up -d'."
