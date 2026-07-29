# Sub-spec A — OAuth Broker (managed relay)

> **Status:** DEFERRED — no implementation planned (2026-07-29). Sub-spec (level-1) of the (deferred) managed-relay architecture. File relocated to `docs/designs/deferred/`.
> **Date:** 2026-07-29
> **Parent:** [`2026-07-29-managed-relay-architecture.md`](2026-07-29-managed-relay-architecture.md) §6, §7.6, §11, §12.
> **Owns:** The `auth.oxios.com` Worker, the device-code flow, JWT issuance, D1 schema for users / devices / audit.

---

## 1. Scope

A standalone Cloudflare Worker that:

1. Brokers GitHub and Google OAuth.
2. Implements OAuth 2.0 Device Authorization Grant (RFC 8628) so the host binary can link a device to a human.
3. Issues and rotates `device_token` (long-lived JWT) and OAuth `access_token` / `refresh_token` for the browser SPA.
4. Stores user/device/audit metadata in D1.
5. Exposes a small REST surface for device revocation, the relay's `device_token` validation, and the host's device-code polling.

This sub-spec does **not** own the relay server, the E2E crypto, or the SPA. It is the foundation that A-unblocks B, C, D.

## 2. What this doc is NOT

- **Not a UX spec** for the device-code verification page. That page is part of the SPA at `app.oxios.com`; sub-spec D owns the visual treatment. The Worker only emits the data (user_code, verification URL, expires_in) and the SPA renders it.
- **Not a Cloudflare infra runbook.** `wrangler.toml` shape, secrets storage, region choice — owned by sub-spec C's deployment plan.
- **Not a security review.** All privacy/audit guarantees live in the architecture doc §3 / §8. This doc operationalizes them.

## 3. Dependencies

- **Cloudflare Workers** (Workers Paid plan; see architecture §12 Q6).
- **D1** (SQLite, single-region at first).
- **KV** (optional; for OAuth `state` nonces if we need server-side state. Default: stateless with signed `state` cookie).
- **Web Crypto** (built into Workers; for JWT sign/verify, random nonces).
- **No npm packages** for crypto. Use Workers' built-in `subtle.sign`/`subtle.verify` with EdDSA (Ed25519). JWT is hand-rolled, not from a library, to keep the bundle small and the threat surface auditable. (Trade-off noted in §10.)

## 4. Endpoints

All endpoints respond JSON. CORS: allow `https://app.oxios.com` only. Auth: see per-endpoint.

### 4.1 `GET /v1/oauth/:provider/start`

Where `:provider` is `github` or `google`.

**Auth:** none (initiates a flow).
**Query:** none.
**Behavior:**
1. Generate a 32-byte random `state`, base64url.
2. Build the provider's authorize URL with the configured `client_id`, `redirect_uri=https://auth.oxios.com/v1/oauth/:provider/callback`, `state`, `scope` per §6.1.
3. Set a `Set-Cookie: oauth_state=<state>; HttpOnly; Secure; SameSite=Lax; Path=/v1/oauth/; Max-Age=600`.
4. Respond 302 to the authorize URL.

### 4.2 `GET /v1/oauth/:provider/callback`

**Auth:** cookie `oauth_state` (CSRF defense).
**Query:** `code`, `state` (required).
**Behavior:**
1. If `state` ≠ cookie `oauth_state`, 400.
2. Exchange `code` at the provider's token endpoint.
3. Fetch the user's profile (GitHub `/user` + `/user/emails`; Google `userinfo`).
4. Extract `provider`, `provider_sub`, `primary_email` (see §6.1 for email policy).
5. Upsert into `users` (see §5). Get the internal `user_id`.
6. Mint an OAuth `access_token` (1h) and `refresh_token` (30d, rotated on use) — Worker-signed JWTs, see §6.2.
7. Issue a 302 to `https://app.oxios.com/oauth/done#access_token=...&refresh_token=...&user_id=...&expires_in=3600`. (Fragment, not query — never sent to server.)
8. Audit log: row `{ event: 'oauth_login', user_id, provider, source_ip_hash }`.

### 4.3 `POST /v1/device/code`

Initiates device linking. Called by the host binary's CLI wizard (sub-spec B).

**Auth:** none (the device doesn't have a token yet — that's the whole point).
**Body:** `{ "client_version": "oxios 0.x.y", "device_public_key": "<hex 64 chars>" }`.
**Behavior:**
1. Validate `device_public_key` is a 32-byte hex string. Reject otherwise.
2. Check that `device_public_key` is not already linked to a different user (it can re-link to the same user — that's a re-link flow). If linked to a different user, return 409 Conflict.
3. Generate a `user_code` (RFC 8628 §5.1.1 — 8 chars, Crockford base32, easy to read). Generate a `device_code` (32 bytes hex, opaque to the user; this is what the host polls with). Generate a `verification_url` = `https://app.oxios.com/device?code=<user_code>`.
4. Persist `{ device_code, user_code, device_public_key, status: 'pending', expires_at, attempts: 0, poll_interval_secs: 5 }` in a new row of `device_codes` (see §5).
5. Audit log: `{ event: 'device_code_issued', source_ip_hash, device_public_key_prefix }` (no full key, just first 8 hex chars for traceability).
6. Return `{ user_code, verification_url, device_code, expires_in: 600, poll_interval_secs: 5 }`.

### 4.4 `POST /v1/device/token`

The host polls this until the user confirms on `verification_url`.

**Auth:** none (carries `device_code` in body).
**Body:** `{ "device_code": "...", "client_version": "oxios 0.x.y" }`.
**Behavior:**
1. Look up `device_code`. If not found, 400. If `status != 'pending'`, 400 with `error: 'expired_token'`.
2. If `expires_at < now`, mark `status = 'expired'`, return 400 `error: 'expired_token'`.
3. Increment `attempts`. If `attempts > 100` (rate limit on polling), return 400 `error: 'slow_down'`.
4. If `status == 'pending'`, return 400 `error: 'authorization_pending'`.
5. If `status == 'confirmed'`:
   1. Look up the `devices` row by `device_public_key`. If absent, **INSERT** a new row with `device_id = device_public_key`, `user_id = <from device_codes.user_id>`, `hostname = NULL`, `created_at = now`, `last_seen_at = NULL`, `revoked_at = NULL`. (The row did not exist before — the device_codes row tracks the in-flight link, the devices row tracks the active link.)
   2. Mint a `device_token` (90d JWT; see §6.2 for the no-per-request-rotation policy).
   3. Update the `device_codes` row: `status = 'consumed'`, `consumed_at = now`.
   4. Audit log: `{ event: 'device_token_issued', user_id, device_public_key_prefix }`.
   5. Return `{ device_token, user_id, expires_in: 7776000 }`.

### 4.5 `POST /v1/device/confirm`

Called by the SPA when the user types the `user_code` on the verification page and clicks "Authorize device". The user is already OAuth-authenticated to the Worker at this point (cookie session — see §6.3).

**Auth:** OAuth session (cookie).
**Body:** `{ "user_code": "ABCD-1234" }`.
**Behavior:**
1. Look up `user_code`. If not found, 404. If `status != 'pending'`, 409. If `expires_at < now`, 410.
2. Set `status = 'confirmed'`, `user_id = <session user_id>`, `confirmed_at = now`. (Do not touch the `devices` table here — that happens on the next host poll via §4.4 step 5.)
3. Audit log: `{ event: 'device_confirmed', user_id, device_public_key_prefix }`.
4. Return 200.

### 4.6 `POST /v1/devices/revoke`

The user revokes a device from the SPA's device-list page.

**Auth:** OAuth session.
**Body:** `{ "device_id": "<hex>" }`.
**Behavior:**
1. Verify the `device_id` belongs to the session `user_id`. Otherwise 404 (don't leak existence).
2. Update the `devices` row: `revoked_at = now`. (Soft delete, not hard delete, so the `audit_log` rows referencing this device_id remain coherent.) The relay's per-user Durable Object is notified separately by sub-spec C — out of scope here.
3. Audit log: `{ event: 'device_revoked', user_id, device_id }`.
4. Return 200.

### 4.7 `POST /v1/relay/validate`

Called by the relay server (sub-spec C) on every `device_hello` and `browser_hello`. This is the hot path.

**Auth:** shared secret (`RELAY_AUTH_SECRET`, configured in Worker secrets) in `Authorization: Bearer`.
**Body:** `{ "token": "<device_token or access_token>", "kind": "device" | "browser" }`.
**Behavior:**
1. Verify the JWT's **EdDSA (Ed25519) signature** using the Worker's public key (the same key that signed it in §6.2). Sub-spec C will cache validation results, but the cache invalidates on every key rotation or revocation.
2. Check `exp`, `aud` (`aud: "device"` or `aud: "browser"`), `sub` (user_id).
3. For `kind: "device"`: also look up `devices` table for the `device_id` (from the JWT `device_id` claim), return `{ valid: true, user_id, device_id }` if the row exists and `revoked_at IS NULL`.
4. For `kind: "browser"`: just verify the JWT itself (revocation of a browser session = revoke the `refresh_token` chain — see §6.4).
5. Return `{ valid: true, user_id, ... }` or `{ valid: false, reason: "expired"|"revoked"|"bad_signature" }`. Do not distinguish "expired" from "revoked" externally — both look the same to a non-trusted caller.

## 5. D1 schema

```sql
CREATE TABLE users (
  user_id         TEXT PRIMARY KEY,             -- "github:12345" or "google:67890"
  provider        TEXT NOT NULL,                -- "github" | "google"
  provider_sub    TEXT NOT NULL,
  primary_email   TEXT NOT NULL,
  created_at      INTEGER NOT NULL,             -- unix seconds
  last_login_at   INTEGER NOT NULL,
  UNIQUE (provider, provider_sub)
);

CREATE TABLE devices (
  device_id       TEXT PRIMARY KEY,             -- hex of device public key
  user_id         TEXT NOT NULL,                -- FK to users
  hostname        TEXT,
  last_seen_at    INTEGER,                      -- updated by relay on hello
  created_at      INTEGER NOT NULL,
  revoked_at      INTEGER,
  FOREIGN KEY (user_id) REFERENCES users(user_id)
);

CREATE INDEX idx_devices_user ON devices(user_id);

CREATE TABLE device_codes (
  device_code        TEXT PRIMARY KEY,
  user_code          TEXT NOT NULL UNIQUE,
  device_public_key  TEXT NOT NULL,
  status             TEXT NOT NULL,             -- 'pending' | 'confirmed' | 'expired' | 'consumed'
  user_id            TEXT,                      -- NULL until confirmed
  created_at         INTEGER NOT NULL,
  expires_at         INTEGER NOT NULL,
  confirmed_at       INTEGER,
  consumed_at        INTEGER,
  attempts           INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_device_codes_user_code ON device_codes(user_code);

CREATE TABLE audit_log (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  ts              INTEGER NOT NULL,
  user_id         TEXT,
  device_id       TEXT,
  event           TEXT NOT NULL,                -- 'oauth_login', 'device_code_issued', etc.
  source_ip_hash  TEXT NOT NULL,                -- SHA-256(ip + daily_salt), 16-byte prefix
  metadata        TEXT                         -- JSON, metadata only — never payload
);

CREATE INDEX idx_audit_user_ts ON audit_log(user_id, ts);
CREATE INDEX idx_audit_ts ON audit_log(ts);
```

**No payload, no message bodies, no LLM keys are ever stored.** The D1 footprint is metadata-only.

## 6. Crypto & session design

### 6.1 OAuth provider scope

- **GitHub**: scope `read:user user:email`. We need the user's primary email; the rest of the profile is unused.
- **Google**: scope `openid email profile`.

**Email policy (per the brainstorming decision, 2026-07-29):** the primary email returned by the provider is accepted as the user identifier, regardless of `email_verified`. Rationale: GitHub users without a verified email are a small minority; a hard block would prevent legitimate use. We do not gate `device_token` issuance on email verification. (Future: add an "email unverified" banner in the SPA; not in this sub-spec.)

### 6.2 JWT signing

**All JWTs in this sub-spec are signed with EdDSA (Ed25519)** using the Worker's Ed25519 key. EdDSA is asymmetric: the Worker signs with the private key, the relay and any verifier use the corresponding public key. There is no HMAC anywhere.

- **Worker key (Ed25519):** signs all JWTs. The private half is stored in Worker secrets; the public half is published in a `/.well-known/jwks.json` endpoint so any verifier (the relay in particular) can fetch and validate.
- **Key rotation:** quarterly. During a 7-day grace window, both `kid`s are accepted.

JWT payload shape:

```json
{
  "iss": "auth.oxios.com",
  "aud": "device" | "browser",
  "sub": "<user_id>",
  "device_id": "<hex>",            // present only when aud=device
  "iat": 1700000000,
  "exp": 1700003600,
  "jti": "<random 16 bytes hex>"  // for revocation lookups
}
```

`access_token` (browser): `aud: "browser"`, exp 1h.
`refresh_token` (browser): `aud: "browser"`, exp 30d, **rotated on every use** (the SPA's auth store must persist the latest).
`device_token` (host): `aud: "device"`, exp 90d. **First cut: no per-request rotation.** The host re-validates with the relay at most once per minute; the relay caches the Worker's `validate` result for at most 5s. A follow-up sub-spec will add per-request rotation via a new Worker endpoint `POST /v1/relay/refresh` (see §11 open question 1).

The relay never holds the Worker's signing key. The relay fetches the public key from `/.well-known/jwks.json` and verifies signatures itself; it only calls `POST /v1/relay/validate` for revocation checks (which require D1 access the relay doesn't have).

### 6.3 Browser session

The SPA's `auth.ts` store holds `access_token` in sessionStorage (per architecture §11 of the level-0 doc) and `refresh_token` in localStorage with a 30-day TTL. There is no server-side session — the Worker is stateless. CSRF defense: the `oauth_state` cookie is the only stateful piece and is scoped to `/v1/oauth/`.

### 6.4 Token revocation

- **Access token (browser)**: expires in 1h, no explicit revocation. To "log out everywhere", the SPA clears local/session storage; the access token is then useless. (A leaked access_token is valid for at most 1h.)
- **Refresh token (browser)**: stored in a `refresh_tokens` table on issuance; deleted on rotation; can be deleted by `/v1/devices/revoke?kind=browser` to force re-login. (Out of scope for this sub-spec — added when the SPA requests it.)
- **Device token (host)**: on `/v1/devices/revoke`, the `devices.revoked_at` is set. §4.7's `validate` endpoint checks this on every relay-validate. Latency: revocation propagates within the relay's cache TTL (≤5s, owned by sub-spec C).

## 7. Rate limits

Implemented in the Worker, before any DB read.

| Endpoint | Limit | Window | Action |
|---|---|---|---|
| `POST /v1/oauth/:provider/start` | 10 | 1 min per IP | 429 |
| `POST /v1/oauth/:provider/callback` | 10 | 1 min per IP | 429 |
| `POST /v1/device/code` | 5 | 1 hour per IP | 429 |
| `POST /v1/device/token` | 12 | 1 min per `device_code` | 400 `error: "slow_down"` (handled in §4.4) |
| `POST /v1/device/confirm` | 10 | 1 min per user | 429 |
| `POST /v1/devices/revoke` | 30 | 1 min per user | 429 |
| `POST /v1/relay/validate` | 600 | 1 min per source IP | 429 (matches existing `security.rate_limit_per_minute`) |

## 8. Audit log discipline

The `audit_log` table is the only place that records who did what when. The Worker writes one row per `event` per §4. Per the level-0 architecture §8:

- Audit row content is metadata only. **No payload, no LLM keys, no chat content.**
- `source_ip_hash = SHA-256(ip + daily_salt)[0..32]` where `daily_salt` rotates every 24h. We do not retain raw IPs.

## 9. File structure (this sub-spec will create)

- `workers/auth/` — the Worker (TypeScript via the Workers runtime, default for speed of iteration).
- `workers/auth/src/handlers/oauth.ts` — §4.1, §4.2
- `workers/auth/src/handlers/device.ts` — §4.3, §4.4, §4.5
- `workers/auth/src/handlers/revoke.ts` — §4.6
- `workers/auth/src/handlers/relay-validate.ts` — §4.7
- `workers/auth/src/handlers/jwks.ts` — `/.well-known/jwks.json`
- `workers/auth/src/db/schema.sql` — §5
- `workers/auth/src/db/migrations/` — D1 migration runner
- `workers/auth/src/crypto/jwt.ts` — §6.2 (no external libs; Web Crypto only)
- `workers/auth/src/crypto/state.ts` — `state` generation/validation
- `workers/auth/src/audit.ts` — §8
- `workers/auth/src/rate-limit.ts` — §7
- `workers/auth/wrangler.toml` — Worker config (D1 binding, secrets, env)
- `workers/auth/test/` — Vitest tests against `miniflare` for local runs

## 10. Trade-offs (open during planning, not blockers)

- **No external crypto libs.** Workers' Web Crypto is sufficient. We give up some ergonomics (e.g., compact base64url) but gain a tiny bundle and an auditable threat surface.
- **TypeScript vs Rust via workers-rs.** TypeScript is faster to iterate; Rust is faster at runtime. For an auth-broker hot path that's mostly DB lookups, TypeScript is fine. Revisit if the validate endpoint becomes a bottleneck.
- **No server-side session.** The OAuth `state` cookie is the only stateful thing. A truly stateless design is appealing but means we can't invalidate an access token mid-flight — the 1h lifetime is the cap.
- **Soft delete on device revoke** (`revoked_at`) instead of hard delete. The `audit_log` references `device_id` and we want those references to remain coherent.

## 11. Open questions for this sub-spec

1. **Device token rotation on the relay hot path.** Two options:
   - (a) Host re-validates `device_token` once per hour at most; no per-request rotation. Simpler, less load on Worker.
   - (b) Per-request rotation: the relay's `validate` call mints a new `device_token` and returns it; the host updates its local cache. Higher security, higher load.
   - **Recommendation:** (a) for the first cut, with a follow-up sub-spec to migrate to (b) once we have telemetry.
2. **GitHub App vs GitHub OAuth App.** OAuth App is simpler but deprecated for new apps; GitHub App is the modern path. Sub-spec A's plan phase must choose. Default: GitHub OAuth App (simpler scopes, no installation flow).
3. **What happens if the user revokes the OAuth app on GitHub?** The Worker's stored `provider_sub` becomes orphaned. Out of scope: detect on next login, log the user out, require re-auth.

## 12. Acceptance criteria

- A `curl` flow that:
  1. `POST /v1/device/code` returns a `user_code`, `device_code`, `verification_url`.
  2. `POST /v1/device/token` returns 400 `error: "authorization_pending"`.
  3. After a manual `POST /v1/device/confirm` (simulating the SPA), `POST /v1/device/token` returns a valid `device_token`.
  4. `POST /v1/relay/validate` with the token returns `{ valid: true, user_id, device_id }`.
  5. `POST /v1/devices/revoke` invalidates the token; subsequent `validate` returns `{ valid: false, reason: "revoked" }`.
- 100% of the level-0 architecture's §8 privacy ceiling items are mechanically enforced: a unit test asserts the `audit_log` table has no `body` / `payload` / `message` columns.
- The Worker's `validate` endpoint sustains 600 req/min/IP for 5 minutes without errors (load test in the plan phase).
- An E2E test that links a real GitHub account, returns a `device_token`, and uses it to call the relay's `device_hello` succeeds (this requires sub-spec C to be at least stubbed — the test is split: A-only and A+C-integration).
- `/.well-known/jwks.json` returns the Worker's public Ed25519 key in JWKS format, with a `kid` matching the most recent signing key.
- A unit test asserts that the Worker's JWT library signs with EdDSA (Ed25519), not HMAC, by inspecting the JWT header's `alg` field.
