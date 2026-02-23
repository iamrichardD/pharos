# Security Policy

## Supported Versions

Currently, the following versions of `pharos` are supported with security updates:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take the security of `pharos` seriously. If you believe you have found a security vulnerability, please report it to us privately to ensure the safety of the community.

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please send an email to: **pharos-security [at] iamrichardd.com**

### What to include in your report:
- A description of the vulnerability.
- The steps needed to reproduce the issue (proof-of-concept).
- The potential impact of the vulnerability.

### What to expect:
- You will receive an acknowledgment of your report within 48 hours.
- We will keep you informed of our progress as we investigate and resolve the issue.
- Once the issue is resolved, we will provide a fix in the next release and, if appropriate, publish a security advisory.

## DevSecOps Practices

The `pharos` project is built with security in mind from the ground up:

1.  **Zero-Host Execution:** All builds and tests are performed in isolated Podman containers to prevent host contamination and ensure reproducible, secure environments.
2.  **Strict Typing & Memory Safety:** By using **Rust**, we eliminate entire classes of memory safety vulnerabilities (buffer overflows, use-after-free, etc.).
3.  **Automated Scanning:** Our CI/CD pipeline includes automated security scanning (static analysis and dependency auditing).
4.  **Minimalist Architecture:** We follow the principle of least privilege and keep our attack surface small by minimizing dependencies and complexity.
5.  **Transparent Attribution:** All source files include a standardized prologue for clear traceability and legal compliance.

Thank you for helping us keep `pharos` secure!
