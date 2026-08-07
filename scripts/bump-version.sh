#!/usr/bin/env bash
# ========================================================================
# Project: pharos
# Component: Release Tooling
# File: scripts/bump-version.sh
# Author: Richard D. (https://github.com/iamrichardd)
# License: AGPL-3.0 (See LICENSE file for details)
# * Purpose (The "Why"):
# Updates every version-bearing file in this repo together, so cutting a
# release never again means remembering to update N different places by
# hand - crates/pharos-pulse/Cargo.toml drifted stale for 3 releases in a
# row despite this exact risk being documented after the first time it
# happened.
# ========================================================================

set -euo pipefail

NEW_VERSION="${1:?Usage: scripts/bump-version.sh X.Y.Z}"
if [[ ! "${NEW_VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Invalid version: '${NEW_VERSION}' (expected X.Y.Z)" >&2
    exit 1
fi

sed -i "s/^VERSION=\".*\"/VERSION=\"${NEW_VERSION}\"/" scripts/install.sh
grep -q "^VERSION=\"${NEW_VERSION}\"\$" scripts/install.sh || {
    echo "Failed to update scripts/install.sh — expected line pattern not found." >&2
    exit 1
}

sed -i "s/^version = \".*\"/version = \"${NEW_VERSION}\"/" crates/pharos-pulse/Cargo.toml
grep -q "^version = \"${NEW_VERSION}\"\$" crates/pharos-pulse/Cargo.toml || {
    echo "Failed to update crates/pharos-pulse/Cargo.toml — expected line pattern not found." >&2
    exit 1
}

# mdb/ph report their own version via clap's #[command(version)] (Debt #35/Issue #182),
# sourced from CARGO_PKG_VERSION - each crate's own Cargo.toml, not scripts/install.sh's
# VERSION= - so they need the same treatment or `--version` silently reports a stale 0.1.0
# forever, exactly like crates/pharos-pulse/Cargo.toml itself used to drift before this
# script existed.
sed -i "s/^version = \".*\"/version = \"${NEW_VERSION}\"/" mdb/Cargo.toml
grep -q "^version = \"${NEW_VERSION}\"\$" mdb/Cargo.toml || {
    echo "Failed to update mdb/Cargo.toml — expected line pattern not found." >&2
    exit 1
}

sed -i "s/^version = \".*\"/version = \"${NEW_VERSION}\"/" ph/Cargo.toml
grep -q "^version = \"${NEW_VERSION}\"\$" ph/Cargo.toml || {
    echo "Failed to update ph/Cargo.toml — expected line pattern not found." >&2
    exit 1
}

echo "Bumped to ${NEW_VERSION} in:"
echo "  scripts/install.sh"
echo "  crates/pharos-pulse/Cargo.toml"
echo "  mdb/Cargo.toml"
echo "  ph/Cargo.toml"
echo ""
echo "Next: rebuild (cargo build -p pharos-server -p pharos-pulse, in Podman per this repo's"
echo "Zero-Host policy) to refresh Cargo.lock before committing."
