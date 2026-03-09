#!/bin/bash
# ========================================================================
# Project: pharos
# Component: Sandbox Utility
# File: scripts/gen-sandbox-certs.sh
# Author: Richard D. (https://github.com/iamrichardd)
# License: AGPL-3.0 (See LICENSE file for details)
# * Purpose (The "Why"):
# Generates an ephemeral Internal CA and signs certificates for the 
# Pharos Sandbox services (Server, Web). This ensures end-to-end 
# encryption without persisting secrets in Git or on the host.
# ========================================================================

set -e

CERT_DIR=${1:-./certs}
mkdir -p "$CERT_DIR"

echo "Creating Ephemeral Sandbox Root CA..."
openssl genrsa -out "$CERT_DIR/root-ca.key" 4096
openssl req -x509 -new -nodes -key "$CERT_DIR/root-ca.key" -sha256 -days 1 -out "$CERT_DIR/root-ca.crt" -subj "/C=US/ST=Sandbox/L=Pharos/O=Pharos Ecosystem/CN=Pharos Sandbox Root CA"

# --- Function to sign a certificate ---
sign_cert() {
    local name=$1
    local dns=$2
    echo "Generating certificate for $name ($dns)..."
    
    openssl genrsa -out "$CERT_DIR/$name.key" 2048
    openssl req -new -key "$CERT_DIR/$name.key" -out "$CERT_DIR/$name.csr" -subj "/CN=$dns"
    
    cat > "$CERT_DIR/$name.ext" <<EOF
[v3_req]
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, nonRepudiation, keyEncipherment, dataEncipherment
subjectAltName = @alt_names

[alt_names]
DNS.1 = $dns
DNS.2 = localhost
IP.1 = 127.0.0.1
EOF

    openssl x509 -req -in "$CERT_DIR/$name.csr" -CA "$CERT_DIR/root-ca.crt" -CAkey "$CERT_DIR/root-ca.key" \
    -CAcreateserial -out "$CERT_DIR/$name.crt" -days 1 -sha256 -extfile "$CERT_DIR/$name.ext" -extensions v3_req
    
    rm "$CERT_DIR/$name.csr" "$CERT_DIR/$name.ext"
}

# Sign for Pharos Server
sign_cert "pharos-server" "pharos-server"

# Sign for Pharos Web
sign_cert "pharos-web" "pharos-web"

echo "PKI Bootstrap Complete."
ls -l "$CERT_DIR"
