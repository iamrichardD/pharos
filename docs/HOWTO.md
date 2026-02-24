# Pharos How-To Guides

Welcome to Project Pharos. These guides will help you get started whether you are a Home Labber or an Enterprise Engineer.

## 🏠 Home Lab Tier: Restart-Survivable Storage

For Home Lab environments (e.g., Proxmox LXC), Pharos provides a file-based storage engine that survives server restarts.

### 1. Configuration
Set the `PHAROS_STORAGE_PATH` environment variable to point to your data file.

```bash
export PHAROS_STORAGE_PATH="/var/lib/pharos/data.json"
./pharos-server
```

### 2. Record Management
Add a new contact using the `ph` CLI:

```bash
# Add a person
./ph add name="John Doe" email="john@home.lab" type="person"

# Add a machine (using mdb)
./mdb add hostname="srv-01" ip="192.168.1.10" type="machine"
```

## 🏢 Enterprise Tier: LDAP & SSH Authentication

Pharos integrates seamlessly with Enterprise infrastructure via LDAP backends and SSH-key authorization.

### 1. LDAP Integration
Configure LDAP connection details:

```bash
export PHAROS_LDAP_URL="ldap://ldap.enterprise.com:389"
export PHAROS_LDAP_BIND_DN="cn=pharos,ou=services,dc=enterprise,dc=com"
export PHAROS_LDAP_BIND_PW="secret"
export PHAROS_LDAP_BASE_DN="dc=enterprise,dc=com"
./pharos-server
```

Pharos maps fields to standard object classes:
- **People:** `inetOrgPerson` (maps `name` -> `cn`, `email` -> `mail`)
- **Machines:** `ipHost` (maps `hostname` -> `cn`, `ip` -> `ipHostNumber`)

### 2. SSH Authentication
Secure your write operations by authorizing SSH keys.

1.  **Server Setup:** Place authorized public keys in a directory.
    ```bash
    mkdir -p /etc/pharos/keys
    cp ~/.ssh/id_ed25519.pub /etc/pharos/keys/admin.pub
    export PHAROS_KEYS_DIR="/etc/pharos/keys"
    ```

2.  **Client Setup:** The CLI clients will look for `~/.ssh/id_ed25519` by default.
    ```bash
    # Command will trigger challenge-response automatically
    ./ph add name="New User" email="user@enterprise.com"
    ```

## 📊 Monitoring

Pharos exposes Prometheus metrics on port `9090`.

- **Endpoint:** `http://localhost:9090/metrics`
- **Key Metrics:**
    - `pharos_cpu_usage_percentage`
    - `pharos_memory_usage_bytes`
    - `pharos_total_records`
