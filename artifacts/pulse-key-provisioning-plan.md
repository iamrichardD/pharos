/* ========================================================================
 * Project: pharos
 * Component: Installer / Pulse Agent
 * File: pulse-key-provisioning-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * pharos-pulse runs as `User=pharos`, a dedicated nologin system account with no
 * $HOME. sign_message_async's entire key-resolution chain (PHAROS_PRIVATE_KEY env var
 * -> ${HOME}/.ssh/id_ed25519 -> ${HOME}/.ssh/admin_id_ed25519 -> hub-local fallback)
 * resolves to nothing for that identity, so pharos-pulse has never actually been able
 * to authenticate against a Protected/Scoped-tier hub. Root-caused live against the
 * Implementation team's real hub (Issue #181 / TODO.md Debt #34).
 * * Explicit scope constraint (user instruction, verbatim): "don't go down the path of
 * using the loggin user. Just focus on the originating issue and get that resolved."
 * The User=pharos identity is NOT changed by this plan. This plan only gives that
 * existing identity a key it can actually find and use.
 * * Traceability:
 * Root-caused 2026-08-03 from the Implementation team's node install transcript
 * (private-key-not-found errors, 60s wait-then-fallback messages). Issue #181.
 * ======================================================================== */

# Plan: give pharos-pulse its own provisioned signing key

## Background (already established in this codebase — reused, not invented)

- Hub's own bootstrap key, `pharos-server/src/auth.rs`'s `auto_generate_admin_key()`
  (lines 215-254): generates an Ed25519 keypair with `PrivateKey::random(&mut rng,
  ssh_key::Algorithm::Ed25519)`, writes the private key + `chmod 0o600`, registers the
  public half, and `warn!`s that a "LIVE, unmanaged credential" was created. This plan's
  bash equivalent mirrors that precedent exactly (same algorithm, same permission
  model, same class of warning).
- `install_server()` already creates the hub's key registry directory with the exact
  convention this plan reuses (`scripts/install.sh:343-345`):
  ```bash
  ${SUDO} mkdir -p "${PHAROS_DIR}/keys"
  ${SUDO} chown pharos:pharos "${PHAROS_DIR}/keys"
  ${SUDO} chmod 700 "${PHAROS_DIR}/keys"
  ```
- `fetch_ca_via_ssh()` (`scripts/install.sh:244-279`) already proves the safe SSH
  round-trip pattern this plan reuses for enrollment: `BatchMode=yes`,
  `ConnectTimeout=10`, no auto-accept of unknown host keys, `${fetch_ca_ssh_target}`
  already validated at parse time (`^([A-Za-z0-9][A-Za-z0-9_.-]*@)?[A-Za-z0-9][A-Za-z0-9.-]*$`,
  the exact fix for the SSH argument-injection class found in this session's DevSecOps
  pass).
- Role parsing (`pharos-server/src/auth.rs::register_key`) is token-based on `-`/`_`
  splits — `admin` is a recognized exact token; there is no "pulse"/"node" role. Using
  an `admin`-token filename is the only way today to get a filename this codebase
  already treats as a working, universally-accepted (Protected and Scoped tier alike)
  identity, without inventing a new server-side role.

## The change — all in `scripts/install.sh`, `install_pulse()` only

### 1. Provision the key (idempotent)

Immediately after `ensure_system_user` in `install_pulse()`, before the CA-trust probe:

```bash
    local pulse_key_dir="${PHAROS_DIR}/keys"
    local pulse_key_path="${pulse_key_dir}/pulse_id_ed25519"
    local pulse_key_freshly_generated="no"

    ${SUDO} mkdir -p "${pulse_key_dir}"
    ${SUDO} chown pharos:pharos "${pulse_key_dir}"
    ${SUDO} chmod 700 "${pulse_key_dir}"

    if ! ${SUDO} test -f "${pulse_key_path}"; then
        command -v ssh-keygen >/dev/null 2>&1 || error "ssh-keygen is required to provision the Pulse agent's signing key but is not installed."
        ${SUDO} ssh-keygen -t ed25519 -N "" -f "${pulse_key_path}" -C "pharos-pulse@$(hostname -s)" >/dev/null
        ${SUDO} chown pharos:pharos "${pulse_key_path}" "${pulse_key_path}.pub"
        ${SUDO} chmod 600 "${pulse_key_path}"
        ${SUDO} chmod 644 "${pulse_key_path}.pub"
        pulse_key_freshly_generated="yes"
        warn "Generated a new signing key for Pharos Pulse at ${pulse_key_path} — this is a live, unmanaged credential (like the hub's own auto-generated admin key). Treat it accordingly."
    fi
```

`ssh-keygen` runs as `${SUDO}` (root) so it can write into a root/pharos-owned
directory, then ownership is fixed up on the resulting files — matches how
`setup_pki` already handles cert generation-then-chown in this same file.

### 2. Wire it into the unit explicitly

In the existing `cat <<EOF | ${SUDO} tee /etc/systemd/system/pharos-pulse.service`
heredoc, add one line:

```
Environment=PHAROS_PRIVATE_KEY=${pulse_key_path}
```

This bypasses the entire HOME-guessing resolution chain for this identity — a direct,
unambiguous reference instead of relying on env-var/HOME lookups that structurally
cannot resolve for a nologin system user. `sign_message_async` itself is untouched.

### 3. Enrollment — reuse `--fetch-ca-ssh`'s existing SSH round-trip

Only attempted when a key was **freshly generated** (never re-attempt enrollment on a
re-run against an existing key) **and** `fetch_ca_ssh_target` was provided:

```bash
    local pulse_key_enrolled="no"
    if [[ "${pulse_key_freshly_generated}" == "yes" && -n "${fetch_ca_ssh_target:-}" ]]; then
        local node_label
        node_label="$(hostname -s | tr -cd 'A-Za-z0-9-')"
        local remote_pub_path="${PHAROS_DIR}/keys/${node_label}-admin_id_ed25519.pub"
        if ${SUDO} cat "${pulse_key_path}.pub" | ssh -o BatchMode=yes -o ConnectTimeout=10 "${fetch_ca_ssh_target}" \
            "sudo tee ${remote_pub_path} >/dev/null && sudo systemctl reload pharos-server" >/dev/null 2>&1; then
            pulse_key_enrolled="yes"
            log "Enrolled Pulse's key on ${fetch_ca_ssh_target} as ${remote_pub_path} (admin-equivalent trust — see warning above)."
        else
            warn "Could not auto-enroll Pulse's key via SSH on ${fetch_ca_ssh_target} — see manual command in Next Steps below."
        fi
    fi
```

Piping the local `.pub` file content through SSH's stdin (rather than interpolating it
into the remote command string) avoids any quoting/injection concerns — the remote
command itself takes no attacker-influenced arguments.

### 4. Next Steps messaging (the `node`/`pulse` branch of `main()`)

Add a new numbered line reporting key status, and — only when not auto-enrolled — the
exact one-liner with the real public key content embedded (no `scp` step needed; public
keys aren't secret):

```bash
if [[ "${pulse_key_enrolled:-no}" != "yes" ]]; then
    echo -e "N. Signing key: generated locally at ${PHAROS_DIR}/keys/pulse_id_ed25519 but not enrolled on a hub yet. To enroll it now, run:"
    echo -e "     ${SUDO} cat ${PHAROS_DIR}/keys/pulse_id_ed25519.pub | ssh <user@host> 'sudo tee /etc/pharos/keys/'\"\$(hostname -s)\"'-admin_id_ed25519.pub >/dev/null && sudo systemctl reload pharos-server'"
fi
```

(Only shown when a key was freshly generated this run and not auto-enrolled — an
already-enrolled key from a prior run doesn't need this reminder every time.)

## Non-goals (do not touch)

- **Do not** change `User=pharos` or run pulse as the invoking/logged-in user — explicitly
  rejected by the user this session.
- **Do not** touch `pharos-client`'s key-resolution chain (`sign_message_async`) — the
  fix is entirely in how `install.sh` provisions and points at a key, not in how the
  client looks for one.
- **Do not** add a new server-side role token (e.g. "pulse") — reusing the existing
  `admin` token is the minimal, already-supported path across both Protected and
  Scoped tiers.
- **Do not** touch `mdb`/`ph` — this plan is scoped to the pulse agent only. (The
  parallel interactive-key-setup plan for `mdb`/`ph` remains separately filed and
  un-dispatched.)
- **Do not** regenerate or re-enroll an existing pulse key on a re-run/upgrade — only
  the first-ever install on a given node generates one.

## Verification steps (concrete, live)

1. **Fresh node install with `--fetch-ca-ssh`** against a disposable sshd+hub Podman
   pair (reusing this session's proven disposable-sshd pattern): confirm the key is
   generated, auto-enrolled on the hub, and — running `pharos-pulse` for real as the
   `pharos` user against that hub configured for Protected tier — its presence write
   actually succeeds (`journalctl -u pharos-pulse` shows no auth errors, hub's storage
   shows the record).
2. **Fresh node install without `--fetch-ca-ssh`**: confirm the key is generated, NOT
   enrolled, and the printed one-liner (copy-pasted verbatim against the disposable hub)
   actually enrolls it and pulse then authenticates successfully.
3. **Re-run `install_pulse` a second time** (simulating a version upgrade): confirm the
   existing key's fingerprint is unchanged (not regenerated), no duplicate enrollment
   attempted, and the regenerated systemd unit still contains the correct
   `PHAROS_PRIVATE_KEY=` line.
4. **Permissions check**: confirm `sudo -u pharos cat ${PHAROS_DIR}/keys/pulse_id_ed25519`
   succeeds (the actual identity that runs the service can read its own key) and that a
   different unprivileged user cannot.
5. Clean up all disposable containers/keys.

## Report back

State clearly: exact diff (`scripts/install.sh` only), results of all 5 verification
steps (especially #1 and #3), and confirmation `pharos-client`, `mdb`, `ph`, and
`User=pharos` were untouched. Do not commit or push — this repo requires explicit
instruction for that, every time.
