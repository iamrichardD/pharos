/* ========================================================================
 * Project: pharos
 * Component: Documentation & UX
 * File: pulse-ca-fetch-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Self-contained implementation plan for Gap 1 of the node-install UX
 * investigation: joining a remote node to a hub currently requires the
 * operator to manually copy the hub's CA cert, hand-edit a systemd unit,
 * and restart pulse. This adds an opt-in, non-interactive SSH-based fetch
 * that automates that copy when the operator already has SSH access to
 * the hub, falling back to today's manual instructions otherwise.
 * * Traceability:
 * Related to the node-install UX panel investigation (2026-08-02).
 * ======================================================================== */

# Plan: opt-in SSH-based CA fetch for remote node installs (Gap 1)

## Background / design decision (already made, do not re-litigate)

Panel-reviewed design (Kent Beck, Robert Martin, Martin Fowler, Kathy Sierra, Seth Godin, Ubuntu
System Specialist, DX/UX Specialist), confirmed by the user:

- **Opt-in only** — default `install.sh` behavior (no new flag) is byte-for-byte unchanged.
- **Mechanism: SSH, not a new HTTP endpoint.** The operator installing a node already has SSH
  access to the hub in the realistic current scenario (a team standing up their own hub +
  connecting a dev laptop as a node) — reuse that existing trust relationship instead of building
  new server-side infrastructure (a `pharos-server` CA-distribution endpoint is explicitly
  out of scope, see Non-goals).
- **Must be non-interactive-safe.** `curl | bash` consumes stdin, so this can never prompt.
  `ssh -o BatchMode=yes` guarantees a clean, fast failure (never hangs on a password/host-key
  prompt) if passwordless key auth isn't already set up — in which case this falls back to
  today's manual instructions, unchanged.
- **No new TOFU weakening.** Do **not** pass `-o StrictHostKeyChecking=accept-new`. If the hub's
  SSH host key isn't already in the operator's `known_hosts` (i.e., they've never SSH'd there
  before), the fetch fails closed and falls back to manual instructions — it never silently
  trusts a new SSH host key on the operator's behalf.
- **Visibility, not confirmation.** Print the fetched CA's SHA-256 fingerprint to the install log
  so the operator can see what was trusted. Since there's no interactive prompt available, this
  is informational, not a blocking confirmation gate.

## Current code (quoted verbatim, re-read fresh this session)

`scripts/install.sh:395-423` (argument parsing — no flag support exists today, only two
positional args):

```bash
# --- Main Flow ---
main() {
    check_dependencies
    detect_os
    detect_arch

    local target=${1:-"node"}
    local host_override=${2:-}

    log "Starting Pharos Installation: ${target} (${OS_NAME}/${ARCH_NAME})"

    case "${target}" in
        hub)
            log "Installing Pharos Hub (Server + Console + Scan)..."
            install_server "${host_override}"
            install_web_console
            download_binary "pharos-scan"
            ;;
        node)
            log "Installing Pharos Node (Pulse + ph + mdb)..."
            install_pulse "${host_override}"
            download_binary "ph"
            download_binary "mdb"
            ;;
        server)   install_server "${host_override}";;
        pulse)    install_pulse "${host_override}";;
        toolbelt) install_toolbelt;;
        *)        error "Unknown target: ${target}. Use hub, node, server, pulse, or toolbelt.";;
    esac
```

`scripts/install.sh:331-372` (`install_pulse` — where CA detection currently happens):

```bash
install_pulse() {
    local host_arg="${1:-}"
    ensure_system_user

    local host="${host_arg:-${PHAROS_HOST:-127.0.0.1:2378}}"
    if [[ ! "${host}" =~ ^[A-Za-z0-9.-]+(:[0-9]+)?$ ]]; then
        error "Invalid host value for Pharos Pulse: '${host}'"
    fi

    log "Installing Pharos Pulse Agent..."
    download_binary "pharos-pulse"

    # If this host already has an install.sh-provisioned CA (i.e. it also runs a
    # server/hub, or one was manually copied here), trust it automatically — pulse
    # otherwise has no way to trust the exact kind of certificate install.sh generates.
    local ca_cert_line=""
    if ${SUDO} test -f "${PHAROS_DIR}/certs/pharos-ca.crt"; then
        ca_cert_line="Environment=PHAROS_CA_CERT=${PHAROS_DIR}/certs/pharos-ca.crt"
        pulse_ca_found="yes"
    else
        pulse_ca_found="no"
    fi
    ...
```

`scripts/install.sh:446-451` (the manual-instructions message this change must NOT alter the
wording of — it should simply stop being reached in the success case, because the CA file will
already exist by the time this check runs):

```bash
    elif [[ "${target}" == "node" || "${target}" == "pulse" ]]; then
        if [[ "${pulse_ca_found:-no}" == "yes" ]]; then
            echo -e "1. TLS: found a local Pharos CA at ${PHAROS_DIR}/certs/pharos-ca.crt — pulse trusts it automatically."
        else
            echo -e "1. TLS: no local Pharos CA found. If ${host_override:-your server} is a REMOTE host, copy ITS ${PHAROS_DIR}/certs/pharos-ca.crt to this machine, add 'Environment=PHAROS_CA_CERT=<path-to-copied-file>' to /etc/systemd/system/pharos-pulse.service, then run: ${SUDO} systemctl daemon-reload && ${SUDO} systemctl restart pharos-pulse"
        fi
```

**Key insight driving the minimal-diff design**: the "Next Steps" message already checks whether
`${PHAROS_DIR}/certs/pharos-ca.crt` exists locally (`pulse_ca_found`) and prints the right message
either way. So this change does **not** need to touch that message-printing logic at all — it only
needs to *place the CA file at that exact path before `install_pulse`'s existing check runs*. If
the fetch succeeds, the existing "found a local Pharos CA" branch fires naturally and correctly.
If it fails, the existing manual-instructions branch fires naturally and correctly. That's the
entire integration surface.

## The change

**1. `scripts/install.sh` argument parsing (`main()`, lines 401-402 area)** — add support for an
optional `--fetch-ca-ssh <[user@]host>` flag anywhere after the two existing positional args,
without disturbing existing positional parsing:

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

**2. New function `fetch_ca_via_ssh`** (place near `setup_pki`, since it's PKI-related):

```bash
# Best-effort, opt-in CA fetch over SSH for a remote node joining an existing hub. Never
# prompts (curl | bash has no stdin to prompt on) and never weakens SSH's own trust model —
# if the hub's SSH host key isn't already known, or key-based auth isn't already set up, this
# fails fast and the caller falls back to the existing manual-copy instructions.
fetch_ca_via_ssh() {
    local ssh_target=$1
    local cert_dir="${PHAROS_DIR}/certs"

    log "Attempting to fetch hub CA cert via SSH from ${ssh_target}..."
    ${SUDO} mkdir -p "${cert_dir}"

    local fetched
    if ! fetched="$(ssh -o BatchMode=yes -o ConnectTimeout=10 "${ssh_target}" \
        "sudo cat ${cert_dir}/pharos-ca.crt" 2>/dev/null)" || [[ -z "${fetched}" ]]; then
        warn "Could not fetch hub CA cert via SSH from ${ssh_target} (no passwordless SSH/sudo, or the file doesn't exist there) — falling back to manual TLS trust instructions below."
        return 1
    fi

    echo "${fetched}" | ${SUDO} tee "${cert_dir}/pharos-ca.crt" > /dev/null
    ${SUDO} chmod 644 "${cert_dir}/pharos-ca.crt"

    local fingerprint
    fingerprint="$(${SUDO} openssl x509 -in "${cert_dir}/pharos-ca.crt" -noout -fingerprint -sha256 2>/dev/null | cut -d= -f2)"
    if [[ -z "${fingerprint}" ]]; then
        warn "Fetched data from ${ssh_target} was not a valid certificate — removing it and falling back to manual TLS trust instructions below."
        ${SUDO} rm -f "${cert_dir}/pharos-ca.crt"
        return 1
    fi

    log "Fetched hub CA cert via SSH from ${ssh_target} (SHA-256 fingerprint: ${fingerprint})"
    return 0
}
```

**3. Call it from `install_pulse`**, immediately before the existing CA-detection check
(`scripts/install.sh:346-352`), only if a target was given and no local CA already exists:

```bash
    log "Installing Pharos Pulse Agent..."
    download_binary "pharos-pulse"

    if [[ -n "${fetch_ca_ssh_target:-}" ]] && ! ${SUDO} test -f "${PHAROS_DIR}/certs/pharos-ca.crt"; then
        fetch_ca_via_ssh "${fetch_ca_ssh_target}" || true
    fi

    # If this host already has an install.sh-provisioned CA (i.e. it also runs a
    # server/hub, one was manually copied here, or --fetch-ca-ssh just fetched one), trust it
    # automatically — pulse otherwise has no way to trust the exact kind of certificate
    # install.sh generates.
    local ca_cert_line=""
    if ${SUDO} test -f "${PHAROS_DIR}/certs/pharos-ca.crt"; then
        ca_cert_line="Environment=PHAROS_CA_CERT=${PHAROS_DIR}/certs/pharos-ca.crt"
        pulse_ca_found="yes"
    else
        pulse_ca_found="no"
    fi
```

`install_pulse`'s signature and its `host_arg`/`host` handling are unchanged. `fetch_ca_ssh_target`
is a `main()`-local variable — pass it to `install_pulse` as a second parameter, or reference it
via a global (this codebase's existing style already uses bare globals for cross-function state
like `tier` and `pulse_ca_found` — set via `main()` and read/set inside the `install_*` functions —
so follow that same convention rather than introducing parameter-passing that doesn't match the
rest of the file).

## Non-goals (do not touch)

- **Do not** add any new `pharos-server` HTTP/network endpoint for CA distribution — SSH reuses
  existing trust and requires zero server-side changes. A server-side enrollment/token protocol is
  a legitimate future idea but explicitly out of scope for this change.
- **Do not** pass `-o StrictHostKeyChecking=accept-new` or otherwise auto-accept an unknown SSH
  host key. If the hub isn't already in the operator's `known_hosts`, this must fail closed to the
  manual fallback, not silently establish new trust.
- **Do not** add any interactive confirmation prompt — `curl | bash` has no stdin to prompt on;
  the fingerprint is logged for visibility only.
- **Do not** change the wording or logic of the existing "Next Steps" message block
  (`scripts/install.sh:446-453`) — it already does the right thing once the CA file exists at the
  expected path; leave it untouched.
- **Do not** change `install_server`/`setup_pki` (the hub side) at all — this is a pure node-side,
  opt-in addition.
- **Do not** change default (no-flag) `install.sh` behavior in any way — every existing invocation
  form (`install.sh`, `install.sh -- node`, `install.sh -- node 192.168.1.5`, `install.sh -- hub
  ...`) must behave byte-for-byte identically to today.
- **Do not** touch Gap 2's already-shipped fix in `crates/pharos-pulse/src/main.rs`, or Gap 3
  (the pulse version string) — unrelated, separately scoped work.

## Verification steps (concrete)

Per this repo's established convention (see the repo-conventions memory: `scripts/install.sh` ends
in an unguarded `main "$@"`, so **never** `source` it directly — extract functions with `sed -n
'/^funcname()/,/^}/p'` into a throwaway harness with stubbed `log()`/`warn()`/`SUDO=""`, and in this
case also stub `PHAROS_DIR` to a temp directory so nothing touches the real `/etc/pharos`).

1. **Argument-parsing test**: extract just the new parsing block into a harness; confirm:
   - `target=node host_override= fetch_ca_ssh_target=` (no args) → unchanged from today.
   - `target=node host_override=192.168.1.5` (existing two-arg form) → `fetch_ca_ssh_target=""`,
     unchanged from today.
   - `target=node host_override=192.168.1.5 --fetch-ca-ssh admin@192.168.1.5` → `fetch_ca_ssh_target="admin@192.168.1.5"`.
   - `target=node --fetch-ca-ssh admin@192.168.1.5` (no host_override) → still parses correctly
     with `host_override=""`.
   - An unrecognized third argument that isn't `--fetch-ca-ssh` → `error "Unknown argument: ..."`
     (exit non-zero), not silently ignored.

2. **Live SSH-fetch test (harder/more real than a stub)** — simulate the two-machine scenario on
   one box using SSH to `localhost`:
   - Confirm passwordless SSH to `localhost` works first: `ssh -o BatchMode=yes localhost true`
     (generate a local test keypair and add it to `~/.ssh/authorized_keys` if not already set up
     for this purpose — do this in a scratch/test context, not by modifying the real user's
     SSH config).
   - Place a real, valid test certificate at a fake `${cert_dir}/pharos-ca.crt` path (stub
     `PHAROS_DIR` to a temp dir on the "hub" side reachable via that SSH session).
   - Run `fetch_ca_via_ssh localhost` (extracted, with `PHAROS_DIR` stubbed to a *different* temp
     dir representing "the node") and confirm: the cert lands at the node's stubbed
     `${cert_dir}/pharos-ca.crt`, its content matches the source file byte-for-byte, and the
     printed fingerprint matches `openssl x509 -in <source> -noout -fingerprint -sha256` computed
     independently.
   - **Fallback test**: run `fetch_ca_via_ssh` against a target that will fail
     (`nonexistent-host-xyz` or a real host with no key-based auth set up) and confirm it returns
     non-zero, prints the `warn` fallback message, and does **not** leave a partial/empty file at
     the destination path.

3. **Full `install_pulse` integration**, still via the sed-extraction harness with stubbed
   `PHAROS_DIR`/`SUDO=""`/`log`/`warn`/`error`/`download_binary` (stub the last one to a no-op —
   don't actually hit GitHub releases in this test): confirm that after a successful
   `--fetch-ca-ssh` fetch, `pulse_ca_found` ends up `"yes"` and `ca_cert_line` points at the
   fetched file — i.e., the existing "Next Steps" success-path branch will fire without any change
   to that branch's own code.

4. All shell-level testing here has no Rust build/test involved, so Podman-vs-host doesn't apply
   in the usual `cargo`/Zero-Host sense — but per `AGENTS.md`, treat any *actual SSH network
   activity* (even to localhost) as live verification, and do it independently in the panel-review
   stage even if the builder already claims to have tested it.

## Report back

State clearly: the exact diff (only `scripts/install.sh` should change), the parsing-test results
for all five argument forms above, the live SSH-fetch test result (success and fallback cases),
and explicit confirmation that no existing invocation form's behavior changed. Do not commit or
push — this repo requires explicit instruction for that, every time.
