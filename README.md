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
*(Installation and configuration guides will be populated here as Phase 1 and 2 are completed).*

## Engineering Philosophy
This project is built using strict Zero-Host DevSecOps practices. All execution, testing, and dependency management happens securely within Podman containers. It enforces atomic unit testing, continuous integration, and transparent DORA metric tracking via GitHub Issues.

## License
This project is licensed under the **AGPL-3.0 License**.

We believe Home Labbers should have absolute, unfettered freedom to experiment, modify, and master their environments. At the same time, we require that Enterprise/SMB entities utilizing Pharos as a networked service contribute their modifications back to the community and maintain clear attribution. See the `LICENSE` file for full details.
