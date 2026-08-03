/* ========================================================================
 * Project: pharos
 * Component: Documentation & UX
 * File: auth-retry-status-code-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * pharos-client's automatic authenticate-and-retry flow (execute_authenticated)
 * has been completely non-functional against any Protected/Scoped-tier hub
 * since the server's "not logged in" status code was standardized from 401 to
 * 506 (an earlier, already-merged fix per this repo's own history) - the
 * client was never updated to match, so it treats the server's real response
 * as a generic fatal error instead of automatically logging in and retrying.
 * Found live-testing `mdb *` against a real Protected-tier hub.
 * * Traceability:
 * Found 2026-08-03 investigating a live "506: Authentication required for
 * Protected tier" error from mdb, which should have been transparently
 * handled by the client's own existing (but never-triggered) auth-retry logic.
 * ======================================================================== */

# Plan: fix pharos-client's dead 401 check (server has always sent 506)

## Background

`PharosClient::authenticate()` and `execute_authenticated()` (`crates/pharos-client/src/lib.rs`,
unmodified, already correct) implement a **complete, working** automatic login flow: send
`login <client_id>`, read the challenge (a `301:` response), sign it via `sign_message_async`,
send `auth "<pubkey>" "<sig>"`, confirm `200`. `execute_authenticated()` calls this automatically
whenever a command comes back as `PharosResponse::AuthenticationRequired`. This machinery is
correctly built and has never been the problem.

The bug is entirely in `parse_response()` (`crates/pharos-client/src/lib.rs:243-322`, the relevant
part quoted verbatim, re-read fresh this session):

```rust
            match code {
                200 => { ... }
                102 => { ... }
                401 => {
                    // New message format: 401:Authentication required. Use 'login [alias]' to receive a challenge.
                    return Ok(PharosResponse::AuthenticationRequired { challenge: String::new() });
                }
                c if c >= 400 => {
                    return Ok(PharosResponse::Error { code: c, message: message.to_string() });
                }
                ...
```

**Confirmed this session**: the server has never sent `401` for this case, anywhere. Every
"not logged in" response — write operations, general auth-required, Protected tier, Scoped tier —
uses `506` (`pharos-server/src/middleware.rs:177,214,223,232`, quoted verbatim):

```rust
"506:Authentication required for write operations. Use 'login [alias]' to receive a challenge.\n"
"506:Authentication required. Use 'login [alias]' to receive a challenge.\n"
"506:Authentication required for Protected tier. Use 'login [alias]' to receive a challenge.\n"
"506:Authentication required for Scoped tier. Use 'login [alias]' to receive a challenge.\n"
```

Confirmed `401` is not used *anywhere* server-side (grepped `pharos-server/src/*.rs` for `"401`,
zero matches) — it's entirely dead code client-side, left over from before the server's status
codes were standardized to RFC 2378's actual numbering (this repo's own earlier history,
Debt #29/Issue #176). The client was simply never updated to match, so every response that should
trigger transparent auto-login instead falls through to the generic `c if c >= 400` branch and
surfaces as a raw, fatal `506: Authentication required...` error to the end user — exactly what was
observed live against a real Protected-tier hub.

## The change

**File: `crates/pharos-client/src/lib.rs`, `parse_response()` only** — change the literal match arm
from `401` to `506`, and fix the now-accurate comment:

```rust
                506 => {
                    // The server's actual status code for "not logged in yet" (see
                    // pharos-server/src/middleware.rs's SecurityTierMiddleware) — triggers
                    // execute_authenticated()'s automatic login-challenge-sign-retry flow.
                    return Ok(PharosResponse::AuthenticationRequired { challenge: String::new() });
                }
```

That's the entire code change. No reordering needed — this literal-value match arm already sits
textually before the generic `c if c >= 400` guard arm, so it continues to take priority for this
exact value with no other changes.

## Non-goals (do not touch)

- **Do not** touch `authenticate()` or `execute_authenticated()` themselves — they are already
  correct; they've simply never been reachable due to this one dead status-code check.
- **Do not** add handling for `401` alongside `506` "just in case" — confirmed the server never
  sends it; keeping a dead branch around for a code that doesn't exist adds confusion, not safety.
- **Do not** change any other status code's handling (`102`, `200`, the generic `>= 400` branch,
  data lines) — unrelated, already correct.
- **Do not** add a synthetic unit test that merely asserts `506 == 506` — that would provide false
  confidence without proving the actual bug (a broken end-to-end retry flow) is fixed. The
  verification below requires proving the real flow works against a real server, not a
  trivial-equality test standing in for it.
- **Do not** touch `mdb`/`ph`'s own handling of `PharosResponse::AuthenticationRequired` — once
  `parse_response` correctly reports it, `execute_authenticated()` already intercepts it
  transparently before either CLI ever sees it for a normal query; no CLI-level change is needed.

## Verification steps (concrete — live, end-to-end, not a synthetic unit test)

This bug is specifically about real client/server wire-protocol agreement, so verification must be
a real client talking to a real server, matching this session's established live-verification
discipline:

1. Stand up a real `pharos-server` instance in `Protected` tier (`PHAROS_SECURITY_TIER=open` is
   the wrong tier for this test — must be `protected` or `scoped` to actually trigger the
   `506` path) with a real self-signed cert (same `openssl`-based pattern used repeatedly this
   session), and at least one enrolled key in its `PHAROS_KEYS_DIR` with the `admin` role (matching
   Part 1's fixed token-based role detection).
2. Using the real `PharosClient` (not a raw socket), with `PHAROS_PRIVATE_KEY` pointed at that
   enrolled key's private half, call `execute_authenticated()` with a simple command (e.g.
   `"status"` or a `query *`-equivalent) **before this fix** and confirm it currently fails with
   the raw `PharosResponse::Error { code: 506, .. }` surfaced as a fatal error — reproduce the bug
   for real first.
3. Apply the fix, rebuild, repeat the exact same live call, and confirm it now **transparently
   authenticates and succeeds** — the caller gets back a normal successful response, with no
   manual login step and no raw 506 error visible at the call site.
4. Regression check: confirm a command that should genuinely fail for an *unrelated* reason (e.g.
   a real authorization-denial code like `510`/`511`/`516` from Debt #29's status-code work, using
   a key that's authenticated but lacks the required role) is **not** swallowed by this change —
   it must still surface as a normal `PharosResponse::Error`, not be misinterpreted as
   "needs login."
5. Run the full existing test suite (`cargo test --workspace --all-features`, in Podman per this
   repo's Zero-Host policy) to confirm nothing else regresses — in particular the existing
   server-side tests that already assert on the literal `"506:Authentication required"` wire text
   (`pharos-server/tests/middleware_integration.rs`, `rbac_integration.rs`) must still pass
   unmodified, since this fix doesn't touch the server at all.
6. Clean up all test server instances/certs/keys created for this verification.

## Report back

State clearly: the exact one-line diff (plus comment), the real before/after live-test transcript
proving the bug reproduction and the fix (step 2 and step 3 above, actual command output, not
paraphrased), the regression check result (step 4), and the full test suite result. Do not commit
or push — this repo requires explicit instruction for that, every time.
