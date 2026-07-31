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
export PHAROS_LDAP_URL="ldap://ldap.example.com:389"
./pharos-server
```

### Security Configuration
**Note:** a fresh install defaults to the `open` security tier — unauthenticated reads are allowed over the network; writes always require a key, in every tier. If you're exposing the server beyond a trusted local network, set `PHAROS_SECURITY_TIER=protected` (see below for provisioning a key first, since `protected`/`scoped` refuse to self-generate one).

Authorize SSH keys for write access:
```bash
mkdir -p /etc/pharos/keys
cp ~/.ssh/id_ed25519.pub /etc/pharos/keys/admin.pub
export PHAROS_KEYS_DIR="/etc/pharos/keys"
```
Enrolling or rotating a key takes effect immediately, no restart needed — `pharos-server` re-scans `PHAROS_KEYS_DIR` on `SIGHUP`:
```bash
systemctl reload pharos-server   # or: kill -HUP $(pgrep pharos-server)
```
The same reload also picks up a renewed `PHAROS_TLS_CERT`/`PHAROS_TLS_KEY` pair, if you're using an externally renewed certificate — no restart needed for that either.

**Note:** `protected`/`scoped` tiers refuse to self-generate an admin credential (that only happens for `open`). If you switch to `protected`/`scoped` with an empty keys directory, the server starts but rejects every authenticated command until you enroll a key and reload.

---

## 5. Troubleshooting & Support

We want your experience with Pharos to be frictionless. If you encounter issues, check these common areas.

### Port Conflicts (Bug #81 Remediation)
If you see "Connection Refused", ensure your client and server are using the standard port:
- **Default Port:** `2378`
- **Web Console Port:** `3000`

These are two different variables for two different components — setting one does not affect the
other:
- To change which port **`pharos-server` listens on**, set `PHAROS_ADDR` (host *and* port
  together), e.g. `export PHAROS_ADDR=0.0.0.0:9999`.
- To tell **`ph`/`mdb`/`pharos-scan`** which port to connect to, set `PHAROS_PORT`, e.g.
  `export PHAROS_PORT=9999`. This has no effect on the server itself.

### Quiet or Missing Logs
Set `RUST_LOG` to control verbosity (`error`/`warn`/`info`/`debug`/`trace`, default `info`). **Careful with typos** — a value that doesn't match any known module (including a plain typo) silently disables all output with no warning, not even at the default level.

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
