/* ========================================================================
 * Project: pharos
 * Component: Documentation & UX
 * File: node-tls-trust-probe-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * install.sh's node install currently decides whether to show CA-trust
 * setup advice by checking one thing: does a local file called
 * pharos-ca.crt already exist on this machine? That's the wrong test. A
 * hub using a publicly-trusted certificate (e.g. Let's Encrypt) needs NO
 * CA configuration at all - pharos-client already loads native OS trust
 * roots - but today's logic can't tell the difference and shows the same
 * "no local CA found, do X/Y/Z" message regardless. This plan replaces the
 * file-existence check with an actual TLS-trust probe against the real
 * host, so operators using a public CA (confirmed to be the Implementation
 * team's actual setup) stop seeing irrelevant, alarming-looking advice for
 * a problem they don't have.
 * * Traceability:
 * Follow-up to the node-install UX panel investigation (2026-08-02/03),
 * the --fetch-ca-ssh feature (Gap 1, v1.10.0), and the fallback-message
 * simplification (in progress as of this plan). Prompted by confirmation
 * from the user that Pharos supports both a self-signed Pharos-generated
 * CA and an operator-supplied public CA (e.g. Let's Encrypt) for the hub,
 * and that the Implementation team's hub specifically uses the latter.
 *
 * SEQUENCING NOTE: do not dispatch/build this plan until the separate,
 * already-in-flight "node-ca-fallback-oneliner-plan.md" fix has landed and
 * been independently verified. Both plans touch the same "Next Steps"
 * printing block in main() (though this plan changes a different function,
 * install_pulse(), for its core logic) - building them concurrently risks
 * the exact kind of overlapping-edit conflict this repo hit earlier this
 * week. Re-read scripts/install.sh fresh immediately before starting this
 * plan's own work, since line numbers and exact surrounding text will have
 * shifted once the other fix lands.
 * ======================================================================== */

# Plan: probe real TLS trust instead of checking for a local CA file

## Background

`install_pulse()`'s current logic (quoted verbatim, `scripts/install.sh`, re-read fresh this
session):

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

    if [[ -n "${fetch_ca_ssh_target:-}" ]] && ! ${SUDO} test -f "${PHAROS_DIR}/certs/pharos-ca.crt"; then
        fetch_ca_via_ssh "${fetch_ca_ssh_target}" || true
    fi

    # If this host already has an install.sh-provisioned CA (i.e. it also runs a
    # server/hub, or one was manually copied here, or --fetch-ca-ssh just fetched one), trust it automatically — pulse
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

**The problem**: `pulse_ca_found` is set purely from "does `${PHAROS_DIR}/certs/pharos-ca.crt`
exist on THIS node" — a proxy for "will TLS trust succeed" that's only valid for the self-signed
Pharos-CA deployment path. Pharos hubs can also run with an operator-supplied public CA certificate
(confirmed: the Implementation team's hub uses Let's Encrypt) — in that case
`pharos-client::PharosClient::connect()` (`crates/pharos-client/src/lib.rs`) already trusts the
connection via native OS trust roots (`rustls_native_certs::load_native_certs()`) plus a webpki
fallback bundle, with **zero** `PHAROS_CA_CERT` needed. But `install_pulse()` has no way to know
this — it shows the exact same "no local CA found" fallback advice (copy a cert that may not even
be the relevant trust anchor, configure `PHAROS_CA_CERT`, etc.) to *every* node install that
doesn't happen to have a local `pharos-ca.crt` file, even when the real connection would already
succeed without any of that.

**The fix**: before falling back to the file-existence check (or attempting `--fetch-ca-ssh`),
actually attempt a real TLS handshake against `${host}` and check whether it's *already* trusted.
If it is — public CA or otherwise — skip all the CA-configuration logic entirely; nothing is
needed. Only fall through to today's existing logic (unchanged) if the probe fails.

**Hostname-vs-IP subtlety, found during design review — the probe must check this, not just chain
trust**: `pharos-client::PharosClient::connect()` (`crates/pharos-client/src/lib.rs`) splits
`addr` on `:` and uses only the host part for both SNI and hostname verification
(`ServerName::try_from(domain)`). A public CA like Let's Encrypt only ever issues certs for an
FQDN, never for a bare IP SAN. So the probe must check **hostname match**, not just **chain
trust** — `openssl s_client -verify_return_error` alone only checks the latter. Without
`-verify_hostname`, the probe could wrongly report "trusted" in a way that doesn't match what the
real Rust client would actually do. This means: an operator connecting via the hub's FQDN gets a
correct "trusted" probe result; an operator connecting via a bare IP to that same
publicly-certified hub correctly gets a *failed* probe (hostname mismatch) — matching the fact
that the real pulse agent would *also* fail TLS verification in that exact scenario. That failure
case should get its own hint (see the "Next Steps" branch below) rather than being silently
lumped in with the genuine self-signed-CA case.

## The change

**File: `crates/... ` — none. This is a pure `scripts/install.sh` change.**

**1. `install_pulse()`** — wrap the existing fetch/file-check block in a new probe gate:

```bash
    log "Installing Pharos Pulse Agent..."
    download_binary "pharos-pulse"

    log "Checking whether ${host} already presents a trusted certificate (e.g. a public CA like Let's Encrypt)..."
    ensure_openssl
    local ca_cert_line=""
    local host_only="${host%%:*}"
    local host_for_probe="${host}"
    [[ "${host}" == *:* ]] || host_for_probe="${host}:2378"
    if [[ "${host_only}" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        pulse_host_is_ip="yes"
    else
        pulse_host_is_ip="no"
    fi
    if echo | timeout 10 openssl s_client -connect "${host_for_probe}" -verify_hostname "${host_only}" -verify_return_error >/dev/null 2>&1; then
        pulse_ca_found="trusted"
    else
        if [[ -n "${fetch_ca_ssh_target:-}" ]] && ! ${SUDO} test -f "${PHAROS_DIR}/certs/pharos-ca.crt"; then
            fetch_ca_via_ssh "${fetch_ca_ssh_target}" || true
        fi

        # If this host already has an install.sh-provisioned CA (i.e. it also runs a
        # server/hub, or one was manually copied here, or --fetch-ca-ssh just fetched one), trust it automatically — pulse
        # otherwise has no way to trust the exact kind of certificate install.sh generates.
        if ${SUDO} test -f "${PHAROS_DIR}/certs/pharos-ca.crt"; then
            ca_cert_line="Environment=PHAROS_CA_CERT=${PHAROS_DIR}/certs/pharos-ca.crt"
            pulse_ca_found="yes"
        else
            pulse_ca_found="no"
        fi
    fi
```

Notes:
- `ensure_openssl` (existing function, already used by `setup_pki`) is called here too, since
  `install_pulse()` can run on a bare node that has never called `setup_pki` and may not have
  `openssl` installed yet — the probe needs it.
- `timeout 10` bounds the probe so a firewalled/unreachable host doesn't hang the installer;
  `coreutils`' `timeout` is a standard part of every mainstream Linux distro.
- `-verify_return_error` makes `openssl s_client` exit non-zero specifically when certificate
  verification fails — that's the exact signal being tested for, not just "did a TCP connection
  happen."
- If the probe fails for *any* reason (untrusted cert, unreachable host, firewall, `openssl`
  itself missing despite `ensure_openssl`), behavior falls through to exactly today's existing
  logic, unchanged — this is a strict addition, not a behavior change for the failure case.
- `pulse_ca_found="trusted"` is a **new** third state (existing states: `"yes"`, `"no"`) — the
  "Next Steps" printing block needs a new branch for it (see below).
- `pulse_host_is_ip` (new global, same cross-function convention this file already uses for
  `pulse_ca_found`/`tier`) records whether `${host_only}` is a bare IPv4 address, computed
  unconditionally (regardless of probe outcome) so it's available for the fallback branch's hint
  even when the probe never runs into trouble for IP-unrelated reasons.
- **Found during panel review, fixed above**: `${host}` can legally be a bare hostname/IP with
  *no port* (`install_pulse`'s own validation regex, `^[A-Za-z0-9.-]+(:[0-9]+)?$`, makes the port
  optional — and the file's own comment elsewhere gives `install.sh -- node 192.168.1.5` as an
  example with no port). Passing a portless `${host}` straight to `openssl s_client -connect`
  would make it default to port 443 — silently probing the wrong service entirely and producing a
  misleading result. Fixed with `host_for_probe`, which appends the documented default port
  (`2378`) only when `${host}` doesn't already contain one, purely for the probe's own connection
  target — `${host_only}` (used for `-verify_hostname`) is deliberately left alone since hostname
  verification doesn't involve a port at all. **Separately worth flagging, not fixing here**: a
  portless `${host}` also becomes the literal `PHAROS_SERVER` value written into the systemd unit
  — whether the real `pharos-pulse` binary's own `TcpStream::connect` handles a portless address
  correctly is a pre-existing question this plan doesn't touch; worth its own follow-up if
  confirmed broken.
- **Argument-injection check (given Gap 1's `--fetch-ca-ssh` history this session)**: unlike that
  earlier finding, `"${host_for_probe}"`/`"${host_only}"` are each the *value of an explicitly-named
  flag* (`-connect X`, `-verify_hostname Y`) rather than a bare positional argument — standard
  getopt-style parsing always consumes the next argv slot as that flag's value regardless of
  leading `-`, so a dash-prefixed host value cannot be reinterpreted as a different `openssl` flag
  the way an unnamed positional `ssh` argument could. Confirmed by reasoning about argument
  parsing, not just assumed — double-check this holds during implementation/verification rather
  than re-deriving it from scratch.

**2. `main()`'s "Next Steps" printing block** (the `node`/`pulse` branch — re-read this section
fresh at implementation time, since the separate in-flight fallback-message fix changes its `else`
branch's exact text; only the branch structure matters here, not the fallback text's specific
wording):

```bash
    elif [[ "${target}" == "node" || "${target}" == "pulse" ]]; then
        if [[ "${pulse_ca_found:-no}" == "trusted" ]]; then
            echo -e "1. TLS: ${host_override:-your server} already presents a certificate this machine trusts (e.g. a public CA like Let's Encrypt) — no CA configuration needed."
        elif [[ "${pulse_ca_found:-no}" == "yes" ]]; then
            echo -e "1. TLS: found a local Pharos CA at ${PHAROS_DIR}/certs/pharos-ca.crt — pulse trusts it automatically."
        else
            <the fallback block from the separately-landed one-liner fix — do not touch its wording, only add this new elif above it>
            if [[ "${pulse_host_is_ip:-no}" == "yes" ]]; then
                echo -e "   Note: you connected via a bare IP address. If ${host_override:-your server} uses a public certificate (e.g. Let's Encrypt), that cert won't cover a bare IP — connecting via its hostname instead may resolve this without needing any of the above."
            fi
        fi
        echo -e "2. Verify the pulse agent is running: ${SUDO} systemctl status pharos-pulse"
        echo -e "3. Check logs: ${SUDO} journalctl -u pharos-pulse -f"
```

## Non-goals (do not touch)

- **Do not** change the *wording* of the existing `"yes"` branch or the fallback (`else`) branch's
  existing text — only add the new `"trusted"` branch above them, and append the one new
  IP-hint line inside the fallback branch (don't rewrite anything already there).
- **Do not** attempt to distinguish *why* the probe failed (untrusted cert vs. unreachable host vs.
  firewall) — treat any non-success uniformly as "fall through to existing logic," matching
  today's behavior exactly for every case except the new success path. Building a more detailed
  diagnosis is a reasonable future idea, not part of this fix.
- **Do not** skip the probe when `--fetch-ca-ssh` was explicitly passed — if the connection is
  already trusted, skipping the SSH fetch too is a harmless, correct bonus (less work for the same
  outcome), not a regression to guard against.
- **Do not** touch `pharos-client`'s TLS code, `setup_pki()`, `install_server()`, or anything
  related to Issue #180/the console fix — unrelated.
- **Do not** touch the *specific wording* of the fallback message shipped by the separate
  in-flight plan — this plan only adds a new sibling branch, verify that fix has landed first and
  re-read the actual current text before writing the new branch above it.

## Verification steps (concrete)

1. **Real "already trusted" case, without needing real Let's Encrypt infrastructure**: generate a
   test CA and a server cert issued for a real-looking FQDN (e.g. `pharos-test.local`, added to
   `/etc/hosts` in the test container so it actually resolves), then **add that test CA to the
   system trust store** (e.g. `cp` into `/usr/local/share/ca-certificates/` +
   `update-ca-certificates` on Debian/Ubuntu, in a disposable Podman container so the host's real
   trust store is never touched) — this honestly simulates "a certificate this machine already
   trusts without `PHAROS_CA_CERT`," the same condition a real Let's Encrypt cert produces. Run a
   real `pharos-server` with that cert, then run the actual modified `install_pulse` probe logic
   (extracted per this repo's established testing convention) **connecting via the FQDN** and
   confirm `pulse_ca_found="trusted"`, `pulse_host_is_ip="no"`, and the new "Next Steps" line
   prints.
2. **The exact gap this plan's design review caught — connecting via bare IP to that same
   publicly-trusted-style cert**: run the identical probe against the identical server from step
   1, but this time **connect via its IP address instead of the FQDN**. Confirm the probe *fails*
   (hostname verification mismatch, even though the chain is fully trusted) — proving
   `-verify_hostname` is doing real work, not just chain-trust — and confirm it falls through to
   the existing fallback branch with `pulse_host_is_ip="yes"`, and that the new bare-IP hint line
   prints alongside the fallback text.
3. **Regression case**: run the same probe against a `pharos-server` using an *untrusted*
   self-signed cert (system trust store untouched) and confirm the probe correctly fails,
   `pulse_ca_found` still ends up `"yes"`/`"no"` exactly as today (test both: a local
   `pharos-ca.crt` present, and absent), and existing behavior (including `--fetch-ca-ssh`, if
   exercised) is completely unaffected.
4. **Unreachable-host case**: run the probe against a host/port nothing is listening on, confirm
   it fails within the `timeout 10` bound (doesn't hang) and falls through to existing logic.
5. **`ensure_openssl` still triggers correctly** on a system where `openssl` isn't yet installed —
   confirm the probe doesn't crash the installer if `openssl` is genuinely unavailable and
   `ensure_openssl`'s own install attempt also fails (matches `ensure_openssl`'s existing hard
   `error` exit in that case — this plan doesn't change that fallback behavior, just calls it from
   a new call site).
6. Clean up all test containers/certs/trust-store modifications — never touch the real host's
   system trust store during verification.

## Report back

State clearly: the exact diff (`scripts/install.sh` only), results of all 6 verification cases
above, and confirmation the separately-landed fallback-message fix's wording was preserved
unchanged (only a new sibling branch added above it). Do not commit or push — this repo requires
explicit instruction for that, every time.
