# Pharos How-To Guides

Welcome to Project Pharos. These guides will help you get started whether you are a Home Labber or an Enterprise Engineer.

## 🏠 Home Lab Tier: Restart-Survivable Storage

For Home Lab environments (e.g., Proxmox LXC), Pharos provides a file-based storage engine that survives server restarts.

### Proxmox / LXC: 5-Minute Deployment

This guide will help you set up Pharos as your infrastructure source of truth within a Proxmox LXC container.

#### 1. Prepare the LXC Container
Create an Ubuntu 24.04 LXC container. We recommend at least 512MB RAM and 2GB Disk.

```bash
# Inside the LXC container
apt-get update && apt-get install -y ca-certificates
```

#### 2. Install the Pharos Server
Download the latest `pharos-server` binary for `x86_64-unknown-linux-gnu`.

```bash
wget https://github.com/iamrichardd/pharos/releases/download/v1.0.0/pharos-server-linux-x86_64
chmod +x pharos-server-linux-x86_64
mv pharos-server-linux-x86_64 /usr/local/bin/pharos-server
```

#### 3. Configure Persistent Storage
Create a directory for your data and set the environment variable.

```bash
mkdir -p /var/lib/pharos
export PHAROS_STORAGE_PATH="/var/lib/pharos/data.json"
```

    #### 4. Run as a Systemd Service
    For maximum reliability and uptime, run Pharos as a service so it's always available.
```bash
cat <<EOF > /etc/systemd/system/pharos.service
[Unit]
Description=Pharos Infrastructure Server
After=network.target

[Service]
ExecStart=/usr/local/bin/pharos-server
Environment=PHAROS_STORAGE_PATH=/var/lib/pharos/data.json
Restart=always
User=root

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now pharos
```

#### 5. Verification
Verify your server is responding to queries:

```bash
telnet localhost 1050
# Type: query name=test
# You should receive a response
```

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

### 3. DevSecOps Integration: Infrastructure as Code

Pharos is designed for high-velocity DevSecOps teams. You can automate your infrastructure records by integrating Pharos into your CI/CD pipelines.

#### 1. Configure the GitHub Actions / CI Server
Securely store the SSH private key used for infrastructure updates.

- **GitHub Secret:** `PHAROS_SSH_KEY`

#### 2. Workflow Example: Automatic Infrastructure Updates
Add a new machine record automatically after a successful build.

```yaml
jobs:
  update-inventory:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Deploy Pharos CLI
        run: |
          wget -q https://github.com/iamrichardd/pharos/releases/download/v1.0.0/mdb-linux-x86_64
          chmod +x mdb-linux-x86_64
          mv mdb-linux-x86_64 /usr/local/bin/mdb
      - name: Update Machine Record
        env:
          SSH_KEY: ${{ secrets.PHAROS_SSH_KEY }}
          PHAROS_SERVER: pharos.enterprise.com
        run: |
          mkdir -p ~/.ssh
          echo "$SSH_KEY" > ~/.ssh/id_ed25519
          chmod 600 ~/.ssh/id_ed25519
          mdb add hostname="web-app-v2" ip="${{ steps.deploy.outputs.ip }}" type="machine"
```

## 📊 Monitoring

Pharos exposes Prometheus metrics on port `9090`.

- **Endpoint:** `http://localhost:9090/metrics`
- **Key Metrics:**
    - `pharos_cpu_usage_percentage`
    - `pharos_memory_usage_bytes`
    - `pharos_total_records`
