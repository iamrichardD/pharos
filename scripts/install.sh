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
VERSION="1.10.6"
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

activate_systemd_service() {
    local service_name=$1
    ${SUDO} systemctl daemon-reload
    ${SUDO} systemctl enable "${service_name}"
    ${SUDO} systemctl restart "${service_name}"
}

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
    for cmd in curl; do
        if ! command -v "${cmd}" >/dev/null 2>&1; then
            error "Missing dependency: ${cmd}. Please install it and try again."
        fi
    done

    if [[ "${EUID}" -eq 0 ]]; then
        SUDO=""
    elif command -v sudo >/dev/null 2>&1; then
        SUDO="sudo"
    else
        error "This installer needs root privileges. Re-run as root, or install 'sudo' and try again."
    fi
}

# --- System User Setup ---
ensure_system_user() {
    [[ "${OS_NAME}" == "linux" ]] || error "This installation target requires a systemd-based Linux host (detected: ${OS_NAME})."
    [[ -d /run/systemd/system ]] || error "This installation target requires systemd as the running init system (not detected). Common on minimal containers or WSL without systemd enabled."
    command -v useradd >/dev/null 2>&1 || error "Missing dependency: useradd. Install your distribution's shadow-utils/passwd package and try again."

    if ! id -u pharos &>/dev/null; then
        log "Creating dedicated 'pharos' system user..."
        local nologin_shell="" candidate
        for candidate in /usr/sbin/nologin /sbin/nologin "$(command -v nologin 2>/dev/null)"; do
            if [[ -n "${candidate}" && -x "${candidate}" ]]; then
                nologin_shell="${candidate}"
                break
            fi
        done
        [[ -n "${nologin_shell}" ]] || error "Could not locate a 'nologin' shell binary on this system."
        ${SUDO} useradd --system --no-create-home --shell "${nologin_shell}" pharos
    fi
}

# --- Dependency Auto-Install ---
ensure_openssl() {
    command -v openssl >/dev/null 2>&1 && return

    warn "openssl not found — attempting to install it automatically..."
    if command -v apt-get >/dev/null 2>&1; then
        ${SUDO} apt-get update -qq || true
        ${SUDO} apt-get install -y openssl || true
    elif command -v dnf >/dev/null 2>&1; then
        ${SUDO} dnf install -y openssl || true
    elif command -v yum >/dev/null 2>&1; then
        ${SUDO} yum install -y openssl || true
    else
        error "Missing dependency: openssl, and no supported package manager (apt/dnf/yum/pacman/apk) was found to install it automatically. Please install openssl manually (e.g. 'pacman -S openssl' or 'apk add openssl') and try again."
    fi

    command -v openssl >/dev/null 2>&1 || error "Failed to install openssl automatically. Please install it manually and try again."
}

# Warns (does not fail) if an existing certificate's SAN no longer covers the host's current LAN
# IP(s) — e.g. after a DHCP renewal or NIC change. Never auto-regenerates; the fix requires an
# operator to delete the stale cert and re-run the installer, since silently rotating a cert an
# operator didn't ask to change could itself be a surprise.
check_cert_ip_drift() {
    local cert_path=$1
    local cert_label=$2
    local current_ips san_ips missing_ips=""

    current_ips="$(hostname -I 2>/dev/null || true)"
    if [[ -z "${current_ips}" ]]; then
        return
    fi

    san_ips="$(${SUDO} openssl x509 -in "${cert_path}" -noout -ext subjectAltName 2>/dev/null \
        | grep -o 'IP Address:[0-9.]*' | cut -d: -f2 || true)"

    local ip
    for ip in ${current_ips}; do
        if [[ "${ip}" == "127.0.0.1" ]]; then
            continue
        fi
        # IPv6 addresses are intentionally out of scope for this drift check: openssl normalizes
        # IPv6 SAN entries to uppercase while `hostname -I` returns lowercase, and the SAN-extraction
        # regex above only captures dotted-decimal IPv4 — naively comparing IPv6 entries produces a
        # false-positive "drift" warning on every host with IPv6 networking (nearly all real hosts)
        # even with zero actual drift. The documented pulse/node-by-IP workflow this check protects
        # is IPv4-only in practice.
        if [[ ! "${ip}" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
            continue
        fi
        if ! grep -qx "${ip}" <<< "${san_ips}"; then
            missing_ips="${missing_ips} ${ip}"
        fi
    done

    if [[ -n "${missing_ips}" ]]; then
        warn "Certificate '${cert_label}' (${cert_path}) does not cover this host's current LAN IP(s):${missing_ips}"
        warn "This can happen after a DHCP lease renewal, subnet change, or NIC reconfiguration since the certificate was generated."
        warn "Remote clients connecting via${missing_ips} will fail TLS hostname verification (\"certificate not valid for name\")."
        warn "To fix: remove ${cert_path} and its .key, then re-run this installer to regenerate the certificate with the current IP(s)."
    fi
}

# --- PKI Setup ---
setup_pki() {
    local cert_name=$1
    local dns_name=$2
    local extra_hostname="${3:-}"
    local cert_dir="${PHAROS_DIR}/certs"

    ensure_openssl

    ${SUDO} mkdir -p "${cert_dir}"
    ${SUDO} chown root:pharos "${cert_dir}"
    ${SUDO} chmod 750 "${cert_dir}"

    if ${SUDO} test -f "${cert_dir}/${cert_name}.crt"; then
        log "Existing certificate found for ${cert_name}. Skipping generation."
        check_cert_ip_drift "${cert_dir}/${cert_name}.crt" "${cert_name}"
        ${SUDO} chown pharos:pharos "${cert_dir}/${cert_name}.key" "${cert_dir}/${cert_name}.crt"
        ${SUDO} chmod 600 "${cert_dir}/${cert_name}.key"
        ${SUDO} chmod 644 "${cert_dir}/${cert_name}.crt"
        return
    fi

    log "Generating self-signed SSL certificate for ${cert_name} (${dns_name})..."
    
    # Generate a Root CA if it doesn't exist (for local trust)
    if ! ${SUDO} test -f "${cert_dir}/pharos-ca.crt"; then
        log "Creating local Pharos Root CA..."
        ${SUDO} openssl genrsa -out "${cert_dir}/pharos-ca.key" 4096
        ${SUDO} openssl req -x509 -new -nodes -key "${cert_dir}/pharos-ca.key" -sha256 -days 3650 -out "${cert_dir}/pharos-ca.crt" -subj "/C=US/ST=Local/L=Pharos/O=Pharos Ecosystem/CN=Pharos Local Root CA"
        ${SUDO} chmod 600 "${cert_dir}/pharos-ca.key"
    fi

    # Generate and sign the service certificate
    ${SUDO} openssl genrsa -out "${cert_dir}/${cert_name}.key" 2048
    ${SUDO} openssl req -new -key "${cert_dir}/${cert_name}.key" -out "${cert_dir}/${cert_name}.csr" -subj "/CN=${dns_name}"
    
    # Include the host's actual LAN IP(s) in the SAN, not just localhost/127.0.0.1 —
    # otherwise a remote pulse node connecting by IP (the documented usage, e.g.
    # `install.sh -- node 192.168.1.5`) passes CA trust but fails hostname
    # verification against a cert that's only ever valid for this machine itself.
    local lan_ips
    lan_ips="$(hostname -I 2>/dev/null || true)"

    {
        echo "[v3_req]"
        echo "authorityKeyIdentifier=keyid,issuer"
        echo "basicConstraints=CA:FALSE"
        echo "keyUsage = digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment"
        echo "subjectAltName = @alt_names"
        echo ""
        echo "[alt_names]"
        echo "DNS.1 = ${dns_name}"
        local dns_index=2
        if [[ -n "${extra_hostname}" ]]; then
            echo "DNS.${dns_index} = ${extra_hostname}"
            dns_index=$((dns_index + 1))
        fi
        echo "DNS.${dns_index} = localhost"
        echo "IP.1 = 127.0.0.1"
        local ip_index=2
        for ip in ${lan_ips}; do
            echo "IP.${ip_index} = ${ip}"
            ip_index=$((ip_index + 1))
        done
    } | ${SUDO} tee "${cert_dir}/${cert_name}.ext" > /dev/null

    ${SUDO} openssl x509 -req -in "${cert_dir}/${cert_name}.csr" -CA "${cert_dir}/pharos-ca.crt" -CAkey "${cert_dir}/pharos-ca.key" \
    -CAcreateserial -out "${cert_dir}/${cert_name}.crt" -days 365 -sha256 -extfile "${cert_dir}/${cert_name}.ext" -extensions v3_req

    ${SUDO} rm "${cert_dir}/${cert_name}.csr" "${cert_dir}/${cert_name}.ext"

    ${SUDO} chown pharos:pharos "${cert_dir}/${cert_name}.key" "${cert_dir}/${cert_name}.crt"
    ${SUDO} chmod 600 "${cert_dir}/${cert_name}.key"
    ${SUDO} chmod 644 "${cert_dir}/${cert_name}.crt"
    log "Certificate generated: ${cert_dir}/${cert_name}.crt"
}

# Best-effort, opt-in CA fetch over SSH for a remote node joining an existing hub. Never
# prompts (curl | bash has no stdin to prompt on) and never weakens SSH's own trust model —
# if the hub's SSH host key isn't already known, or key-based auth isn't already set up, this
# fails fast and the caller falls back to the existing manual-copy instructions.
fetch_ca_via_ssh() {
    local ssh_target=$1
    local cert_dir="${PHAROS_DIR}/certs"

    if ! command -v ssh >/dev/null 2>&1; then
        warn "ssh is not installed on this machine — cannot use --fetch-ca-ssh. Falling back to manual TLS trust instructions below."
        return 1
    fi

    log "Attempting to fetch hub CA cert via SSH from ${ssh_target}..."
    ${SUDO} mkdir -p "${cert_dir}"
    ${SUDO} chown root:pharos "${cert_dir}"
    ${SUDO} chmod 750 "${cert_dir}"

    local fetched
    if ! fetched="$(ssh -o BatchMode=yes -o ConnectTimeout=10 "${ssh_target}" \
        "sudo cat ${cert_dir}/pharos-ca.crt" 2>/dev/null)" || [[ -z "${fetched}" ]]; then
        warn "Could not fetch hub CA cert via SSH from ${ssh_target} (no passwordless SSH/sudo, or the file doesn't exist there) — falling back to manual TLS trust instructions below."
        warn "If you want --fetch-ca-ssh to work, grant narrowly-scoped passwordless sudo on the hub, e.g.: echo '<hub-user> ALL=(ALL) NOPASSWD: /usr/bin/cat ${cert_dir}/pharos-ca.crt' | sudo tee /etc/sudoers.d/pharos-ca-fetch — avoid a blanket 'NOPASSWD: ALL' just for this."
        return 1
    fi

    echo "${fetched}" | ${SUDO} tee "${cert_dir}/pharos-ca.crt" > /dev/null
    ${SUDO} chmod 644 "${cert_dir}/pharos-ca.crt"

    local fingerprint
    fingerprint="$(${SUDO} openssl x509 -in "${cert_dir}/pharos-ca.crt" -noout -fingerprint -sha256 2>/dev/null | cut -d= -f2)"
    if [[ -z "${fingerprint}" ]]; then
        warn "Fetched data from ${ssh_target} was not a valid certificate — removing it and falling back to manual TLS trust instructions below."
        ${SUDO} rm -f "${cert_dir}/pharos-ca.crt"
        return 1
    fi

    log "Fetched hub CA cert via SSH from ${ssh_target} (SHA-256 fingerprint: ${fingerprint})"
    return 0
}

# --- Installation Logic ---
download_binary() {
    local component=$1
    local platform_suffix=""

    case "${OS_NAME}" in
        linux)
            platform_suffix="linux-${ARCH_NAME}"
            ;;
        macos)
            if [[ "${ARCH_NAME}" != "aarch64" ]]; then
                error "Pharos does not publish macOS Intel (x86_64) binaries. Only Apple Silicon (aarch64) Macs are supported — build from source instead: https://github.com/${REPO}"
            fi
            case "${component}" in
                ph|mdb) ;;
                *) error "'${component}' is not distributed for macOS. Pharos only publishes the 'ph' and 'mdb' client tools for macOS (Apple Silicon) — 'pharos-server', 'pharos-scan', and 'pharos-pulse' are Linux-only. Run this on a Linux host, or build from source: https://github.com/${REPO}" ;;
            esac
            platform_suffix="macos-aarch64"
            ;;
        windows)
            platform_suffix="windows-x86_64.exe"
            ;;
    esac

    local url="https://github.com/${REPO}/releases/download/v${VERSION}/${component}-${platform_suffix}"

    local tmp_file
    tmp_file="$(mktemp)"
    trap 'rm -f "${tmp_file}"' EXIT

    log "Downloading ${component} (v${VERSION}) for ${platform_suffix}..."
    if ! curl -fsSL --retry 3 --retry-delay 2 --connect-timeout 10 -o "${tmp_file}" "${url}"; then
        error "Failed to download ${component} from ${url}. Check that release v${VERSION} has an asset for ${platform_suffix}."
    fi

    if [[ ! -s "${tmp_file}" ]]; then
        error "Downloaded file for ${component} is empty: ${url}"
    fi

    chmod +x "${tmp_file}"
    ${SUDO} install -m 755 "${tmp_file}" "${INSTALL_DIR}/${component}"
    rm -f "${tmp_file}"
    trap - EXIT
    log "Installed ${component} to ${INSTALL_DIR}/${component}"
}

install_server() {
    local hostname_arg="${1:-}"
    # Run under LC_ALL=C in a subshell: under a UTF-8 locale, [A-Za-z0-9] becomes
    # locale-collation-aware and can accept non-ASCII characters (e.g. accented
    # letters) that look like they should be rejected — a well-known bash/glibc
    # gotcha. Scoped to a subshell so it doesn't affect the rest of the script.
    if [[ -n "${hostname_arg}" ]] && ! ( LC_ALL=C; [[ "${hostname_arg}" =~ ^[A-Za-z0-9]([A-Za-z0-9-]*[A-Za-z0-9])?(\.[A-Za-z0-9]([A-Za-z0-9-]*[A-Za-z0-9])?)*$ ]] ); then
        error "Invalid hostname for Pharos Server: '${hostname_arg}'"
    fi
    tier="open"
    ensure_system_user
    log "Installing Pharos Server..."
    download_binary "pharos-server"

    setup_pki "pharos-server" "pharos-server" "${hostname_arg}"

    ${SUDO} mkdir -p "${PHAROS_DIR}/keys"
    ${SUDO} chown pharos:pharos "${PHAROS_DIR}/keys"
    ${SUDO} chmod 700 "${PHAROS_DIR}/keys"

    log "Configuring Systemd service for Pharos Server..."
    cat <<EOF | ${SUDO} tee /etc/systemd/system/pharos-server.service > /dev/null
[Unit]
Description=Pharos Protocol Server
After=network.target

[Service]
ExecStart=${INSTALL_DIR}/pharos-server
ExecReload=/bin/kill -HUP \$MAINPID
Restart=always
User=pharos
Group=pharos
Environment=PHAROS_TLS_CERT=${PHAROS_DIR}/certs/pharos-server.crt
Environment=PHAROS_TLS_KEY=${PHAROS_DIR}/certs/pharos-server.key
Environment=PHAROS_STORAGE_PATH=${PHAROS_DIR}/data.json
Environment=PHAROS_KEYS_DIR=${PHAROS_DIR}/keys
Environment=PHAROS_SECURITY_TIER=${tier}
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

    activate_systemd_service "pharos-server"
}

install_pulse() {
    local host_arg="${1:-}"
    ensure_system_user

    local host="${host_arg:-${PHAROS_HOST:-127.0.0.1:2378}}"
    if [[ ! "${host}" =~ ^[A-Za-z0-9.-]+(:[0-9]+)?$ ]]; then
        error "Invalid host value for Pharos Pulse: '${host}'"
    fi
    [[ "${host}" == *:* ]] || host="${host}:2378"

    log "Installing Pharos Pulse Agent..."
    download_binary "pharos-pulse"

    log "Checking whether ${host} already presents a trusted certificate (e.g. a public CA like Let's Encrypt)..."
    ensure_openssl
    local ca_cert_line=""
    local host_only="${host%%:*}"
    if [[ "${host_only}" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        pulse_host_is_ip="yes"
    else
        pulse_host_is_ip="no"
    fi
    if echo | timeout 10 openssl s_client -connect "${host}" -verify_hostname "${host_only}" -verify_return_error >/dev/null 2>&1; then
        pulse_ca_found="trusted"
    else
        if [[ -n "${fetch_ca_ssh_target:-}" ]] && ! ${SUDO} test -f "${PHAROS_DIR}/certs/pharos-ca.crt"; then
            fetch_ca_via_ssh "${fetch_ca_ssh_target}" || true
        fi

        # If this host already has an install.sh-provisioned CA (i.e. it also runs a
        # server/hub, or one was manually copied here, or --fetch-ca-ssh just fetched one), trust it automatically — pulse
        # otherwise has no way to trust the exact kind of certificate install.sh generates.
        if ${SUDO} test -f "${PHAROS_DIR}/certs/pharos-ca.crt"; then
            ca_cert_line="Environment=PHAROS_CA_CERT=${PHAROS_DIR}/certs/pharos-ca.crt"
            pulse_ca_found="yes"
        else
            pulse_ca_found="no"
        fi
    fi

    log "Configuring Systemd service for Pharos Pulse..."
    cat <<EOF | ${SUDO} tee /etc/systemd/system/pharos-pulse.service > /dev/null
[Unit]
Description=Pharos Pulse Agent
After=network.target

[Service]
ExecStart=${INSTALL_DIR}/pharos-pulse
Restart=always
User=pharos
Environment=PHAROS_SERVER=${host}
${ca_cert_line}

[Install]
WantedBy=multi-user.target
EOF

    activate_systemd_service "pharos-pulse"
}

install_web_console() {
    ensure_system_user
    log "Installing Pharos Web Console..."

    warn "Pharos Web Console ships as a container image (ghcr.io/<owner>/pharos-console-web) — see the Server Setup docs' container Quick Start to run it. It reuses this host's existing pharos-server TLS certificate. Native binary/systemd installation is not available yet."
}

install_toolbelt() {
    log "Installing Pharos Toolbelt (ph, mdb$([[ "${OS_NAME}" != "macos" ]] && echo ", pharos-scan"))..."
    download_binary "ph"
    download_binary "mdb"
    if [[ "${OS_NAME}" == "macos" ]]; then
        warn "pharos-scan is not distributed for macOS (LAN discovery tooling is Linux-only) — skipping."
    else
        download_binary "pharos-scan"
    fi

    log "Pharos Toolbelt installed to ${INSTALL_DIR}"
}

# --- Main Flow ---
main() {
    check_dependencies
    detect_os
    detect_arch

    local target=${1:-"node"}
    local host_override=${2:-}
    local fetch_ca_ssh_target=""

    if [[ $# -gt 2 ]]; then
        shift 2
        while [[ $# -gt 0 ]]; do
            case "$1" in
                --fetch-ca-ssh)
                    [[ -n "${2:-}" ]] || error "--fetch-ca-ssh requires a [user@]host argument."
                    fetch_ca_ssh_target="$2"
                    shift 2
                    ;;
                *)
                    error "Unknown argument: $1"
                    ;;
            esac
        done
    fi

    if [[ -n "${fetch_ca_ssh_target}" ]] && ! [[ "${fetch_ca_ssh_target}" =~ ^([A-Za-z0-9][A-Za-z0-9_.-]*@)?[A-Za-z0-9][A-Za-z0-9.-]*$ ]]; then
        error "Invalid --fetch-ca-ssh target: '${fetch_ca_ssh_target}'. Expected [user@]host, e.g. admin@192.168.1.5."
    fi

    log "Starting Pharos Installation: ${target} (${OS_NAME}/${ARCH_NAME})"

    case "${target}" in
        hub)
            log "Installing Pharos Hub (Server + Console + Scan)..."
            install_server "${host_override}"
            install_web_console
            download_binary "pharos-scan"
            ;;
        node)
            log "Installing Pharos Node (Pulse + ph + mdb)..."
            install_pulse "${host_override}"
            download_binary "ph"
            download_binary "mdb"
            ;;
        server)   install_server "${host_override}";;
        pulse)    install_pulse "${host_override}";;
        toolbelt) install_toolbelt;;
        *)        error "Unknown target: ${target}. Use hub, node, server, pulse, or toolbelt.";;
    esac

    echo -e "\n${GREEN}Successfully installed Pharos ${target}!${NC}"
    echo -e "Next Steps:"
    if [[ "${target}" == "hub" ]]; then
        echo -e "1. Security tier: ${tier} (unauthenticated reads; writes always need a key — see server-setup.mdx to change tiers)"
        echo -e "2. On a fresh install, a root-equivalent admin key was auto-generated at ${PHAROS_DIR}/keys/admin_id_ed25519 — treat it accordingly."
        echo -e "3. To use your own key instead: add it to ${PHAROS_DIR}/keys and run ${SUDO} systemctl reload pharos-server"
        echo -e "4. Verify the server is running: ${SUDO} systemctl status pharos-server"
        local next_num=5
        if [[ -n "${host_override}" ]]; then
            echo -e "${next_num}. TLS: certificate SAN includes ${host_override} — clients can connect using that hostname."
            next_num=$((next_num + 1))
        fi
        echo -e "${next_num}. Access the Web Console via container (see 'Server Setup' docs for container Quick Start)."
    elif [[ "${target}" == "server" ]]; then
        echo -e "1. Security tier: ${tier} (unauthenticated reads; writes always need a key — see server-setup.mdx to change tiers)"
        echo -e "2. On a fresh install, a root-equivalent admin key was auto-generated at ${PHAROS_DIR}/keys/admin_id_ed25519 — treat it accordingly."
        echo -e "3. To use your own key instead: add it to ${PHAROS_DIR}/keys and run ${SUDO} systemctl reload pharos-server"
        echo -e "4. Verify the server is running: ${SUDO} systemctl status pharos-server"
        if [[ -n "${host_override}" ]]; then
            echo -e "5. TLS: certificate SAN includes ${host_override} — clients can connect using that hostname."
        fi
    elif [[ "${target}" == "node" || "${target}" == "pulse" ]]; then
        if [[ "${pulse_ca_found:-no}" == "trusted" ]]; then
            echo -e "1. TLS: ${host_override:-your server} already presents a certificate this machine trusts (e.g. a public CA like Let's Encrypt) — no CA configuration needed."
        elif [[ "${pulse_ca_found:-no}" == "yes" ]]; then
            echo -e "1. TLS: found a local Pharos CA at ${PHAROS_DIR}/certs/pharos-ca.crt — pulse trusts it automatically."
        else
            echo -e "1. TLS: no local Pharos CA found. Next time, pass --fetch-ca-ssh <user@host> to install.sh to do this automatically."
            echo -e "   To fix this manually now, if ${host_override:-your server} is a REMOTE host, run:"
            echo -e "     scp <user@host>:${PHAROS_DIR}/certs/pharos-ca.crt /tmp/pharos-ca.crt && \\"
            echo -e "     ${SUDO} mkdir -p ${PHAROS_DIR}/certs && \\"
            echo -e "     ${SUDO} mv /tmp/pharos-ca.crt ${PHAROS_DIR}/certs/pharos-ca.crt && \\"
            echo -e "     ${SUDO} mkdir -p /etc/systemd/system/pharos-pulse.service.d && \\"
            echo -e "     printf '[Service]\\\\nEnvironment=PHAROS_CA_CERT=${PHAROS_DIR}/certs/pharos-ca.crt\\\\n' | ${SUDO} tee /etc/systemd/system/pharos-pulse.service.d/override.conf >/dev/null && \\"
            echo -e "     ${SUDO} systemctl daemon-reload && ${SUDO} systemctl restart pharos-pulse"
            if [[ "${pulse_host_is_ip:-no}" == "yes" ]]; then
                echo -e "   Note: you connected via a bare IP address. If ${host_override:-your server} uses a public certificate (e.g. Let's Encrypt), that cert won't cover a bare IP — connecting via its hostname instead may resolve this without needing any of the above."
            fi
        fi
        echo -e "2. Verify the pulse agent is running: ${SUDO} systemctl status pharos-pulse"
        echo -e "3. Check logs: ${SUDO} journalctl -u pharos-pulse -f"
    else
        echo -e "1. Try running 'ph search' or 'mdb status'"
    fi
}

main "$@"
