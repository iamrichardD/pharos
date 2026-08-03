/* ========================================================================
 * Project: pharos
 * Component: Documentation & UX
 * File: node-ca-fallback-oneliner-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Simplify install.sh's manual CA-trust fallback message (shown when
 * --fetch-ca-ssh wasn't used and no local CA is found) from 4 prose steps
 * with a placeholder into one copy-pasteable command using standard Linux
 * tools (scp + systemd drop-in override), and surface --fetch-ca-ssh as the
 * better alternative for next time - a DX gap flagged in the original
 * node-install UX investigation but never acted on.
 * * Traceability:
 * Follow-up to the node-install UX panel investigation (2026-08-02/03) and
 * the --fetch-ca-ssh feature (Gap 1, v1.10.0).
 * ======================================================================== */

# Plan: simplify install.sh's manual CA-trust fallback into one command

## Background

Live install output (`install.sh -- node`, no `--fetch-ca-ssh` flag) currently prints:

```
1. TLS: no local Pharos CA found. If your server is a REMOTE host, copy ITS /etc/pharos/certs/pharos-ca.crt to this machine, add 'Environment=PHAROS_CA_CERT=<path-to-copied-file>' to /etc/systemd/system/pharos-pulse.service, then run: sudo systemctl daemon-reload && sudo systemctl restart pharos-pulse
```

Two problems, both already flagged in the original investigation's DX/UX review but not
implemented then:
1. It never mentions `--fetch-ca-ssh <user@host>` (shipped in v1.10.0) as a way to avoid all of
   this — a user hitting this message today has no idea a one-flag fix exists for next time.
2. It's 3 different manual actions described in prose with a `<path-to-copied-file>` placeholder
   to fill in by hand, instead of one copy-pasteable command.

Current code (quoted verbatim, re-read fresh this session — `scripts/install.sh:511-517`):

```bash
    elif [[ "${target}" == "node" || "${target}" == "pulse" ]]; then
        if [[ "${pulse_ca_found:-no}" == "yes" ]]; then
            echo -e "1. TLS: found a local Pharos CA at ${PHAROS_DIR}/certs/pharos-ca.crt — pulse trusts it automatically."
        else
            echo -e "1. TLS: no local Pharos CA found. If ${host_override:-your server} is a REMOTE host, copy ITS ${PHAROS_DIR}/certs/pharos-ca.crt to this machine, add 'Environment=PHAROS_CA_CERT=<path-to-copied-file>' to /etc/systemd/system/pharos-pulse.service, then run: ${SUDO} systemctl daemon-reload && ${SUDO} systemctl restart pharos-pulse"
        fi
        echo -e "2. Verify the pulse agent is running: ${SUDO} systemctl status pharos-pulse"
        echo -e "3. Check logs: ${SUDO} journalctl -u pharos-pulse -f"
```

## The change

Replace the `else` branch's single `echo -e` line with:

```bash
        else
            echo -e "1. TLS: no local Pharos CA found. Next time, pass --fetch-ca-ssh <user@host> to install.sh to do this automatically."
            echo -e "   To fix this manually now, if ${host_override:-your server} is a REMOTE host, run:"
            echo -e "     scp <user@host>:${PHAROS_DIR}/certs/pharos-ca.crt /tmp/pharos-ca.crt && \\"
            echo -e "     ${SUDO} mv /tmp/pharos-ca.crt ${PHAROS_DIR}/certs/pharos-ca.crt && \\"
            echo -e "     ${SUDO} mkdir -p /etc/systemd/system/pharos-pulse.service.d && \\"
            echo -e "     printf '[Service]\\\\nEnvironment=PHAROS_CA_CERT=${PHAROS_DIR}/certs/pharos-ca.crt\\\\n' | ${SUDO} tee /etc/systemd/system/pharos-pulse.service.d/override.conf >/dev/null && \\"
            echo -e "     ${SUDO} systemctl daemon-reload && ${SUDO} systemctl restart pharos-pulse"
        fi
```

This is intentionally a **systemd drop-in override** (`.service.d/override.conf`), not a sed-edit
of the main unit file: it's the correct, idempotent way to add an `Environment=` line (rerunning
the command just overwrites one small file cleanly — no risk of duplicate `Environment=` lines
that hand-editing/appending to the shipped unit file would risk), and it matches what
`systemctl edit` itself does under the hood.

**Escaping note for whoever implements this** (verify live, don't assume): the goal is for the
*operator's terminal* to display and let them copy-paste a `printf` command containing the two
literal characters `\`+`n` (so `printf` itself interprets them as newlines when the operator runs
it) — not an actual embedded newline produced by install.sh's own `echo -e`. Since `echo -e`
already interprets `\n` as an escape, producing a literal `\n` in the output requires escaping the
backslash once for `echo -e` (`\\n` inside the double-quoted `echo -e` string) — but because this
plan's own markdown/shell quoting adds another layer, double-check the actual byte sequence that
lands in `scripts/install.sh` is `\\\\n` (so that after **that** file's own `echo -e "..."`
double-quote processing, the operator's terminal displays literal `\n`, two characters, not a
newline). Confirm this empirically (verification step 1 below) rather than trusting the escaping
math above — this is exactly the kind of thing worth being wrong about on paper.

## Non-goals (do not touch)

- **Do not** touch the `--fetch-ca-ssh`/`fetch_ca_via_ssh` implementation itself — this plan only
  changes what gets *printed* in the fallback case, not the automated path.
- **Do not** touch the `pulse_ca_found == "yes"` branch (line above the one being changed) — only
  the `else` (fallback) branch changes.
- **Do not** touch the hub/server "Next Steps" block, or anything for `target == "hub"/"server"`.
- **Do not** touch anything from the console/Issue #180 fix (already shipped, v1.10.2) or any
  earlier Gap 1/2/3 work — unrelated.
- **Do not** add a `command -v scp` runtime check inside `install.sh` — this text is advisory
  output for a human to read and run themselves, not something the script executes; if `scp`
  isn't present on the operator's system they'll get a normal "command not found" when they try
  it, same as any other documented-but-unvalidated command in this file's existing "Next Steps"
  messages.

## Verification steps (concrete)

1. **Extract and run the real code** (per this repo's established convention — never `source
   scripts/install.sh` directly): build the actual `main()`/output logic path for `target=node`,
   `pulse_ca_found=no`, a placeholder `host_override`, and capture the *actual* printed output.
   Confirm the `printf` line, as displayed, contains the literal two-character sequence `\n`
   (backslash followed by n) and not a real embedded newline — settle the escaping question
   empirically as flagged above.
2. **Copy-paste the actual captured output's command block into a real shell and run it
   end-to-end** against a disposable test setup (reuse the same disposable-Podman-sshd-container
   approach from the original `--fetch-ca-ssh` live verification, plus a real
   `pharos-pulse.service` systemd unit — either a real systemd user-session/system unit in a test
   VM/container capable of running systemd, or at minimum verify each individual command
   (`scp`, `mkdir -p .service.d`, the `printf | tee`, `daemon-reload`, `restart`) succeeds and
   produces a correct `/etc/systemd/system/pharos-pulse.service.d/override.conf` with exactly:
   ```
   [Service]
   Environment=PHAROS_CA_CERT=/etc/pharos/certs/pharos-ca.crt
   ```
   (two lines, real newline between them, no literal `\n` left in the file — confirming `printf`
   really did interpret the escape correctly this time, the reverse check from step 1).
3. **Idempotency check**: run the same one-liner a second time and confirm the drop-in file is
   simply overwritten with identical content (not duplicated, not appended) — proving the "safer
   than hand-editing the unit file" claim in the Background section is real.
4. Clean up any test units/containers/files created for verification.

## Report back

State clearly: the exact diff (`scripts/install.sh` only, the one `else` branch), the *actual*
captured output text (not the plan's draft — prove what really prints), confirmation the escaping
produces a literal `\n` in the terminal output and a real newline after the operator's `printf`
runs, and the idempotency check result. Do not commit or push — this repo requires explicit
instruction for that, every time.
