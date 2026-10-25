# Pennel Cloud v1 — deel 1: server en loginpagina — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `pennel-server` gets accounts with device-code + magic-link login, an end-to-end-encrypted object store for meeting sync, and an OpenAI-compatible proxy to Mistral; `pennel.app/device` lets a user approve a login.

**Architecture:** Axum 0.8 + Postgres 18 via sqlx (runtime queries, embedded migrations). All state lives in Postgres except per-account concurrency and rate-limit buckets (in memory; one server replica). Tokens are opaque random strings stored as SHA-256. The web page is a Next.js client component that talks to the API through same-origin route handlers, so the API needs no CORS.

**Tech Stack:** Rust 2024, axum 0.8, sqlx 0.8 (postgres, runtime-tokio, tls-rustls, migrate, chrono, uuid), reqwest (rustls-tls, stream, multipart), lettre (tokio1-rustls-tls, smtp-transport, builder), sha2, rand, base64, uuid v7, chrono; Next 16, React 19, vitest.

**Repo:** `/Users/ignace.mella/.t3/projects/pennel-cloud`, branch `feat/pennel-cloud-v1` from `main`. Leave the user's uncommitted `docs/infrastructure.md` edit alone (stash or commit around it only with the user's OK).

**Spec:** `musli/docs/superpowers/specs/2026-10-25-meeting-sync-design.md` (§3.1, §3.2 server side, §3.4, §3.6, §7).

## Global Constraints

- Logs hold method, route template, status, duration, byte length and account id only. Never e-mail addresses, object ids, tokens, request or response bodies.
- No audio or text is stored by the proxy; only counters (`audio_seconds`, `input_tokens`, `output_tokens`).
- User-facing error messages are Dutch, JSON `{"error": "<code>", "message": "<Dutch text>"}`.
- Limits (beta, one function `entitlements()`): storage 1 GB (1_073_741_824 bytes) live ciphertext, object ≤ 5 MB (5_242_880 bytes), 10 devices, 20 h audio (72_000 s) and 2_000_000 tokens (input+output) per calendar month (UTC).
- Proxy: request body ≤ 25 MB (26_214_400 bytes), ≤ 2 concurrent proxy requests per account, model must be `pennel-auto` or absent.
- Tokens: access 15 min, refresh rotated on every use, device code 15 min, magic link 15 min single-use, tombstones purged after 90 days.
- TLS through rustls only (the slim Docker builder has no OpenSSL). No compile-time `sqlx::query!` macros (no DB at build).
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` must pass; web: `npm run lint && npm run typecheck && npm test && npm run build`.
- Site copy never claims "encrypted at rest" (marketing spec D14); it may say "end-to-end versleuteld".

## Review Focus

1. A refresh token used twice (stolen copy or a client retry after a lost response) → the device session is revoked, not silently forked. Test in Task 3.
2. Two devices upload the same object from the same base version → exactly one succeeds, the other gets 409 with the current version. Test in Task 5.
3. A device on an old vault generation uploads after another device reset the vault → 409 `generation`, nothing written. Test in Task 5.
4. A streaming chat completion whose client disconnects mid-stream → the concurrency slot is released and the usage seen so far is recorded. Test in Task 6.
5. A magic link opened twice, or after the device code expired → clear Dutch error on the web page, no second session. Test in Task 3 (server) and Task 8 (page).

---

### Task 1: Database foundation, config and state

**Files:**
- Modify: `Cargo.toml` (workspace deps), `crates/pennel-server/Cargo.toml`, `crates/pennel-server/src/lib.rs`, `crates/pennel-server/src/main.rs`, `crates/pennel-server/tests/router.rs`, `.github/workflows/ci.yml`
- Create: `crates/pennel-server/src/config.rs`, `crates/pennel-server/src/state.rs`, `crates/pennel-server/src/error.rs`, `crates/pennel-server/migrations/0001_accounts.sql`, `crates/pennel-server/tests/common/mod.rs`

**Interfaces:**
- Produces:
  - `pub struct Config { pub public_url: String, pub web_url: String, pub mail: MailConfig, pub upstream: UpstreamConfig, pub daily_cost_cap_cents: Option<u64> }` with `Config::from_env() -> Result<Config, String>` and `Config::for_tests() -> Config`.
  - `pub struct AppState { pub db: PgPool, pub config: Arc<Config>, pub mailer: Arc<dyn Mailer>, pub limiter: Arc<Limiter> }` (`Clone`). `Mailer` and `Limiter` are defined in Tasks 2 and 6; in this task create `mailer.rs` with the trait plus `CaptureMailer` stub and `limiter.rs` with an empty `Limiter::default()`.
  - `pub fn router(state: AppState) -> Router`.
  - `pub enum ApiError { BadRequest(&'static str, &'static str), Unauthorized, Forbidden(&'static str, &'static str), NotFound, Conflict(&'static str, &'static str, Option<serde_json::Value>), Gone(&'static str, &'static str), TooLarge(&'static str), TooMany(&'static str, &'static str), Unavailable(&'static str), Internal }` implementing `IntoResponse` as the JSON shape above; `From<sqlx::Error>` maps to `Internal` and logs the error kind only.
  - `tests/common/mod.rs`: `pub fn app(pool: PgPool) -> (Router, Arc<CaptureMailer>)` and `pub async fn send(app: &Router, req: Request<Body>) -> (StatusCode, serde_json::Value)`.

- [ ] **Step 1:** Add workspace deps (`sqlx = { version = "0.8", default-features = false, features = ["postgres","runtime-tokio","tls-rustls","migrate","macros","chrono","uuid"] }`, `chrono`, `uuid` v7, `rand`, `sha2`, `base64`, `reqwest` (default-features off; `rustls-tls`, `json`, `stream`, `multipart`), `lettre` (default-features off; `tokio1-rustls-tls`, `smtp-transport`, `builder`, `hostname`), `futures-util`, `bytes`). `macros` is only for `#[sqlx::test]`.
- [ ] **Step 2:** Write the migration `0001_accounts.sql`: `users(id uuid pk, email text unique not null, created_at, version_counter bigint not null default 0, purged_through bigint not null default 0)`; `devices(id uuid pk, user_id fk cascade, name text, platform text, created_at, last_seen_at, revoked_at null)`; `sessions(refresh_hash bytea pk, device_id fk cascade, created_at, used_at null)`; `access_tokens(token_hash bytea pk, device_id fk cascade, expires_at)`; `device_codes(code_hash bytea pk, user_code text unique, device_name, platform, expires_at, user_id null, approved_at null, consumed_at null, last_poll_at null)`; `email_logins(token_hash bytea pk, email text, user_code text, expires_at, used_at null)`.
- [ ] **Step 3:** Change the existing `tests/router.rs` to `#[sqlx::test(migrations = "./migrations")] async fn …(pool: PgPool)` using `common::app`, and add `readyz_reports_database` asserting `/readyz` is 200 `"ready"`. Run `DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres cargo test -p pennel-server` with a local `docker run -d -p 5432:5432 -e POSTGRES_PASSWORD=postgres postgres:18-alpine`. Expected: FAIL (router takes no state).
- [ ] **Step 4:** Implement `Config`, `AppState`, `ApiError`, `router(state)`; `/readyz` runs `SELECT 1` (503 `"database"` on failure). `main.rs`: connect lazily with `PgPoolOptions::connect_lazy` (Swarm has no `depends_on`), run `sqlx::migrate!()` in a retry loop (5 s backoff, logs attempt count) before serving, then spawn background jobs (Task 5). Env read by `Config::from_env`: `DATABASE_URL`, `PENNEL_PUBLIC_URL`, `WEB_URL`, `SMTP_URL`, `MAIL_FROM`, `MISTRAL_API_KEY`, `PENNEL_STT_URL` (default `https://api.mistral.ai/v1`), `PENNEL_STT_MODEL` (default `voxtral-mini-latest`), `PENNEL_LLM_URL` (default `https://api.mistral.ai/v1`), `PENNEL_LLM_MODEL` (default `mistral-small-latest`), `PENNEL_DAILY_COST_CAP_CENTS`, `PENNEL_PRICE_AUDIO_CENTS_PER_HOUR`, `PENNEL_PRICE_CENTS_PER_MTOKEN`.
- [ ] **Step 5:** Add to `ci.yml` rust job: `services: postgres: image postgres:18-alpine, env POSTGRES_PASSWORD: postgres, ports 5432:5432, health-cmd pg_isready` and `env: DATABASE_URL: postgres://postgres:postgres@localhost:5432/postgres`.
- [ ] **Step 6:** Run fmt, clippy, tests. Expected: PASS.
- [ ] **Step 7:** Commit `feat(server): connect to Postgres and run migrations at start`.

### Task 2: Mail and the login flow (device code + magic link)

**Files:**
- Create: `crates/pennel-server/src/mailer.rs`, `crates/pennel-server/src/auth/mod.rs`, `crates/pennel-server/src/auth/tokens.rs`, `crates/pennel-server/src/auth/login.rs`, `crates/pennel-server/src/auth/disposable.txt`, `crates/pennel-server/tests/login.rs`
- Modify: `crates/pennel-server/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub trait Mailer: Send + Sync { fn send_login(&self, to: &str, link: &str, user_code: &str, device_name: &str) -> BoxFuture<'_, Result<(), String>>; }`, `SmtpMailer::new(smtp_url, from)`, `CaptureMailer { pub sent: Mutex<Vec<(String, String)>> }` (to, link).
  - `tokens::new_secret() -> String` (32 random bytes, base64url no pad), `tokens::hash(&str) -> [u8; 32]`, `tokens::user_code() -> String` (8 chars from `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`, formatted `XXXX-XXXX`).
  - Routes:
    - `POST /v1/auth/device {device_name, platform}` → `200 {device_code, user_code, verification_uri, interval: 5, expires_in: 900}`; `verification_uri = "{web_url}/device?code={user_code}"`.
    - `POST /v1/auth/device/lookup {user_code}` → `200 {device_name, platform}` or `404 {"error":"code_unknown"}` / `410 {"error":"code_expired"}`.
    - `POST /v1/auth/email {email, user_code}` → `202 {}`; link = `"{web_url}/device/verify#token={token}"`.
    - `POST /v1/auth/email/verify {token}` → `200 {email, device_name}`; creates the user on first login and approves the device code.
    - `POST /v1/auth/token {device_code}` → pending `400 {"error":"authorization_pending"}`, polled faster than `interval` → `400 {"error":"slow_down"}`, expired → `400 {"error":"expired_token"}`, approved → `200 {access_token, refresh_token, expires_in: 900, device_id}` exactly once (then `400 {"error":"expired_token"}`).
  - Spec change recorded here: the separate `POST /auth/device/approve` from spec §3.4 is folded into `email/verify`, because the web page confirms the code before it asks for the e-mail address.

- [ ] **Step 1: Write failing tests in `tests/login.rs`** (all `#[sqlx::test]`):
  - `full_login_issues_tokens_once`: device → email (assert `CaptureMailer` got one mail to that address whose link contains `/device/verify#token=`) → verify returns `{email, device_name}` → token returns 200 with both tokens and a `device_id` → a second token call returns `expired_token`.
  - `token_before_approval_is_pending` and `polling_too_fast_slows_down` (two polls within 5 s → second is `slow_down`).
  - `magic_link_is_single_use`: second verify → 410 `link_used`.
  - `magic_link_after_device_code_expired_fails` (set `expires_at` in the past with SQL) → 410 `code_expired`, and no session row exists.
  - `email_is_normalised`: `" Ignace@Example.COM "` and `"ignace@example.com"` log into the same user.
  - `disposable_email_rejected`: `x@mailinator.com` → 400 `email_disposable`.
  - `eleventh_device_rejected`: with 10 non-revoked devices, verify → 403 `device_limit`, message `"Je hebt al 10 toestellen. Meld er eerst één af op een ander toestel."`.
- [ ] **Step 2:** Run `cargo test -p pennel-server --test login`. Expected: FAIL (routes 404).
- [ ] **Step 3:** Implement. E-mail normalisation: trim + lowercase; basic shape check (one `@`, a dot in the domain, ≤ 254 chars) → 400 `email_invalid`. `disposable.txt` is one domain per line (start with ~50 common ones). Mail body (Dutch, plain text): subject `Inloggen bij Pennel`, body names the device and the code: `Klik om in te loggen op "{device_name}" (code {user_code}): {link}\n\nDeze link werkt één keer en vervalt na 15 minuten. Heb je dit niet gevraagd? Negeer deze mail.` The device row and the session are created in the `token` call, inside one transaction that also sets `consumed_at`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit `feat(server): device-code login with e-mail magic links`.

### Task 3: Sessions, refresh and the bearer extractor

**Files:**
- Create: `crates/pennel-server/src/auth/session.rs`, `crates/pennel-server/tests/session.rs`
- Modify: `crates/pennel-server/src/auth/mod.rs`

**Interfaces:**
- Consumes: tables and `tokens::*` from Tasks 1–2.
- Produces:
  - `pub struct Caller { pub user_id: Uuid, pub device_id: Uuid }` implementing `FromRequestParts<AppState>` (Bearer access token, unexpired, device not revoked; updates `devices.last_seen_at` at most once per minute). Missing or bad → 401 `{"error":"unauthorized"}`.
  - `POST /v1/auth/refresh {refresh_token}` → `200 {access_token, refresh_token, expires_in: 900}`; old refresh row gets `used_at`.

- [ ] **Step 1: Write failing tests:**
  - `refresh_rotates_tokens`: the new access token works on `GET /v1/account` (Task 4 route; until then add a test-only `GET /v1/whoami` returning `{user_id, device_id}` and keep it — the app uses it as a cheap token check).
  - `reused_refresh_token_revokes_device` (Review Focus 1): refresh with token A → B; refresh with A again → 401 `session_revoked`; B and its access tokens now also fail with 401.
  - `expired_access_token_is_unauthorized` (expire with SQL).
  - `revoked_device_is_unauthorized`.
- [ ] **Step 2:** Run `cargo test -p pennel-server --test session`. Expected: FAIL.
- [ ] **Step 3:** Implement. Reuse detection: a refresh hash with `used_at` set revokes the whole device (`revoked_at = now()`, delete its sessions and access tokens). Delete expired access tokens opportunistically on refresh.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5:** Commit `feat(server): rotate refresh tokens and authenticate API calls`.

### Task 4: Account, devices and entitlements

**Files:**
- Create: `crates/pennel-server/src/account.rs`, `crates/pennel-server/src/entitlements.rs`, `crates/pennel-server/migrations/0002_usage.sql`, `crates/pennel-server/tests/account.rs`

**Interfaces:**
- Produces:
  - `pub struct Limits { pub storage_bytes: i64, pub object_bytes: i64, pub devices: i64, pub audio_seconds_per_month: i64, pub tokens_per_month: i64 }`, `pub fn entitlements(_user: Uuid) -> Limits` (the beta values from Global Constraints).
  - `usage_daily(user_id, day date, audio_seconds bigint, input_tokens bigint, output_tokens bigint, pk(user_id, day))` and `pub async fn month_usage(db, user_id) -> Usage { audio_seconds, tokens }` (current UTC calendar month).
  - `GET /v1/account` → `{email, created_at, usage: {audio_seconds, tokens, storage_bytes}, limits: {…Limits}}`.
  - `DELETE /v1/account` → 204; deletes the user (cascade removes devices, sessions, vault, objects, usage).
  - `GET /v1/devices` → `[{id, name, platform, created_at, last_seen_at, current: bool}]` (revoked omitted). `PATCH /v1/devices/{id} {name}` (1–60 chars) → 204. `DELETE /v1/devices/{id}` → 204 (revoke). Another user's device id → 404.

- [ ] **Step 1: Write failing tests:** `account_reports_usage_and_limits` (insert usage rows for this month and last month; only this month counts), `delete_account_removes_everything` (afterwards row counts for that user are 0 in every table, and the token is 401), `devices_list_marks_current`, `rename_device`, `revoke_other_device_logs_it_out`, `cannot_touch_other_users_device`.
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Implement. `storage_bytes` = `sum(size) where not deleted` from Task 5's `objects` table; until Task 5 lands, return 0 (Task 5 adds the query and extends this test).
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5:** Commit `feat(server): account, devices and beta limits`.

### Task 5: Vault, objects and the change feed

**Files:**
- Create: `crates/pennel-server/src/sync.rs`, `crates/pennel-server/src/jobs.rs`, `crates/pennel-server/migrations/0003_sync.sql`, `crates/pennel-server/tests/sync.rs`
- Modify: `crates/pennel-server/src/main.rs` (spawn jobs), `crates/pennel-server/src/account.rs` (storage_bytes)

**Interfaces:**
- Produces (the wire contract the core client in plan 2 is written against — keep it exact):
  - `vaults(user_id pk fk cascade, generation int not null, wrapped_key bytea null, kdf jsonb null, updated_at)`. `objects(user_id fk cascade, object_id text, version bigint, generation int, size int, deleted bool, blob bytea null, updated_at, device_id uuid null, pk(user_id, object_id))`, index `(user_id, version)`.
  - `GET /v1/vault` → `200 {generation, wrapped_key: base64 | null, kdf: object | null}`; no row → `200 {generation: 1, wrapped_key: null, kdf: null}`.
  - `PUT /v1/vault {generation, wrapped_key, kdf}` → 204. Allowed when `generation` equals the current one: creates the key if none, or replaces it (new recovery code). Otherwise 409 `generation`.
  - `DELETE /v1/vault` → `200 {generation}`: deletes all objects of the user, sets `generation = generation + 1`, clears key and kdf, bumps the version counter.
  - `PUT /v1/objects/{object_id}` with headers `If-Match: <version>` (`0` = must not exist or must be a tombstone), `X-Pennel-Generation: <n>`, body = raw ciphertext (`application/octet-stream`) → `200 {version}`. `object_id` must match `^[0-9a-f]{64}$` (400 `object_id`).
  - `GET /v1/objects/{object_id}` → 200 body = ciphertext, headers `ETag: <version>`, `X-Pennel-Generation`; tombstone or missing → 404.
  - `DELETE /v1/objects/{object_id}` with `If-Match`, `X-Pennel-Generation` → `200 {version}` (tombstone: blob null, size 0, deleted true).
  - `GET /v1/sync/changes?since=<cursor>&limit=<n>` (default 500, max 1000) → `{changes: [{id, version, size, deleted}], cursor, more: bool, generation}`, ordered by version. `since < purged_through` → 410 `cursor_expired`.
  - Errors: version mismatch → 409 `{"error":"conflict", "message":"…", "current": {version, deleted}}`; generation mismatch → 409 `generation`; body > 5 MB → 413 `object_too_large`; storage over limit → 403 `storage_full`, message `"Je cloudopslag is vol (1 GB). Lokaal blijft alles werken."`.
  - Every write takes the next value of `users.version_counter` (`UPDATE … SET version_counter = version_counter + 1 RETURNING`) inside the transaction that writes the object, so versions are unique per user and the feed never skips a committed write.
  - `jobs::purge_tombstones(db) -> Result<u64>`: deletes tombstones older than 90 days and sets `purged_through` to the max purged version per user; run daily from `main.rs`. Also delete expired device codes, e-mail logins and access tokens.

- [ ] **Step 1: Write failing tests:**
  - `vault_roundtrip_and_rewrap`.
  - `vault_reset_bumps_generation_and_clears_objects`.
  - `put_get_roundtrip`: the bytes come back identical, and `ETag` equals the returned version.
  - `concurrent_put_same_base_one_wins` (Review Focus 2): two `If-Match: 0` puts → one 200, one 409 with `current.version`.
  - `stale_generation_rejected` (Review Focus 3): after `DELETE /v1/vault`, a put with the old generation → 409 `generation`, and the object count stays 0.
  - `delete_creates_tombstone_in_feed`.
  - `feed_pages_with_more_flag`: 3 objects, `limit=2` → `more: true`; the next page returns the rest.
  - `object_too_large` (5 MB + 1).
  - `storage_full` (set the limit via a seeded large object row).
  - `cursor_expired_after_purge`.
  - `other_users_objects_invisible`.
  - Extend `account_reports_usage_and_limits` with `storage_bytes`.
- [ ] **Step 2:** Run `cargo test -p pennel-server --test sync`. Expected: FAIL.
- [ ] **Step 3:** Implement. Body limit per route: `DefaultBodyLimit::max(5_242_880 + 1)` on object routes, then check precisely. The conflict check is `SELECT … FOR UPDATE` on the row (or on the user row when the object is new) inside the transaction.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5:** Commit `feat(server): encrypted object store with versioned change feed`.

### Task 6: Hosted-model proxy

**Files:**
- Create: `crates/pennel-server/src/proxy/mod.rs`, `crates/pennel-server/src/proxy/stt.rs`, `crates/pennel-server/src/proxy/chat.rs`, `crates/pennel-server/src/limiter.rs`, `crates/pennel-server/tests/proxy.rs`
- Modify: `crates/pennel-server/src/lib.rs`

**Interfaces:**
- Consumes: `Caller` (Task 3), `entitlements`, `month_usage`, `usage_daily` (Task 4).
- Produces:
  - `pub struct UpstreamConfig { pub stt_url: String, pub stt_model: String, pub llm_url: String, pub llm_model: String, pub api_key: String }`.
  - `Limiter::acquire(user_id) -> Result<OwnedSemaphorePermit, ApiError>`: at most 2 per account, plus a global semaphore of 16 audio uploads.
  - `POST /v1/audio/transcriptions` (multipart, OpenAI fields `file`, `model`, `language`, `prompt`, `response_format`) → the upstream JSON unchanged (`{text, …}`).
  - `POST /v1/chat/completions` (OpenAI JSON, `stream` true or false) → the upstream response; with `stream: true`, SSE is passed through chunk by chunk.
  - `pub async fn record_usage(db, user_id, audio_seconds: i64, input_tokens: i64, output_tokens: i64)` (upsert into today's row).

- [ ] **Step 1: Write failing tests** against a fake upstream (an axum app on `127.0.0.1:0` whose URL is put in `UpstreamConfig`):
  - `transcription_forwards_and_counts_audio`: the fake returns `{"text":"hallo","usage":{"prompt_audio_seconds":12}}`. Assert the client gets `text`, the fake saw `model = voxtral-mini-latest` and `Authorization: Bearer <api_key>`, and `usage_daily.audio_seconds = 12`.
  - `chat_rewrites_model_and_counts_tokens` (non-stream; `usage.prompt_tokens/completion_tokens`).
  - `chat_stream_passes_chunks_and_counts_usage`: the fake emits 3 `data:` chunks, a usage chunk and `[DONE]`. Assert the client receives them in order, the fake received `stream_options.include_usage = true`, and usage is recorded.
  - `stream_disconnect_releases_slot` (Review Focus 4): drop the client body after the first chunk, then start two new requests → both are admitted, not 429.
  - `unknown_model_rejected` (`gpt-4o` → 400 `model`).
  - `third_concurrent_request_is_429`.
  - `month_limit_reached_is_429`: seed `audio_seconds = 72000` → 429 `quota`, message `"Je hebt je Pennel Cloud-limiet voor deze maand bereikt. Je kan een eigen provider kiezen in Providers."`.
  - `daily_cost_cap_is_503`: message `"Pennel Cloud is tijdelijk overbelast. Probeer het later opnieuw."`.
  - `upstream_error_is_502_without_body_leak` (the fake returns 500 with text → the client gets 502 `upstream` with the Dutch message; the upstream body is not echoed).
  - `unauthenticated_is_401`.
- [ ] **Step 2:** Run `cargo test -p pennel-server --test proxy`. Expected: FAIL.
- [ ] **Step 3:** Implement. Multipart is rebuilt field by field into a `reqwest::multipart::Form`, with `model` replaced. Audio seconds come from upstream `usage.prompt_audio_seconds`; if that is absent, use the WAV header duration when the file is WAV, else `ceil(bytes / 16_000)` (a conservative estimate, labelled so in a comment). The 30-minute clip limit from the spec is enforced only for WAV, where it is cheap; other formats rely on the 25 MB cap (my call, recorded in the spec's §6 list in Task 7). SSE: wrap the upstream byte stream in a stream that parses `data:` lines for a `usage` object, passes bytes through unchanged, and records usage in a `Drop` guard that also holds the permit. Daily cost cap: today's global sums × `PENNEL_PRICE_*` ≥ `PENNEL_DAILY_COST_CAP_CENTS` → 503; unset = no cap.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5:** Commit `feat(server): OpenAI-compatible proxy to Mistral with per-account limits`.

### Task 7: Rate limits, logging and deployment

**Files:**
- Modify: `crates/pennel-server/src/limiter.rs`, `crates/pennel-server/src/lib.rs`, `deploy/docker-stack.yml`, `deploy/docker-compose.yml`, `deploy/.env.example`, `.github/workflows/set-env.yml`, `README.md`, `docs/infrastructure.md` (only after the user's own pending edit is committed or with their OK), `musli/docs/superpowers/specs/2026-10-25-meeting-sync-design.md` (§3.4 table: fold approve into verify; §6: WAV-only clip limit)
- Test: `crates/pennel-server/tests/limits.rs`

**Interfaces:**
- Produces: `Limiter::check_ip(ip, bucket: &'static str) -> Result<(), ApiError>` token buckets. Per IP: `auth` 20/min, `email` 5/hour. Per e-mail address: 3 mails/hour (keyed by a hash of the address). Client IP comes from the first `X-Forwarded-For` entry (Traefik) and falls back to the peer address.

- [ ] **Step 1: Write failing tests:** `sixth_login_mail_per_hour_is_429` and `auth_burst_is_429` (Dutch message `"Te veel pogingen. Wacht even en probeer opnieuw."`), plus `trace_layer_logs_no_bodies_or_ids`. For the last one, capture tracing output with a test subscriber over a sync PUT and an e-mail request, and assert that neither the object id nor the e-mail address appears.
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Implement. Use `tower_http::trace::TraceLayer` with a custom `make_span` that records `MatchedPath`, not the raw URI. Add `tower-http` (`trace`, `limit`) to the workspace.
- [ ] **Step 4:** Deploy files. Server env: `WEB_URL: ${WEB_URL:-}`, `SMTP_URL: ${SMTP_URL:-}`, `MAIL_FROM: ${MAIL_FROM:-Pennel <login@pennel.app>}`, `MISTRAL_API_KEY: ${MISTRAL_API_KEY:-}`, `PENNEL_DAILY_COST_CAP_CENTS: ${PENNEL_DAILY_COST_CAP_CENTS:-}`. Raise the server memory limit from 256M to 512M (multipart buffering). Add `SMTP_URL`, `MISTRAL_API_KEY` and `PENNEL_DAILY_COST_CAP_CENTS` to the `options` in `set-env.yml`. Document all of them in `.env.example` and README. The server must start without `SMTP_URL` or `MISTRAL_API_KEY`: login mail then fails with 503 `mail`, and the proxy with 503 `upstream`.
- [ ] **Step 5:** Run fmt, clippy and `cargo test --workspace`. Expected: PASS.
- [ ] **Step 6:** Commit `feat(server): rate limits, content-free logs and deploy settings`.

### Task 8: Login page `pennel.app/device`

**Files:**
- Create: `apps/web/app/[lang]/device/page.tsx`, `apps/web/app/[lang]/device/device-flow.tsx` (client), `apps/web/app/[lang]/device/verify/page.tsx`, `apps/web/app/[lang]/device/verify/verify-flow.tsx` (client), `apps/web/app/api/device/[action]/route.ts`, `apps/web/app/api/device/route.test.ts`, `apps/web/app/[lang]/device/device-flow.test.tsx`
- Modify: `apps/web/lib/messages/{en,nl,fr,de}.ts` (new `device` section), `apps/web/lib/site.ts` (read `API_URL`), `apps/web/proxy.ts` (no CDN cache for `/device`), `apps/web/proxy.test.ts`

**Interfaces:**
- Consumes: server routes `device/lookup`, `email`, `email/verify` (Task 2).
- Produces:
  - `POST /api/device/lookup|email|verify` route handlers that forward the JSON body to `${API_URL}/v1/auth/device/lookup|email|email/verify` and return status and JSON unchanged. Any other action → 404. `API_URL` unset → 503.
  - The page `/{lang}/device?code=XXXX-XXXX`. The bare `/device?code=…` is redirected by the existing proxy to the negotiated locale, and the query string must survive that redirect (test it).

- [ ] **Step 1: Write failing tests:**
  - `route.test.ts`: forwards to `API_URL` with the same body, passes status through, rejects unknown actions, and returns 503 without `API_URL`.
  - `proxy.test.ts`: `/device?code=ABCD-EFGH` → 307 to `/nl/device?code=ABCD-EFGH` (Accept-Language nl), and `/nl/device` carries no `CDN-Cache-Control`.
  - `device-flow.test.tsx` (vitest with jsdom for this file only): it shows the code from the URL and the device name from lookup. The "Ja, dat is mijn code" button reveals the e-mail field; submit shows "Check je mail". It shows the `code_expired` message.
  - A verify test: `link_used` shows its message (Review Focus 5); success shows `"Je bent ingelogd op {device_name}. Je kan dit venster sluiten en teruggaan naar Pennel."`.
- [ ] **Step 2:** Run `npm test`. Expected: FAIL.
- [ ] **Step 3:** Implement. The verify page reads the token from `location.hash`, so it never reaches server logs or the `Referer`. `export const dynamic = "force-dynamic"` is not needed, because the page is a static shell with client fetches. Dutch copy in `nl.ts` (en/fr/de translated in the same style; `messages.test.ts` enforces parity):
  - title `Inloggen bij Pennel`
  - step 1 `Komt deze code overeen met wat Pennel toont op {device_name}?` with button `Ja, dat is mijn code`
  - step 2 label `Je e-mailadres` with button `Stuur inloglink`
  - sent `Check je mail. Klik op de link om in te loggen. De link werkt 15 minuten.`
  - errors `code_unknown`: `Deze code kennen we niet. Start het inloggen opnieuw in Pennel.`; `code_expired`: `Deze code is verlopen. Start het inloggen opnieuw in Pennel.`; `link_used`: `Deze link is al gebruikt. Start het inloggen opnieuw in Pennel.`; `email_disposable`: `Gebruik een vast e-mailadres.`

  Styling uses the existing brand classes only (the palette test limits CSS to seven hex values).
- [ ] **Step 4:** Run `npm run lint && npm run typecheck && npm test && npm run build`. Expected: PASS.
- [ ] **Step 5:** Commit `feat(web): device login page`.

### Task 9: Site copy at release (only when sync and the proxy are live on production)

**Files:**
- Modify: `apps/web/lib/messages/{en,nl,fr,de}.ts` (Cloud page, `/privacy`, pricing feature list, FAQ), `README.md`

- [ ] **Step 1:** Replace "Je meetings blijven op je toestel" (and its translations) where it describes Cloud with `Zet je sync aan, dan gaan je meetings end-to-end versleuteld naar Pennel Cloud. Wij kunnen ze niet lezen. Audio blijft op je toestel.` Add what the server sees: `aantal bestanden, grootte, tijdstippen en welk toestel uploadt`, and that the recovery code is the only way back. Change the Sync feature list to what ships: meetings and templates in v1, not settings or the dictionary. The pricing badges stay "Coming soon" until payments exist. During the beta, describe Cloud as `Gratis tijdens de beta, binnen de limieten`.
- [ ] **Step 2:** `npm run lint && npm run typecheck && npm test && npm run build`. Expected: PASS (catalogue parity).
- [ ] **Step 3:** Commit `docs(web): describe Pennel Cloud sync as shipped`.
