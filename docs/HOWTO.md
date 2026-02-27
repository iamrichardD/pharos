# Pharos How-To Guides

Welcome to Project Pharos. This documentation is organized to help you first understand the **client tools** and how they automate your workflows, followed by the technical details of setting up the **Pharos server**.

---

## 1. CLI Clients (`ph` & `mdb`)

The primary way to interact with your Pharos registry is through our specialized CLI clients. These are lightweight, dependency-free binaries built for speed and scriptability.

### Basic Usage
Pharos uses a simple key-value query syntax.

```bash
# Query for a person (ph client)
./ph name="John Doe"

# Query for a machine (mdb client)
./mdb hostname="srv-web-01"
```

### Adding Records
Write operations require an authorized SSH key (using `~/.ssh/id_ed25519` by default).

```bash
# Add a person
./ph add name="Jane Smith" email="jane@lab.local" type="person"

# Add a machine
./mdb add hostname="db-01" ip="10.0.0.5" type="machine" status="up"
```

---

## 2. Management Console & WebMCP

The **Pharos Console** is the dynamic interface for your infrastructure.

*   **Pulse Monitor**: Real-time visualization of node health (CPU/Memory).
*   **Key Management**: Enroll and revoke SSH keys for write access.
*   **WebMCP**: Securely grant AI agents (like Gemini or Claude) access to manage your lab with human-in-the-loop safety.

To enable the console on your server:
```bash
export PHAROS_CONSOLE_ENABLE=true
./pharos-server
```

---

## 3. Automation Workflows

Pharos is built to be the automated backbone of your DevOps pipeline.

### Proxmox Hooks
Automate inventory registration whenever an LXC container starts:
```bash
# In your Proxmox hook script
mdb add hostname="$HOSTNAME" ip="$IP" type="machine" vmid="$VMID" status="up"
```

### CI/CD Integration
Update your machine records automatically after a successful build in GitHub Actions:
```yaml
run: |
  mdb add hostname="web-app-v2" ip="${{ steps.deploy.outputs.ip }}" type="machine"
```

---

## 4. Server Setup (Technical Details)

The `pharos-server` acts as the central registry.

### Home Lab Tier (LXC)
Uses persistent JSON storage for a simple, restart-survivable setup.
```bash
export PHAROS_STORAGE_PATH="/var/lib/pharos/data.json"
./pharos-server
```

### Enterprise Tier (LDAP)
Acts as a high-speed cache for your corporate directory.
```bash
export PHAROS_LDAP_URL="ldap://ldap.enterprise.com:389"
./pharos-server
```

### Security Configuration
Authorize SSH keys for write access:
```bash
mkdir -p /etc/pharos/keys
cp ~/.ssh/id_ed25519.pub /etc/pharos/keys/admin.pub
export PHAROS_KEYS_DIR="/etc/pharos/keys"
```

---

## 5. Automated Discovery (`pharos-scan`)

Scanning the network is a great next step after your server is successfully installed. `pharos-scan` uses mDNS and port fingerprinting to find every node in your lab and provision them into your registry with a single keystroke.

1.  **Run the Scanner:**
    ```bash
    ./pharos-scan
    ```

2.  **Interactive TUI:** Use the arrow keys to select discovered nodes and `Enter` to provision them directly into your `mdb` registry using your SSH key.
