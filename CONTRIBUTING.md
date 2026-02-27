/* ========================================================================
 * Project: pharos
 * Component: Documentation - Community
 * File: CONTRIBUTING.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Provides a standardized guide for developers, home labbers, and 
 * enterprise engineers to contribute to the Pharos project while 
 * maintaining architectural integrity and DevSecOps standards.
 * * Traceability:
 * Related to GitHub Issue #31, implements community contribution model.
 * ======================================================================== */

# Contributing to Pharos

Thank you for your interest in Pharos! We welcome contributions from Home Labbers, Enterprise Engineers, and anyone passionate about high-performance directory services.

Pharos is built on the principles of **Clean Architecture**, **Functional Design**, and **Zero-Host DevSecOps**. To maintain these standards, all contributors are expected to follow the guidelines below.

## 🚀 Getting Started

1. **Fork the Repository:** Create your own fork of the `pharos` repository on GitHub.
2. **Clone Your Fork:**
   ```bash
   git clone https://github.com/YOUR_USERNAME/pharos.git
   cd pharos
   ```
3. **Set Up the Environment:** Pharos uses **Podman** for all execution and testing. Never run commands directly on your host machine.
   * Use `Containerfile.test` for running tests.
   * Use `Containerfile.debug` for interactive development.

## 🛠️ Contribution Workflow

We follow a structured Git Flow tied to GitHub Issues.

1. **Find or Create an Issue:** Every change must be tracked by a GitHub Issue. If you find a bug or have a feature idea, open an issue first.
2. **Create a Branch:** Use a descriptive branch name prefix:
   * `feat/issue-X-feature-name`
   * `fix/issue-X-bug-name`
   * `docs/issue-X-topic`
3. **Implement Your Changes:**
   * Adhere to the **Standardized File Prologue** in every new source file.
   * Follow the **Clean Code** standards (SOLID, Atomic Unit Tests).
   * Ensure 100% path coverage for new logic.
4. **Local Validation:** Verify your changes inside the Podman environment:
   ```bash
   # Example: Running tests inside the test container
   podman build -t pharos-test -f Containerfile.test .
   podman run --rm pharos-test
   ```
5. **Submit a Pull Request (PR):**
   * PRs should be atomic and focused on a single issue.
   * Include a clear description of the change and the verification performed.
   * Link the PR to the corresponding GitHub Issue.

## 📐 Engineering Standards

### Zero-Host Execution (Strict)
All code execution, package management, and testing **MUST** occur inside a Podman container. This ensures environment parity and security.

### Standardized File Prologue
Every source file must begin with the standardized header block. Refer to `GEMINI.md` for the exact format.

### Testing Philosophy
* **XP Focus:** We value eXtreme Programming principles.
* **Atomic Tests:** Create tests for every conditional path and all I/O operations.
* **Semantic Naming:** Test functions must follow the format: `test_should_[expected_behavior]_when_[state_under_test]`.

## 📜 Code of Conduct

As a contributor, you are expected to uphold a professional and respectful environment. Focus on technical excellence, architectural integrity, and community empowerment.

## 🏛️ Project Governance

Pharos is maintained by **Richard D.** (Principal Systems Architect). All architectural decisions and PR merges are reviewed against the project's core philosophies and the RFC 2378 specification.

## ⚖️ Licensing

By contributing to Pharos, you agree that your contributions will be licensed under the **AGPL-3.0 License**.

---
*   *Making Badass Developers* - The philosophy of Kathy Sierra (Guides our focus on user success and cognitive ease, though we avoid using the literal term "Badass" in user-facing content).
*   *Purple Cow* - Seth Godin (Informs our approach to making Pharos remarkable through unique value).
