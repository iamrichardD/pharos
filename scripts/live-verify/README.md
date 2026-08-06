# Live Verification Harness (dev/test only)

A reusable disposable topology for testing real built Pharos binaries
against each other — real TCP, real TLS, real SSH-key auth — without
hand-rolling `podman run` invocations fresh every time. Not a production
deployment template. Never used by `install.sh`. Certs and keys are
generated fresh by `setup.sh` on every run and are not committed.

## Quick start

```bash
# 1. Build the binaries you need (in Podman, per this repo's Zero-Host policy):
podman run --rm --security-opt seccomp=unconfined \
  -v "$(pwd)":/workspace:z -w /workspace \
  public.ecr.aws/docker/library/rust:latest \
  cargo build --release -p pharos-server -p pharos-pulse -p mdb -p ph

# 2. Prepare a fresh workspace (certs + admin key + empty data dir):
cd scripts/live-verify
./setup.sh

# 3. Copy the binaries you need into it:
cp ../../target/release/pharos-server ../../target/release/pharos-pulse \
   ../../target/release/mdb ../../target/release/ph \
   workspace/bin/

# 4. Bring up the long-running services:
podman-compose up -d pharos-server pharos-pulse

# 5. Give pharos-pulse a few seconds to register its baseline, then query:
podman-compose run --rm mdb hostname=pulse-test-node

# 6. Tear down:
podman-compose down
rm -rf workspace
```

## What's already handled for you

These are real mistakes made — and fixed — during manual ad-hoc live
verification before this harness existed (including while building and
verifying this harness itself). Don't reintroduce them:

- **Use `podman-compose`, not `podman compose`.** The bare `podman compose`
  subcommand on this host routes through an external `docker-compose`
  1.28.0 provider that expects a real Docker daemon socket and fails
  outright (`Error while fetching server API version`). `podman-compose`
  (a separate, actually-Podman-native tool, also installed) is what
  actually works — confirmed by running this harness end-to-end.
- **`PHAROS_CA_CERT` must point at the CA cert (`root-ca.crt`), not the
  leaf cert (`server.crt`)** — pointing it at the leaf produces a TLS
  `UnknownCA`/`InvalidData` error on both sides that looks like a cert
  problem but is actually just the wrong file.
- **TLS cert**: generated via `../gen-sandbox-certs.sh` (a real CA-signed
  leaf cert), not a hand-rolled self-signed one — a self-signed leaf trips
  `CaUsedAsEndEntity` against `mdb`/`pharos-pulse`'s strict TLS stack.
  SAN/CN is `pharos-server`, matching the compose service name, so
  clients reach it by that name over the compose network — no need for
  `--network container:<name>` tricks or fragile 127.0.0.1-only certs.
- **Every service needs `security_opt: seccomp:unconfined`**, including
  the one-shot `mdb`/`ph` services — omitting it on any of them produces
  `error adding seccomp filter rule for syscall bdflush: permission denied`.
- **Don't edit `docker-compose.yml` while the stack is up.**
  `podman-compose` detects the config-hash drift and tries to recreate
  affected containers, which can cascade into real errors (including a
  `--remove-orphans` bug in this podman-compose version). Bring the stack
  down first, edit, then bring it back up.
- **`podman-compose down` may print a harmless error** for each one-shot
  `run --rm` service (e.g. `no container with ID or name "..._mdb_1"
  found`) — those containers already self-removed after their query, so
  there's nothing there to remove. Not a real failure; the persistent
  services (`pharos-server`, `pharos-pulse`) still get cleaned up
  correctly.
- **Auth**: one pre-enrolled admin identity (`workspace/keys/admin_id_ed25519`)
  used by every service. Both `PHAROS_PRIVATE_KEY` *and* `PHAROS_PUBLIC_KEY`
  must be set for any service that writes (`add`/`change`) — the client
  needs both to sign the SSH challenge; a missing `PHAROS_PUBLIC_KEY` alone
  produces a confusing `401` that looks like an auth failure, not a config
  gap.
- **`pharos-pulse` needs `cap_add: ALL`** in this harness specifically
  because real network-interface enumeration (`if-addrs`, for
  `ip_addr`/`mac_addr` and future hardware-inventory fields) needs broader
  container capabilities than the default. This is fine for a disposable
  test container; it is *not* a statement about what `pharos-pulse` needs
  in production (it doesn't run containerized there, and doesn't need this
  in a normal systemd deployment).
- **`mdb`/`ph` are one-shot** (`podman-compose run --rm mdb <query>`), not
  services `up` starts by default — they exit after one query.

## What this harness *cannot* verify

Containers don't have an independent, permission-settable DMI/sysfs
subsystem — it's either absent or read-only bind-mounted from the host.
Anything that depends on real host filesystem permissions (e.g. the
`/sys/class/dmi/id/*` read-permission fix in
`artifacts/dmi-hardware-fields-fix-plan.md`) genuinely needs a real
bare-metal or VM host, not this harness. Don't claim a permission-model fix
is "live-verified" from a run against this compose file alone.
