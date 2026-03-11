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

*   **MDB Search**: High-performance inventory querying.
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

## 5. Troubleshooting & Support

We want your experience with Pharos to be frictionless. If you encounter issues, check these common areas.

### Port Conflicts (Bug #81 Remediation)
If you see "Connection Refused", ensure your client and server are using the standard port:
- **Default Port:** `2378`
- **Web Console Port:** `3000`

If your environment requires a custom port, set the environment variable:
```bash
export PHAROS_PORT=2378
```

### Authentication Failures
If `403 Forbidden` or `401 Authentication Required` occurs:
1.  Verify your public key is in the server's authorized directory (`PHAROS_KEYS_DIR`).
2.  Ensure your SSH agent has the corresponding private key loaded: `ssh-add -l`.
3.  Check the server's **Security Tier**. If it's set to `Protected` or `Scoped`, even read operations require a login.

---

## 6. Sandbox Evaluation (Zero-Host)

The Pharos Sandbox is an ephemeral environment that allows you to evaluate the entire ecosystem (Server, Pulse, Web Console) without host pollution.

### Browser Access
The **Pharos Console** is accessible at:
*   **URL:** [http://localhost:3000](http://localhost:3000)
*   **Username:** `admin`
*   **Password:** `admin`

### CLI Access (Podman)
To interact with the running containers via the command line:

```bash
# Enter the server container (e.g., to run pharos-server commands)
podman exec -it pharos-server bash

# Check pharos-server logs
podman logs pharos-server

# Check pharos-web (Console) logs
podman logs pharos-web
```

### Manual Querying (Netcat)
Since the server implements the RFC 2378 protocol (TCP), you can query it directly using `nc`:

```bash
# Query the server for all machine records
echo "query type=machine" | nc localhost 2378
```
