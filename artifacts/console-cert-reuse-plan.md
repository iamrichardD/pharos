/* ========================================================================
 * Project: pharos
 * Component: Documentation & UX
 * File: console-cert-reuse-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Implementation plan for Issue #180 / Debt #33: the Web Console container
 * Quick Start generates its own throwaway self-signed cert instead of
 * reusing existing TLS material. Revised from this plan's first draft after
 * a design review question surfaced a real gap: the originally-planned fix
 * (consume the separate pharos-web.crt) doesn't work for operators using a
 * real Let's Encrypt certificate, because nothing ever refreshes
 * pharos-web.crt from an external cert-sync process. This revision instead
 * points the console at pharos-server.crt/.key directly - the one cert file
 * any Let's Encrypt/cert-sync automation already keeps current.
 * * Traceability:
 * Fixes Issue #180 (Debt #33), filed from the node-install UX panel
 * investigation (2026-08-02/03), reported by the Implementation team while
 * standing up pharos-01. Revised 2026-08-03 after review caught the
 * Let's-Encrypt gap in the first draft.
 * ======================================================================== */

# Plan: reuse pharos-server's own cert for the console (Issue #180, revised)

## Background / decision (already made — do not re-litigate)

Issue #180 offered two options: (a) point the console docs at the separate `pharos-web.crt`
`install.sh` primes, or (b) stop priming a cert nothing consumes. **The first draft of this plan
chose (a); that draft was wrong and has been discarded.** Here's why, and what's chosen instead:

- `pharos-web.crt`/`.key` (`setup_pki "pharos-web" "pharos-web"`, called from
  `install_web_console()`) is a **separate cert identity** from `pharos-server.crt`/`.key`.
  `setup_pki` is idempotent — once a cert file exists, it's never regenerated
  (`scripts/install.sh`'s existing "Existing certificate found... Skipping generation" path).
  Nothing in this repo, and no reason for any operator's *external* cert-sync automation
  (referenced in this session's earlier hub-install report, which replaces
  `pharos-server.crt`/`.key` and `pharos-ca.crt` together with real Let's Encrypt material), would
  ever have cause to also replace `pharos-web.crt` — until now, nothing consumed it.
- Consuming `pharos-web.crt` (the first draft's plan) would only help operators using the local
  self-signed Pharos CA. Operators using a real Let's Encrypt cert for their hub would have the
  console permanently stuck on a self-signed cert with no path to ever pick up the real one — not
  fixed, just not made worse.
- **Chosen instead**: point the console at `pharos-server.crt`/`pharos-server.key` directly — the
  exact file `PHAROS_TLS_CERT`/`PHAROS_TLS_KEY` already point `pharos-server` itself at, which
  `pharos-server`'s own SIGHUP-reload logic (Debt #19/Issue #166, built specifically for
  "integrators using an externally-managed renewing certificate") already keeps current, and which
  is already whatever an operator's cert-sync process replaces. This works transparently for both
  the local-CA case (console gets a cert signed by the same local CA as the server) and the real
  Let's Encrypt case (console gets the real cert, automatically, with zero new sync path needed) —
  with no new infrastructure, because it reuses infrastructure that already exists and is already
  exercised for `pharos-server` itself.
- Consequence: `pharos-web.crt`/`.key` priming becomes entirely redundant once nothing points at
  it — remove it from `install_web_console()` rather than leave an orphan behind a second time.

Current code (quoted verbatim, re-read fresh this session):

`scripts/install.sh` (`install_web_console`, unchanged since original investigation):
```bash
install_web_console() {
    ensure_system_user
    log "Installing Pharos Web Console..."
    setup_pki "pharos-web" "pharos-web"

    warn "Pharos Web Console ships as a container image (ghcr.io/<owner>/pharos-console-web) — see the Server Setup docs' container Quick Start to run it. Native binary/systemd installation is not available yet."
}
```

`scripts/install.sh`'s `main()`, `hub` case (confirms ordering — `install_server` always runs
before `install_web_console`, so `pharos-server.crt`/`.key` are guaranteed to already exist by the
time the console step runs):
```bash
        hub)
            log "Installing Pharos Hub (Server + Console + Scan)..."
            install_server "${host_override}"
            install_web_console
            download_binary "pharos-scan"
            ;;
```

`website/src/content/docs/console.mdx` (lines ~68-84, the block being replaced):
```markdown
### Running the Console
The Web Console ships as its own container image, separate from `pharos-server`. It also requires its own `PHAROS_TLS_CERT`/`PHAROS_TLS_KEY` (mandatory HTTPS — it refuses to start without them) and needs `PHAROS_HOST` pointed at your running `pharos-server`, since the container can't resolve it otherwise:

```bash
mkdir -p ./pharos-web-certs
openssl req -x509 -newkey rsa:2048 -nodes -days 365 \
  -keyout ./pharos-web-certs/console.key -out ./pharos-web-certs/console.crt \
  -subj "/CN=pharos-web" -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"
chmod 600 ./pharos-web-certs/console.key

podman run -d --name pharos-web \
  -p 3000:3000 \
  -v ./pharos-web-certs:/certs:Z \
  -e PHAROS_TLS_CERT=/certs/console.crt \
  -e PHAROS_TLS_KEY=/certs/console.key \
  -e PHAROS_HOST=<your-pharos-server-ip-or-hostname> \
  ghcr.io/iamrichardd/pharos-console-web:latest
```

Access the dashboard at `https://<your-server-ip>:3000`. The [Automated Installation Guide](./install)'s `hub` target prepares TLS material for the console but does not start this container for you — run it as a separate step. Didn't come up? Check `podman logs pharos-web`.
```

## The change

**1. `scripts/install.sh` — remove the now-pointless cert priming from `install_web_console()`:**

```bash
install_web_console() {
    ensure_system_user
    log "Installing Pharos Web Console..."

    warn "Pharos Web Console ships as a container image (ghcr.io/<owner>/pharos-console-web) — see the Server Setup docs' container Quick Start to run it. It reuses this host's existing pharos-server TLS certificate. Native binary/systemd installation is not available yet."
}
```
(Only the `setup_pki "pharos-web" "pharos-web"` line is removed; the `warn` message gets one added
sentence — "It reuses this host's existing pharos-server TLS certificate." — so an operator reading
install output understands there's no separate console cert to look for.)

**2. `website/src/content/docs/console.mdx` — replace the self-signed-cert block:**

```markdown
### Running the Console
The Web Console ships as its own container image, separate from `pharos-server`. It also requires its own `PHAROS_TLS_CERT`/`PHAROS_TLS_KEY` (mandatory HTTPS — it refuses to start without them) and needs `PHAROS_HOST` pointed at your running `pharos-server`, since the container can't resolve it otherwise. Reuse the hub's own TLS certificate at `/etc/pharos/certs/pharos-server.crt`/`.key` instead of generating a new one — this is the same cert file `pharos-server` itself uses, so if you're managing a real certificate (e.g. Let's Encrypt via an external renewal process), the console picks it up automatically with no separate sync path to maintain:

```bash
sudo podman run -d --name pharos-web \
  -p 3000:3000 \
  -v /etc/pharos/certs:/certs:ro,Z \
  -e PHAROS_TLS_CERT=/certs/pharos-server.crt \
  -e PHAROS_TLS_KEY=/certs/pharos-server.key \
  -e PHAROS_HOST=<your-pharos-server-ip-or-hostname> \
  ghcr.io/iamrichardd/pharos-console-web:latest
```

Access the dashboard at `https://<your-server-ip>:3000`. Run with `sudo` (or as a user in the `pharos` group) — `pharos-server.key` is only readable by `root`/`pharos` group. If your certificate renews (Let's Encrypt or otherwise), restart this container (`podman restart pharos-web`) to pick up the new file — `pharos-server` itself reloads on `SIGHUP` (see the Server Setup docs), but the console container does not currently support a restart-free reload. Didn't come up? Check `podman logs pharos-web`.
```

Notes on the specific edits, so the builder doesn't improvise beyond them:
- Drop the `mkdir -p ./pharos-web-certs` / `openssl req` / `chmod 600 ./pharos-web-certs/console.key`
  block entirely.
- Mount `/etc/pharos/certs` (not a relative path), read-only (`:ro`) — the console only reads.
- Env vars point at `pharos-server.crt`/`pharos-server.key` (not `console.crt`/`console.key`, not
  `pharos-web.crt`/`.key`).
- `sudo` on the `podman run` line, plus the explanatory sentence — this is a real new requirement
  from mounting a `root:pharos`-owned directory, not cosmetic; don't drop it.
- Add the "restart to pick up a renewed cert" sentence — this is an honest disclosure that the
  console container itself doesn't currently support the same SIGHUP-reload path
  `pharos-server` has; do not imply it does. (A restart-free reload for the console container is a
  reasonable future issue, out of scope here — see Non-goals.)

## Non-goals (do not touch)

- **Do not** add SIGHUP (or any other restart-free) reload support to the `pharos-console-web`
  container itself — real feature work, a separate issue if wanted, not part of this fix. The doc
  change above explicitly says "restart the container," which is accurate for today's actual
  behavior.
- **Do not** touch `pharos-server`'s own cert generation, its SIGHUP-reload logic, or
  `install_server()` — unrelated, already correct.
- **Do not** touch `pharos-console-web/Containerfile` or its source — no code change needed there;
  it already accepts arbitrary bind-mounted cert/key paths via env vars.
- **Do not** add a `USER`/non-root directive to the Containerfile — separate hardening question,
  out of scope.
- **Do not** touch any of the other fixes already shipped this session (Gap 1/2/3, hardening,
  v1.10.0/v1.10.1) — unrelated files.

## Verification steps (concrete)

1. **Real install flow, real cert reuse**: run `install_server` and `install_web_console` (via the
   existing repo testing convention — extract with `sed -n '/^funcname()/,/^}/p'` into a harness
   with stubbed `SUDO=""`/`PHAROS_DIR=<temp dir>`; real `openssl` calls) in that order (matching
   the real `hub` case's ordering) and confirm: (a) `install_web_console` no longer creates any
   `pharos-web.*` files at all, and (b) `pharos-server.crt`/`.key` already exist from the
   `install_server` step before `install_web_console` runs — i.e., there's nothing for the console
   step to wait on or generate.
2. **Real container run against the real server cert**: run the actual
   `ghcr.io/iamrichardd/pharos-console-web:latest` container with the exact new `podman run`
   command from this plan (pointed at the temp dir from step 1 instead of the real
   `/etc/pharos/certs`, to avoid touching real system paths), and confirm:
   - The container starts and stays up (`podman logs pharos-web` shows no TLS-related startup
     failure).
   - `curl -vk https://127.0.0.1:3000/` completes a real TLS handshake presenting the exact
     `pharos-server.crt` from step 1 (confirm via fingerprint match, not just "no error").
3. **Confirm the `sudo`/group-membership note is necessary, not decorative**: attempt the same
   `podman run` *without* `sudo`, from a user not in the `pharos` group, with the key at its real
   `600`/`pharos:pharos` ownership, and confirm it actually fails to read the key — then confirm it
   succeeds with `sudo`.
4. **Confirm the "no restart-free reload" disclosure is accurate, not just assumed**: with the
   container running, replace the mounted cert/key files with a *different* self-signed cert (same
   CN, different keypair) without restarting the container, and confirm the console continues
   serving the *old* cert (proving a restart really is required) — then `podman restart pharos-web`
   and confirm it now serves the new one.
5. Clean up: stop/remove the test container, remove all temp cert/key material generated for this
   verification. Don't leave test containers running or generated test certs behind.

## Report back

State clearly: the exact diff (`scripts/install.sh` and `website/src/content/docs/console.mdx`
only), confirmation `install_web_console` no longer creates any `pharos-web.*` files, the real
container-run verification results (cert fingerprint match, sudo-required proof, restart-required
proof), and explicit confirmation no other file was touched. Do not commit or push — this repo
requires explicit instruction for that, every time.
