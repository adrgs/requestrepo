# NVROOT.md — Upstream Sync Log

## Fork Overview

- **Fork:** `nvroot/requestrepo` (this repo)
- **Upstream:** `adrgs/requestrepo`
- **Fork custom features:** Notification system (Discord/Mattermost/Telegram webhooks + email), reqwest 0.13.4 upgrade

## Divergence Point

**Last shared commit:** `aba70a82` — `fix(dns): add TCP transport and stop resolver retry storms` (2026-07-20)

The fork and upstream diverged here. The fork added 2 commits (notifications + reqwest upgrade), upstream added 2 commits (security policy + content hardening).

## Sync Session: 2026-08-21

### Upstream Commits Synced

| # | Upstream Hash | Summary | Classification | Status |
|---|---------------|---------|---------------|--------|
| 1 | `3155144` | docs: add security policy | [B] NOT PRESENT, CLEANLY APPLICABLE | Ported |
| 2 | `30fd35b` | fix: harden user content and connection handling | [B] NOT PRESENT, CLEANLY APPLICABLE | Ported |

**All items classified [B]. No [C] conflicts. No [D] exclusions.**

The fork's only custom code (notifications module in `src/src/notifications/mod.rs`, `routes_v2.rs`, `NotificationsPage.tsx`, `Toolbar.tsx`, `App.tsx`, `types/index.ts`) is entirely independent of the upstream changes. All upstream changes applied cleanly.

### What Was Ported (18 files)

| File | Change | Upstream Ref |
|------|--------|--------------|
| `SECURITY.md` (new) | Security policy with vulnerability reporting process | `3155144` |
| `src/Cargo.toml` | Trust-DNS → Hickory DNS, rustls-pemfile removed, dotenv → dotenvy, rand/anyhow bumps | `30fd35b` |
| `src/src/main.rs` | `dotenv` → `dotenvy` | `30fd35b` |
| `src/src/utils/config.rs` | `allow_all_headers` → `dangerously_allow_same_origin_user_content` + `dangerously_allow_all_headers` | `30fd35b` |
| `src/src/http/routes.rs` | Path-based user content gating (404 when disabled), expanded header blocklist (1→8 headers), `should_block_header_on_main_domain()` helper, new test | `30fd35b` |
| `src/src/http/static_files.rs` | Inject `DANGEROUSLY_ALLOW_SAME_ORIGIN_USER_CONTENT` into frontend config | `30fd35b` |
| `src/src/http/mod.rs` | HTTPS connection semaphore (1024), TLS handshake timeout (30s), Sentry context sanitization middleware, new test | `30fd35b` |
| `src/src/dns/mod.rs` | Hickory DNS migration (all API calls), TCP connection semaphore (256), message body timeout, write timeout, idle timeout 10s→30s | `30fd35b` |
| `src/src/certs/challenge.rs` | Hickory DNS resolver migration (TokioAsyncResolver → TokioResolver, new API) | `30fd35b` |
| `src/src/certs/tls.rs` | rustls-pemfile → PemObject (simplified PEM parsing from ~35 lines to 2) | `30fd35b` |
| `src/src/tests/dns_tests.rs` | Hickory DNS API migration in tests | `30fd35b` |
| `frontend/src/api/client.ts` | JWT from query params → Authorization header, `authHeaders()` helper; also updated notification API calls (fork-specific) | `30fd35b` |
| `frontend/src/main.tsx` | Sentry scrubbing: added `request` param deletion + shared request path redaction, config type update | `30fd35b` |
| `frontend/src/pages/RequestsPage.tsx` | Conditional `/r/` route display based on `DANGEROUSLY_ALLOW_SAME_ORIGIN_USER_CONTENT` | `30fd35b` |
| `.env.example` | `ALLOW_ALL_HEADERS` → `DANGEROUSLY_ALLOW_SAME_ORIGIN_USER_CONTENT` + `DANGEROUSLY_ALLOW_ALL_HEADERS` | `30fd35b` |
| `docker-compose.yml` | Same env var rename | `30fd35b` |
| `README.md` | Updated env var table | `30fd35b` |

### What Was Skipped

Nothing. All upstream changes were ported.

### Fork-Specific Adaptations

The following fork-specific code was adapted to work with the new security changes:

1. **Notification API calls in `client.ts`:** The fork's notification endpoints (`getNotificationSettings`, `updateNotificationSettings`, `sendTestNotification`, `sendRequestNotification`) were also migrated from query params to Authorization headers, matching the pattern applied to all other API calls.

### Items Flagged for Review

None. All changes are straightforward [B] ports with no ambiguity.

### Breaking Changes to Be Aware Of

1. **Environment variable renames:** `ALLOW_ALL_HEADERS` is now split into `DANGEROUSLY_ALLOW_SAME_ORIGIN_USER_CONTENT` (default: false) and `DANGEROUSLY_ALLOW_ALL_HEADERS` (default: false). Deployments using `ALLOW_ALL_HEADERS=true` need to update their `.env` files.

2. **Path-based user content disabled by default:** The `/r/<subdomain>/` route on the main domain now returns 404 unless `DANGEROUSLY_ALLOW_SAME_ORIGIN_USER_CONTENT=true` is set. This is a security hardening measure.

3. **JWT tokens moved to Authorization headers:** The frontend no longer passes tokens as query parameters. The backend (`routes_v2.rs`) already supports both query params and Bearer headers via `verify_token_error()`, so this is backward-compatible.

4. **DNS library migration:** Trust-DNS → Hickory DNS. The public API is identical; only internal imports and some API calls changed.

### Future Sync Notes

- The divergence point (`aba70a82`) is now fully synced — both upstream commits have been ported.
- The fork is now at parity with upstream `30fd35b` plus the fork's custom notification system.
- Next sync: look for new upstream commits after `30fd35b` (2026-08-21).
