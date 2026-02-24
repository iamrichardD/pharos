# Pharos

**Lightning-fast directory services for your people and your infrastructure.**



Pharos is a highly optimized, cross-platform client-server ecosystem built to modernize **RFC 2378** (The Phonebook Protocol). Designed with rigorous *Clean Architecture* principles, Pharos empowers Home Labbers and Enterprise Engineers to manage directories with uncompromised speed and total environment awareness.

## The Ecosystem

Pharos is composed of three interconnected pieces:
1. **`pharos` (Server):** A read-optimized, ultra-fast backend daemon. It natively understands discriminators to seamlessly route requests for human contacts or machine assets. Designed specifically for Ubuntu LTS, it boasts deep environment introspection, customizable threshold alerting, and robust observability (Push/Pull metrics).
2. **`ph` (CLI):** The people-contact client. Read-only by default, requiring cryptographic SSH-key authentication for updates.
3. **`mdb` (CLI):** The machine database client. Instantly query your hardware, servers, and cloud assets.

## Storage That Scales With You

Pharos meets you where your infrastructure is at:
* **Dev Mode:** Lightning-fast, in-memory execution.
* **Home Lab (MVP):** File-level, restart-survivable storage optimized perfectly for Proxmox LXC containers.
* **Enterprise:** Full LDAP-backed integration with a customizable, standard-compliant schema.

## Getting Started

Pharos is designed to be easy to deploy and use.

## Documentation

The latest documentation, architecture diagrams, and how-to guides are available on our official marketing site:

👉 **[iamrichardd.github.io/pharos/](https://iamrichardd.github.io/pharos/)**

Detailed guides include:
- **[Architecture Overview](https://iamrichardd.github.io/pharos/docs/architecture/)** - Deep dive into the system design.
- **[How-To Guides](https://iamrichardd.github.io/pharos/docs/howto/)** - Deployment and usage instructions for Home Lab and Enterprise.

Local Markdown versions are also maintained in the [`docs/`](docs/) directory.

### Quick Start
1. **Server:** Start the server using the in-memory backend for testing.
   ```bash
   ./pharos-server
   ```
2. **Client:** Query the server using `ph` or `mdb`.
   ```bash
   ./ph name=john
   ```

## Engineering Philosophy
This project is built using strict Zero-Host DevSecOps practices. All execution, testing, and dependency management happens securely within Podman containers. It enforces atomic unit testing, continuous integration, and transparent DORA metric tracking via GitHub Issues.

## License
This project is licensed under the **AGPL-3.0 License**.

We believe Home Labbers should have absolute, unfettered freedom to experiment, modify, and master their environments. At the same time, we require that Enterprise/SMB entities utilizing Pharos as a networked service contribute their modifications back to the community and maintain clear attribution. See the `LICENSE` file for full details.
