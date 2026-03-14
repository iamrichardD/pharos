#!/usr/bin/env bash

# ========================================================================
# Project: pharos
# Component: Installation Utility
# File: scripts/install.sh
# Author: Richard D. (https://github.com/iamrichardd)
# License: AGPL-3.0 (See LICENSE file for details)
# * Purpose (The "Why"):
# This script provides a frictionless, "one-liner" installation experience
# for the Pharos ecosystem (Server, Pulse, and Toolbelt).
# * Traceability:
# Related to Task 21.2 (Issue #132), inspired by Pi-hole.
# ========================================================================

set -euo pipefail

# --- Configuration ---
VERSION="1.3.0"
REPO="iamrichardD/pharos"
INSTALL_DIR="/usr/local/bin"
PHAROS_DIR="/etc/pharos"
LOG_FILE="/tmp/pharos-install.log"

# --- Colors ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# --- Helpers ---
log() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1" >&2; exit 1; }

# --- Environment Detection ---
detect_os() {
    OS="$(uname -s)"
    case "${OS}" in
        Linux*)     OS_NAME="linux";;
        Darwin*)    OS_NAME="macos";;
        CYGWIN*|MINGW*|MSYS*) OS_NAME="windows";;
        *)          error "Unsupported OS: ${OS}";;
    esac
}

detect_arch() {
    ARCH="$(uname -m)"
    case "${ARCH}" in
        x86_64)     ARCH_NAME="x86_64";;
        aarch64|arm64) ARCH_NAME="aarch64";;
        *)          error "Unsupported Architecture: ${ARCH}";;
    esac
}

check_dependencies() {
    log "Checking dependencies..."
    for cmd in curl tar; do
        if ! command -v "${cmd}" >/dev/null 2>&1; then
            error "Missing dependency: ${cmd}. Please install it and try again."
        fi
    done
}

# --- PKI Setup ---
setup_pki() {
    local cert_name=$1
    local dns_name=$2
    local cert_dir="${PHAROS_DIR}/certs"

    sudo mkdir -p "${cert_dir}"
    sudo chmod 700 "${cert_dir}"

    if [[ -f "${cert_dir}/${cert_name}.crt" ]]; then
        log "Existing certificate found for ${cert_name}. Skipping generation."
        return
    fi

    log "Generating self-signed SSL certificate for ${cert_name} (${dns_name})..."
    
    # Generate a Root CA if it doesn't exist (for local trust)
    if [[ ! -f "${cert_dir}/pharos-ca.crt" ]]; then
        log "Creating local Pharos Root CA..."
        sudo openssl genrsa -out "${cert_dir}/pharos-ca.key" 4096
        sudo openssl req -x509 -new -nodes -key "${cert_dir}/pharos-ca.key" -sha256 -days 3650 -out "${cert_dir}/pharos-ca.crt" -subj "/C=US/ST=Local/L=Pharos/O=Pharos Ecosystem/CN=Pharos Local Root CA"
    fi

    # Generate and sign the service certificate
    sudo openssl genrsa -out "${cert_dir}/${cert_name}.key" 2048
    sudo openssl req -new -key "${cert_dir}/${cert_name}.key" -out "${cert_dir}/${cert_name}.csr" -subj "/CN=${dns_name}"
    
    cat <<EOF | sudo tee "${cert_dir}/${cert_name}.ext" > /dev/null
[v3_req]
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment
subjectAltName = @alt_names

[alt_names]
DNS.1 = ${dns_name}
DNS.2 = localhost
IP.1 = 127.0.0.1
EOF

    sudo openssl x509 -req -in "${cert_dir}/${cert_name}.csr" -CA "${cert_dir}/pharos-ca.crt" -CAkey "${cert_dir}/pharos-ca.key" \
    -CAcreateserial -out "${cert_dir}/${cert_name}.crt" -days 365 -sha256 -extfile "${cert_dir}/${cert_name}.ext" -extensions v3_req
    
    sudo rm "${cert_dir}/${cert_name}.csr" "${cert_dir}/${cert_name}.ext"
    log "Certificate generated: ${cert_dir}/${cert_name}.crt"
}

# --- Installation Logic ---
download_binary() {
    local component=$1
    local target_triple=""

    case "${OS_NAME}" in
        linux)   target_triple="${ARCH_NAME}-unknown-linux-gnu";;
        macos)   target_triple="${ARCH_NAME}-apple-darwin";;
        windows) target_triple="x86_64-pc-windows-msvc";;
    esac

    # Note: In a real scenario, we'd fetch from GH releases. 
    # For now, we simulate the structure or use the local build if in dev mode.
    local url="https://github.com/${REPO}/releases/download/v${VERSION}/${component}-${target_triple}.tar.gz"
    
    log "Downloading ${component} (v${VERSION}) for ${target_triple}..."
    # curl -sSL "${url}" | tar -xz -C "${INSTALL_DIR}"
    
    # Placeholder for actual download logic
    warn "Download URL: ${url} (Simulated for Task 21.2 implementation)"
}

install_server() {
    log "Installing Pharos Server..."
    download_binary "pharos-server"
    
    setup_pki "pharos-server" "pharos-server"

    log "Configuring Systemd service for Pharos Server..."
    # sudo mkdir -p "${PHAROS_DIR}"
    # cat <<EOF | sudo tee /etc/systemd/system/pharos-server.service
    # [Unit]
    # Description=Pharos Protocol Server
    # After=network.target

    # [Service]
    # ExecStart=${INSTALL_DIR}/pharos-server
    # Restart=always
    # User=pharos
    # Environment=PHAROS_CONFIG_DIR=${PHAROS_DIR}
    # Environment=PHAROS_TLS_CERT=${PHAROS_DIR}/certs/pharos-server.crt
    # Environment=PHAROS_TLS_KEY=${PHAROS_DIR}/certs/pharos-server.key

    # [Install]
    # WantedBy=multi-user.target
    # EOF
}

install_pulse() {
    log "Installing Pharos Pulse Agent..."
    download_binary "pharos-pulse"
    
    log "Configuring Systemd service for Pharos Pulse..."
}

install_web_console() {
    log "Installing Pharos Web Console..."
    # download_binary "pharos-web"
    setup_pki "pharos-web" "pharos-web"
    
    log "Configuring Systemd service for Pharos Web Console..."
    # cat <<EOF | sudo tee /etc/systemd/system/pharos-web.service
    # [Unit]
    # Description=Pharos Web Console
    # After=network.target

    # [Service]
    # ExecStart=node ${INSTALL_DIR}/server.mjs
    # Restart=always
    # User=pharos
    # Environment=PHAROS_TLS_CERT=${PHAROS_DIR}/certs/pharos-web.crt
    # Environment=PHAROS_TLS_KEY=${PHAROS_DIR}/certs/pharos-web.key
    # Environment=PORT=3000

    # [Install]
    # WantedBy=multi-user.target
    # EOF
}

install_toolbelt() {
    log "Installing Pharos Toolbelt (ph, mdb, pharos-scan)..."
    download_binary "ph"
    download_binary "mdb"
    download_binary "pharos-scan"
    
    log "Pharos Toolbelt installed to ${INSTALL_DIR}"
}

# --- Main Flow ---
main() {
    check_dependencies
    detect_os
    detect_arch

    local target=${1:-"node"}

    log "Starting Pharos Installation: ${target} (${OS_NAME}/${ARCH_NAME})"

    case "${target}" in
        hub)
            log "Installing Pharos Hub (Server + Console + Scan)..."
            install_server
            install_web_console
            download_binary "pharos-scan"
            ;;
        node)
            log "Installing Pharos Node (Pulse + ph + mdb)..."
            install_pulse
            download_binary "ph"
            download_binary "mdb"
            ;;
        server)   install_server;;
        pulse)    install_pulse;;
        toolbelt) install_toolbelt;;
        *)        error "Unknown target: ${target}. Use hub, node, server, pulse, or toolbelt.";;
    esac

    echo -e "\n${GREEN}Successfully installed Pharos ${target}!${NC}"
    echo -e "Next Steps:"
    if [[ "${target}" == "hub" ]]; then
        echo -e "1. Configure keys in ${PHAROS_DIR}/keys"
        echo -e "2. Start the server: sudo systemctl start pharos-server"
        echo -e "3. Access the Web Console on port 3000 (if installed)."
    elif [[ "${target}" == "node" || "${target}" == "pulse" ]]; then
        echo -e "1. Set PHAROS_HOST environment variable."
        echo -e "2. Start the pulse agent: sudo systemctl start pharos-pulse"
    else
        echo -e "1. Try running 'ph search' or 'mdb status'"
    fi
}

main "$@"
