---
layout: default
title: Home
---

# Project Pharos: Empowering Infrastructure Management

Whether you're managing a complex Home Lab with Proxmox and LXC or leading an Enterprise DevSecOps team with LDAP and strict security, Pharos is designed to make you **badass** at your job.

We don't just provide a tool; we provide a high-performance, read-optimized client-server ecosystem based on **RFC 2378** that simplifies your infrastructure's source of truth.

## Why Pharos?

- **Zero-Latency Reads:** Optimized for 90%+ read environments.
- **Protocol First:** Adheres strictly to RFC 2378, the "Phonebook Protocol."
- **Home Lab Optimized:** File-level storage perfect for lightweight LXC containers.
- **Enterprise Ready:** LDAP-backed storage with standard schemas (`inetOrgPerson`, `ipHost`).
- **Secure by Default:** SSH-key based authorization for all write operations.

---

## Which Path Are You On?

### 🏠 The Home Labber
You want speed, simplicity, and restart-survivable storage for your Proxmox/LXC clusters. Pharos gives you a lightweight binary that requires zero complex database setup.

[Get Started with Home Lab Tier &rarr;](./docs/HOWTO.html#home-lab-tier-restart-survivable-storage)

### 🏢 The Enterprise Engineer
You need deep integration with existing LDAP systems and secure management via SSH keys. Pharos integrates into your DevSecOps pipeline with ease.

[Get Started with Enterprise Tier &rarr;](./docs/HOWTO.html#enterprise-tier-ldap--ssh-authentication)

---

## Project Performance & Velocity (DORA)

Pharos isn't just fast to run; it's fast to build. We track our performance through DORA metrics to ensure the highest standards of engineering excellence.

- **Deployment Frequency:** Weekly (v1.0.0 released Feb 2026)
- **Lead Time for Changes:** < 24 Hours
- **Change Failure Rate:** < 1%
- **Time to Restore Service:** < 1 Hour

---

[Explore the Architecture &rarr;](./docs/ARCHITECTURE.html)
