/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: auth-check-tier-bypass-fix-plan.md
 * Author: Richard D. (https://github.com/iamrichardD/pharos)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * pharos-console-web's "CLI Handshake" login (Command::AuthCheck) is
 * completely non-functional on any Protected/Scoped-tier hub - confirmed
 * on the real production hub (pharos-01.iamrichardd.com, which actually
 * runs Protected tier). Root cause and fix plan below.
 * ======================================================================== */

# Plan: fix `auth-check` being blocked by SecurityTierMiddleware

## The bug

The Implementation Team reported "Handshake verification failed" when using
`pharos-console-web`'s CLI Handshake login tab, despite signing correctly
with an already-authenticated CLI identity (`rdelgado@rdelgadoXPS15`).

Reproduced directly: generated a fresh challenge, signed it with the real
key (`mdb auth sign <challenge>`), and sent the exact wire handshake
`pharos-console-web`'s `executeAuthCheck()` uses (`id web-console-auth` then
`auth-check "<pubkey>" "<sig>" "<challenge>"`) straight to the real
production hub. Got back:

```
506:Authentication required for Protected tier. Use 'login [alias]' to receive a challenge.
```

— never reaching the actual signature-verification code at all (confirmed
via `journalctl`: the command is logged as received and parsed, but no
further log line from `AuthManager::verify_with_fingerprint` ever fires).

Root cause: `pharos-server/src/middleware.rs`'s `SecurityTierMiddleware`
gates every command except an allowlist on `context.authenticated`:

```rust
let is_auth_bypassed = matches!(command,
    Command::Status | Command::Id(_) | Command::Login(_) | Command::Auth { .. } | Command::Quit
);
```

`Command::AuthCheck` is missing from this list. Under `Protected` and
`Scoped` tier (both share this same `is_auth_bypassed` check), any
`auth-check` from a not-yet-authenticated connection - which is the *only*
way `auth-check` is ever used, since its whole purpose is to authenticate
an unauthenticated connection statelessly - gets short-circuited with 506
before it can ever succeed. The feature is structurally self-defeating on
any Protected/Scoped hub, which is presumably every real deployment that
matters (`Open` tier doesn't gate on this, which is exactly why this was
never caught: `scripts/live-verify`'s harness and every existing
integration test that exercises `auth-check ` uses `SecurityTier::Open`).

## The fix

One-line change to the allowlist in `pharos-server/src/middleware.rs`:

```rust
let is_auth_bypassed = matches!(command,
    Command::Status | Command::Id(_) | Command::Login(_) | Command::Auth { .. }
        | Command::AuthCheck { .. } | Command::Quit
);
```

This is safe to bypass the tier gate for exactly the same reason
`Command::Auth` already is: `AuthCheck`'s own handler
(`pharos-server/src/lib.rs`'s `Command::AuthCheck` arm) does full
Ed25519 signature verification against a registered key via
`AuthManager::verify()` before ever returning `200:Ok` - an invalid
signature, unregistered key, or malformed input still gets rejected
(`516:No authorization for request`) exactly as before. This change only
lets the *attempt* through to that real check; it grants no new
authenticated capability that `Command::Auth` doesn't already have via the
identical underlying `verify_with_fingerprint()` call.

## Non-goals

- Do not change `AuthManager::verify`/`verify_with_fingerprint` - already
  correct and shared correctly with `Command::Auth`.
- Do not change `Open` tier's behavior - it's unaffected either way (no
  gate on `auth-check` there today, none needed).
- Do not touch `pharos-console-web` - the client-side wire flow
  (`executeAuthCheck` in `src/lib/pharos.ts`) is already correct; this is
  purely a server-side middleware allowlist gap.
- Do not add replay protection / single-use consumption to `AuthCheck`'s
  caller-supplied challenge (unlike `Login`+`Auth`'s server-generated
  one-time challenge). Panel-reviewed (DevSecOps): this is `AuthCheck`'s
  existing, intentional design - a stateless, replayable proof-of-possession
  check, per its own doc comment ("Executes a stateless authentication
  check") - not a gap introduced or worsened by this fix. Noted here so a
  future pass doesn't "fix" it without understanding the tradeoff.

## Test plan

1. **Unit test** in `pharos-server/src/middleware.rs`'s existing
   `#[cfg(test)] mod tests` (matching the style of
   `test_should_deny_unauthenticated_write_in_rbac_middleware` etc.):
   `SecurityTierMiddleware { default_tier: SecurityTier::Protected }`,
   an unauthenticated `ClientContext`, a `Command::AuthCheck { .. }` -
   assert `pre_process` returns `MiddlewareAction::Continue`, not
   `ShortCircuit`. Repeat for `SecurityTier::Scoped`.
2. **Integration test** in `pharos-server/tests/middleware_integration.rs`
   (which already has Protected/Scoped-tier wire-level tests): spin up a
   real listener with `SecurityTier::Protected`, connect unauthenticated,
   send a real (signed, registered-key) `auth-check` over the wire, assert
   `200:Ok` - reproducing the exact reported scenario end-to-end, not just
   at the `pre_process` unit level. Repeat for `Scoped`.
3. Confirm a genuinely bad `auth-check` (wrong signature, or an
   unregistered key) still correctly returns `516`, not `200` - the fix
   must not accidentally weaken the real verification, only unblock
   reaching it.
4. `cargo test --workspace` passes (in Podman, per this repo's Zero-Host
   policy).

## Live verification (required before considering this done)

Reproduce the exact failing scenario against a disposable `Protected`-tier
server built from the fix (`scripts/live-verify/` harness, which currently
only exercises `Open` tier - note this gap for the harness's own tier
coverage separately, not in scope for this fix): sign a fresh challenge
with a real key, send the real `id` + `auth-check` wire sequence, confirm
`200:Ok`. Then confirm against the real production hub
(`pharos-01.iamrichardd.com`, real `Protected` tier) using the same direct
reproduction method already used to find this bug - the login must
actually succeed end-to-end, not just return the right response code.

## Report back

State clearly: the exact diff, all new test names and their pass/fail
result individually, `cargo test --workspace` output, and the live
verification transcript against both the disposable Protected-tier server
and the real production hub. Do not commit, push, tag, or close the GitHub
issue - this repo requires explicit instruction for each of those, every
time.
