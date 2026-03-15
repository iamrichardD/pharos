# Pharos

**The Sovereign Cockpit for your lab. Built on an Invisible High-Performance Engine.**

Pharos is a highly optimized, cross-platform client-server ecosystem built to modernize **RFC 2378** (The Phonebook Protocol). Designed with rigorous *Clean Architecture* principles, Pharos transforms the high-performance RFC 2378 engine into an actionable, resource-first command center for both Humans and AI Agents.

## The Ecosystem

Pharos is composed of four interconnected pieces:
1. **`pharos-server` (The Engine):** An invisible, read-optimized, ultra-fast backend daemon. It natively understands discriminators to seamlessly route requests for human contacts or machine assets via the RFC 2378 protocol.
2. **`pharos-console-web` (The Sovereign Cockpit):** The primary Human/AI interface. A resource-first Web Console that implements the **Model Context Protocol (MCP)** via WebMCP, allowing AI agents to safely manage your lab.
3. **`ph` (CLI):** The people-contact client for the terminal.
4. **`mdb` (CLI):** The machine database client for the terminal.

## Storage That Scales With You

Pharos maintains engineering integrity by distinguishing between Home Lab and Enterprise capabilities:
* **Dev Mode:** Lightning-fast, in-memory execution.
* **Home Lab (Full CRUD):** File-level, restart-survivable storage optimized perfectly for Proxmox LXC containers.
* **Enterprise (Read-Only):** Full LDAP-backed integration, acting as a high-speed, read-only proxy for your corporate source of truth.

## Getting Started

Pharos is designed to be easy to deploy and use.

## Documentation

The latest documentation, architecture diagrams, and how-to guides are available on our official marketing site:

👉 **[iamrichardd.github.io/pharos/](https://iamrichardd.github.io/pharos/)**

Detailed guides include:
- **[CLI Clients](https://iamrichardd.github.io/pharos/docs/cli-clients/)** - Master the `ph` and `mdb` tools.
- **[Management Console](https://iamrichardd.github.io/pharos/docs/console/)** - Real-time dashboard and WebMCP.
- **[Automation Workflows](https://iamrichardd.github.io/pharos/docs/automation/)** - Proxmox and CI/CD integration.
- **[Server Setup](https://iamrichardd.github.io/pharos/docs/server-setup/)** - Technical backend configuration.
- **[Network Scan](https://iamrichardd.github.io/pharos/docs/network-scan/)** - Automated discovery and provisioning.
- **[Architecture Overview](https://iamrichardd.github.io/pharos/docs/architecture/)** - Deep dive into system design.
- **[Contributing Guide](CONTRIBUTING.md)** - How to contribute to the Pharos ecosystem.

Local Markdown versions are also maintained in the [`docs/`](docs/) directory.

### Quick Start (Sandbox / Lab-in-a-Box)
The fastest way to evaluate Pharos is via our one-liner sandbox deployment:
```bash
curl -sSL https://raw.githubusercontent.com/iamrichardD/pharos/main/deploy/sandbox.yml -o sandbox.yml && \
podman-compose -f sandbox.yml up -d && \
rm sandbox.yml
```

Once running, you can access the ecosystem:
*   **Web Console:** [http://localhost:3000](http://localhost:3000) (User: `admin` / Pass: `admin`)
*   **Pharos Server (RFC 2378):** `localhost:2378`
*   **Interactive CLI Access:**
    ```bash
    # Access the server container shell
    podman exec -it pharos-server bash
    
    # Access the Web Console container shell
    podman exec -it pharos-web bash
    ```

## Engineering Philosophy
This project is built using strict **Zero-Host Execution** practices. All execution, testing, and dependency management occurs securely within Podman containers, ensuring total environmental parity and absolute security for CI/CD and production deployments. By isolating the build and run environments from the host system, we eliminate "it works on my machine" issues and provide a predictable, reproducible lifecycle. It further enforces atomic unit testing, continuous integration, and transparent DORA metric tracking via GitHub Issues.

## License
This project is licensed under the **AGPL-3.0 License**.

We believe Home Labbers should have absolute, unfettered freedom to experiment, modify, and master their environments. At the same time, we require that Enterprise/SMB entities utilizing Pharos as a networked service contribute their modifications back to the community and maintain clear attribution. See the `LICENSE` file for full details.

## Troubleshooting

### Sandbox: "Connection Refused" to GHCR.io
If the "One-Liner" fails with a `connection refused` error while pulling from `ghcr.io`, your DNS (e.g., Pi-hole, AdGuard, or corporate firewall) may be blocking the GitHub Container Registry.

**Fix:** Ensure `ghcr.io` is whitelisted in your DNS, or try forcing a public DNS for the pull:
```bash
podman-compose --podman-pull-args="--dns 8.8.8.8" -f sandbox.yml up -d
```

### Sandbox: "403 Forbidden" from GHCR.io
If you receive a `403 Forbidden` error, the packages may still be marked as "Private" (the default for new GHCR images).

**Fix:** Navigate to your GitHub profile -> **Packages**, select the `pharos-*` images, and change their visibility to **Public** in the "Package Settings" at the bottom of the page.

### Sandbox: "syscall bdflush: permission denied"
If you see an error related to `seccomp` and `bdflush` (common on Ubuntu 24.04 with older container runtimes), the sandbox configuration now includes a bypass (`seccomp: unconfined`) to ensure a smooth evaluation.

**Note:** This bypass is only for the ephemeral sandbox and should not be used in production environments where a custom seccomp profile or an updated runtime (`crun`) is preferred.
