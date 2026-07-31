# CLAUDE.md — Pharos

See `AGENTS.md` for the engineering process this project expects any AI agent to follow: the
plan → builder → panel-review → remediate workflow for non-trivial fixes, the live-verification
discipline, the release-cutting sequence, TODO.md/GitHub Issues sync discipline, and issue triage
heuristics. That file is tool-agnostic and shared with Gemini-family tooling (`GEMINI.md`) and any
other agent working in this repo — read it before starting non-trivial work here.

`GEMINI.md` additionally carries product/architecture context (persona framing, RFC 2378 protocol
background, storage-tiering design, VSA/SOLID conventions) that's useful background even though it
was originally written for Gemini-family tooling.

## Zero-Host Execution

Default is container-only (Podman) — see `GEMINI.md`'s "Zero-Host Execution" section and
`CONTRIBUTING.md` for the exact policy. A local-dev carve-out permits host execution during an
interactive session, conditioned on the host's Rust toolchain matching `rust-toolchain.toml`
exactly (`rustc --version`) — check this before running anything on the host, not after.

## Standing rule

Never commit, push, tag a release, or close a GitHub issue without an explicit instruction each
time. A prior approval does not carry forward to the next change.
