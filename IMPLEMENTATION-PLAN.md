# Implementation Plan: Feature Parity with Python Version

## Overview

Port all missing features from `requestrepo-dev/` (Python/FastAPI) to `requestrepo/` (Rust/Axum).
The Rust backend already has: sessions, DNS, files, requests, notifications, WebSocket, SMTP, TCP, TLS/ACME.
What's missing: OAuth/OIDC auth, admin dashboard, login page, profile page, UI theme.

---

## Phase 1: Backend - Config & Dependencies

### 1a. Add env vars to `src/src/utils/config.rs`

Add these fields to the `Config` struct:

```rust
// GitHub OAuth
pub github_enabled: bool,
pub github_client_id: String,
pub github_client_secret: String,
pub github_callback_url: String,
pub github_allowed_usernames: Vec<String>,
pub allowed_admins: Vec<String>,

// OIDC
pub oidc_providers: Vec<OidcProviderConfig>,

// Auth toggle
pub disable_auth: bool,
```

Add a new struct `OidcProviderConfig`:

```rust
#[derive(Debug, Clone)]
pub struct OidcProviderConfig {
    pub name: String,
    pub display_name: String,
    pub client_id: String,
    pub client_secret: String,
    pub discover_url: String,
    pub allowed_users: Vec<String>,
    pub allowed_admins: Vec<String>,
}
```

Parse in `Config::new()` from env vars:
- `GITHUB_ENABLED` (default "true")
- `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET`, `GITHUB_CALLBACK_URL`
- `GITHUB_ALLOWED_USERNAMES` (comma-separated)
- `ALLOWED_ADMINS` (comma-separated)
- `OIDC_PROVIDERS` (comma-separated names like "authentik,pocketid")
- For each provider: `OIDC_{NAME}_CLIENT_ID`, `OIDC_{NAME}_CLIENT_SECRET`, `OIDC_{NAME}_DISCOVER_URL`, `OIDC_{NAME}_DISPLAY_NAME`, `OIDC_{NAME}_ALLOWED_USERS`, `OIDC_{NAME}_ALLOWED_ADMINS`
- `DISABLE_AUTH` (default "false")

### 1b. Add crates to `src/Cargo.toml`

```toml
# OAuth2 / OIDC
oauth2 = "4"
openidconnect = "4"

# Cookie management
tower-cookies = { version = "0.11", features = ["private"] }

# For redirect responses
axum-extra = { version = "0.9", features = ["cookie", "typed-header"] }
```

Note: `axum-extra` is already in Cargo.toml with "cookie" feature. Need to add "typed-header".

---

## Phase 2: Backend - Auth Module

### 2a. Create `src/src/auth/mod.rs`

This is the main auth module. It handles:
- GitHub OAuth flow (redirect -> callback -> token exchange -> user info)
- OIDC flow (discovery -> redirect -> callback -> token exchange -> userinfo)
- Session management (cookie-based, stored in cache)
- User lookup and admin checks
- Route protection logic

```rust
pub mod github;
pub mod oidc;
pub mod session;
```

### 2b. Create `src/src/auth/session.rs`

Session management using cache + cookies:

```rust
// Session data stored in cache:
// session:{session_token_hash} -> UserSession JSON
// registered_users -> Set of usernames (JSON array)
// registered_user:{username} -> UserProfile JSON
// user_sessions:{username} -> Set of session token hashes

pub struct UserSession {
    pub username: String,
    pub access_token: String,  // OAuth provider access token
    pub avatar_url: String,
    pub name: String,
    pub created_at: String,
    pub last_accessed: String,
    pub is_admin: bool,
}

pub struct UserProfile {
    pub username: String,
    pub avatar_url: String,
    pub name: String,
    pub created_at: String,
    pub is_admin: bool,
}

// Functions:
pub async fn create_session(cache, username, user_data, access_token) -> Result<String>
pub async fn get_session(cache, session_token) -> Result<Option<UserSession>>
pub async fn delete_session(cache, session_token) -> Result<()>
pub async fn delete_all_user_sessions(cache, username) -> Result<()>
pub async fn get_current_user(cache, cookies) -> Result<Option<UserSession>>
pub async fn register_user(cache, profile) -> Result<()>
pub async fn get_all_users(cache) -> Result<Vec<UserProfile>>
pub async fn is_user_allowed(cache, username) -> bool
pub async fn is_user_admin(cache, username) -> bool
```

### 2c. Create `src/src/auth/github.rs`

GitHub OAuth2 flow using the `oauth2` crate:

```rust
// Redirect to GitHub:
// GET /auth/github -> 302 redirect to github.com/login/oauth/authorize?client_id=...&scope=read:user+user:email&state=csrf

// Callback:
// GET /auth/github/callback?code=...&state=...
// 1. Verify CSRF state parameter
// 2. Exchange code for access token via POST https://github.com/login/oauth/access_token
// 3. GET https://api.github.com/user with access token
// 4. Check allowed_usernames
// 5. Create session, set cookie, redirect to /
```

### 2d. Create `src/src/auth/oidc.rs`

OIDC flow using the `openidconnect` crate:

```rust
// For each configured provider:
// 1. Discover endpoints from .well-known/openid-configuration
// 2. Build authorization URL with scope=openid email profile
// 3. Redirect to provider
// 4. Handle callback: exchange code for tokens, get userinfo
// 5. Check allowed_users
// 6. Create session, set cookie, redirect to /
```

---

## Phase 3: Backend - Auth Routes

### 3a. Create `src/src/http/routes_auth.rs`

```rust
// GET /auth/providers -> list of {name, display_name, type, icon, login_url}
// GET /auth/github -> redirect to GitHub OAuth
// GET /auth/github/callback -> handle callback, create session, redirect to /
// GET /auth/oidc/{provider} -> redirect to OIDC provider
// GET /auth/oidc/{provider}/callback -> handle callback
// GET /auth/user -> get current user from cookie
// GET /auth/check-token -> check token validity
// POST /auth/refresh-token -> refresh token
// GET /auth/logout -> clear session cookie
```

### 3b. Register in `src/src/http/mod.rs`

Add these routes to the router (without CORS, like catch-all):

```rust
.route("/auth/providers", get(routes_auth::get_providers))
.route("/auth/github", get(routes_auth::github_login))
.route("/auth/github/callback", get(routes_auth::github_callback))
.route("/auth/oidc/{provider}", get(routes_auth::oidc_login))
.route("/auth/oidc/{provider}/callback", get(routes_auth::oidc_callback))
.route("/auth/user", get(routes_auth::get_user))
.route("/auth/check-token", get(routes_auth::check_token))
.route("/auth/refresh-token", post(routes_auth::refresh_token))
.route("/auth/logout", get(routes_auth::logout))
```

---

## Phase 4: Backend - Admin Routes

### 4a. Create `src/src/http/routes_admin.rs`

All routes require authenticated admin user (check cookie session).

```rust
// GET /api/v2/admin/users -> list all registered users
// GET /api/v2/admin/subdomains -> list all subdomains (cache.keys("subdomain:*") but prefix is "files:")
// GET /api/v2/admin/config -> {subdomain_length, subdomain_alphabet}
// GET /api/v2/admin/logs/:subdomain -> get request logs for subdomain
// DELETE /api/v2/admin/subdomains/:subdomain -> delete subdomain + all data
// POST /api/v2/admin/generate_token/:subdomain -> generate JWT for subdomain
```

### 4b. Register in `src/src/http/mod.rs`

```rust
.route("/api/v2/admin/users", get(routes_admin::get_users))
.route("/api/v2/admin/subdomains", get(routes_admin::get_subdomains))
.route("/api/v2/admin/config", get(routes_admin::get_config))
.route("/api/v2/admin/logs/:subdomain", get(routes_admin::get_logs))
.route("/api/v2/admin/subdomains/:subdomain", delete(routes_admin::delete_subdomain))
.route("/api/v2/admin/generate_token/:subdomain", post(routes_admin::generate_token))
```

---

## Phase 5: Frontend - Static Assets

Copy these files from `requestrepo-dev/frontend/public/` to `frontend/public/`:
- `nvroot-dark.svg` (login page logo)
- `nvroot-light.svg` (login page logo light variant)
- `github.svg` (GitHub OAuth button)
- `authentik.svg` (Authentik OAuth button)
- `pocketid.svg` (PocketID OAuth button)

---

## Phase 6: Frontend - Login Page + Doodles

### 6a. Create `frontend/src/components/ui/Doodles.tsx`

Port from Python `Doodles.jsx`:
- 7 SVG types: crosshair, plus, bracket, square, dot, wave, triangle
- Random positions, sizes, rotations, opacities
- Fixed position, pointer-events: none, z-index: 0

### 6b. Create `frontend/src/pages/LoginPage.tsx`

Full-page login matching old version:
- Dark background `#0c0a09`
- `<Doodles count={120} />` background
- Centered card (max-width 480px, border `#555555`, bg `#222222`)
- Logo header (nvroot-dark.svg or nvroot-light.svg based on theme)
- Title "OOB SERVER"
- OAuth provider buttons fetched from `/auth/providers`
- Each button: full-width image (`/{provider.icon}`), no border, click -> redirect to login URL
- Loading spinner with yellow accent `#F6D30F`

### 6c. Add route in `App.tsx`

```tsx
<Route path="/login" element={<LoginPage />} />
```

---

## Phase 7: Frontend - Profile Page

### 7a. Create `frontend/src/pages/ProfilePage.tsx`

Match old version's `profile-page.jsx`:
- User avatar (circle, 10rem, shadow-2)
- If no avatar: show first letter of name in colored circle
- User info: name, username, account created, admin status
- Session management: Logout button, Logout All Sessions button
- Uses auth context/store for user data

### 7b. Create `frontend/src/components/layout/FullWidthLayout.tsx`

Layout without sidebar (for login, profile, admin):
- Topbar only
- Content area below topbar

### 7c. Add route in `App.tsx`

```tsx
<Route path="/profile" element={<FullWidthLayout><ProfilePage /></FullWidthLayout>} />
```

---

## Phase 8: Frontend - Admin Dashboard

### 8a. Create `frontend/src/pages/AdminPage.tsx`

Port from Python `admin-dashboard.jsx` (1901 lines). Key sections:

1. **Stats cards row**: Total Users, Total Subdomains, Active Sessions, Total Logs
2. **Two-column layout**:
   - Left: Users table (Username, Name, Admin status) with custom subdomain input + "Get" button
   - Right: Subdomains table with search, refresh, delete all, per-row actions (view logs, open session, share, delete)
3. **Logs section**: Full-width table below, shows when subdomain selected
   - Method, Path, IP, Type, User Agent, Date columns
   - Click row -> modal with full request details
   - Modal has: delete, copy raw, send to Discord/Mattermost/Telegram, close buttons
   - Modal shows: request details table, headers, query params, form data, raw request

### 8b. API client additions (`frontend/src/api/client.ts`)

Add admin API methods:
```typescript
getAdminUsers(token: string)
getAdminSubdomains(token: string)
getAdminConfig(token: string)
getAdminLogs(token: string, subdomain: string)
deleteAdminSubdomain(token: string, subdomain: string)
generateAdminToken(token: string, subdomain: string)
```

### 8c. Add route in `App.tsx`

```tsx
<Route path="/admin" element={<FullWidthLayout><AdminPage /></FullWidthLayout>} />
```

---

## Phase 9: Frontend - UI Overhaul

### 9a. `frontend/tailwind.config.ts`

Add custom theme colors matching old version:

```ts
theme: {
  extend: {
    colors: {
      brand: {
        yellow: '#F6D30F',
        'yellow-darker': '#D4B00D',
      },
      surface: {
        ground: '#0c0a09',
        card: '#222222',
        border: '#555555',
        overlay: '#424242',
      },
    },
  },
},
```

### 9b. `frontend/src/index.css`

Add global styles:

```css
/* Request method badge colors */
.badge-get { background-color: #20d077; }
.badge-post { background-color: #ffae00; }
.badge-put { background-color: #0036ff; }
.badge-delete { background-color: #ff0000; }
.badge-dns { background-color: #33daff; }
.badge-smtp { background-color: #e91e63; }

/* Login page styles */
.login-card { ... }
.login-header { ... }
.loading-spinner { ... }
```

### 9c. `frontend/src/components/layout/AppLayout.tsx`

- Sidebar width: 300px (was 240px)
- Dark mode defaults
- Border styling

### 9d. `frontend/src/components/layout/Sidebar.tsx`

- Better badge colors (GET=green, POST=amber, etc.)
- Country flag icons
- Relative timestamps
- "NEW" badges

---

## Phase 10: Frontend - Auth Flow & Route Protection

### 10a. `frontend/src/stores/authStore.ts`

Expand to store user object:
```typescript
interface AuthState {
  user: UserProfile | null;
  showAuthOverlay: boolean;
  authError: string | null;
  backendOffline: boolean;
  // actions...
}
```

### 10b. Create `frontend/src/components/auth/ProtectedRoute.tsx`

Route guard:
- If `DISABLE_AUTH=true` or no providers configured -> allow all
- If not authenticated -> redirect to `/login`
- If `adminOnly` prop and user is not admin -> redirect to `/`

### 10c. `frontend/src/App.tsx`

Update routing:
```tsx
<Routes>
  <Route path="/login" element={<LoginPage />} />
  <Route path="/" element={<ProtectedRoute><AppLayout /></ProtectedRoute>}>
    <Route index element={<RedirectWithParams />} />
    <Route path="requests" element={<RequestsPage />} />
    <Route path="response" element={<ResponseEditorPage />} />
    <Route path="dns" element={<DnsSettingsPage />} />
    <Route path="notifications" element={<NotificationsPage />} />
    <Route path="profile" element={<ProfilePage />} />
    <Route path="admin" element={<ProtectedRoute adminOnly><AdminPage /></ProtectedRoute>} />
  </Route>
</Routes>
```

### 10d. `frontend/src/components/layout/Topbar.tsx`

Add user profile section (like old version):
- Profile circle with avatar/initials
- Click -> dropdown with Profile, Admin (if admin), Logout

---

## Phase 11: Backend .env.example Update

Add to `.env.example`:

```env
# === OAuth/OIDC Configuration (optional) ===
# Set DISABLE_AUTH=true to skip all authentication
DISABLE_AUTH=false

# GitHub OAuth
GITHUB_ENABLED=true
GITHUB_CLIENT_ID=
GITHUB_CLIENT_SECRET=
GITHUB_CALLBACK_URL=https://yourdomain.com/auth/github/callback
GITHUB_ALLOWED_USERNAMES=user1,user2
ALLOWED_ADMINS=admin1

# OIDC Providers (comma-separated names)
OIDC_PROVIDERS=authentik,pocketid

# Authentik OIDC
OIDC_AUTHENTIK_CLIENT_ID=
OIDC_AUTHENTIK_CLIENT_SECRET=
OIDC_AUTHENTIK_DISCOVER_URL=https://auth.yourdomain.com/application/o/provider/.well-known/openid-configuration
OIDC_AUTHENTIK_DISPLAY_NAME=Authentik
OIDC_AUTHENTIK_ALLOWED_USERS=user1,user2
OIDC_AUTHENTIK_ALLOWED_ADMINS=admin1

# PocketID OIDC
OIDC_POCKETID_CLIENT_ID=
OIDC_POCKETID_CLIENT_SECRET=
OIDC_POCKETID_DISCOVER_URL=https://pocketid.yourdomain.com/.well-known/openid-configuration
OIDC_POCKETID_DISPLAY_NAME=PocketID
OIDC_POCKETID_ALLOWED_USERS=user1,user2
OIDC_POCKETID_ALLOWED_ADMINS=admin1
```

---

## File Creation Summary

### Backend (Rust) - New Files
| File | Purpose |
|------|---------|
| `src/src/auth/mod.rs` | Auth module root |
| `src/src/auth/session.rs` | Session management |
| `src/src/auth/github.rs` | GitHub OAuth |
| `src/src/auth/oidc.rs` | OIDC provider |
| `src/src/http/routes_auth.rs` | Auth HTTP routes |
| `src/src/http/routes_admin.rs` | Admin HTTP routes |

### Backend (Rust) - Modified Files
| File | Changes |
|------|---------|
| `src/Cargo.toml` | Add oauth2, openidconnect, tower-cookies |
| `src/src/utils/config.rs` | Add OAuth/OIDC env vars |
| `src/src/http/mod.rs` | Register auth + admin routes |
| `src/src/cache/mod.rs` | Remove #[allow(dead_code)] from keys() |

### Frontend (React) - New Files
| File | Purpose |
|------|---------|
| `frontend/src/pages/LoginPage.tsx` | Login page |
| `frontend/src/pages/ProfilePage.tsx` | Profile page |
| `frontend/src/pages/AdminPage.tsx` | Admin dashboard |
| `frontend/src/components/ui/Doodles.tsx` | Animated background |
| `frontend/src/components/layout/FullWidthLayout.tsx` | Layout without sidebar |
| `frontend/src/components/auth/ProtectedRoute.tsx` | Route guard |

### Frontend (React) - Modified Files
| File | Changes |
|------|---------|
| `frontend/src/App.tsx` | Add routes + auth flow |
| `frontend/src/api/client.ts` | Add admin API methods |
| `frontend/src/stores/authStore.ts` | Add user state |
| `frontend/src/components/layout/Topbar.tsx` | Add profile dropdown |
| `frontend/src/components/layout/AppLayout.tsx` | Update sidebar width |
| `frontend/src/components/layout/Sidebar.tsx` | Update badge colors |
| `frontend/src/index.css` | Add global styles |
| `frontend/tailwind.config.ts` | Add custom theme |
| `frontend/public/` | Add static assets (SVGs) |

### Config Files
| File | Changes |
|------|---------|
| `.env.example` | Add OAuth/OIDC env vars |

---

## Implementation Order

1. Create IMPLEMENTATION-PLAN.md (this file)
2. Backend: config.rs + Cargo.toml (dependencies)
3. Backend: auth module (session, github, oidc)
4. Backend: auth routes
5. Backend: admin routes
6. Backend: register routes in mod.rs
7. Frontend: copy static assets
8. Frontend: Doodles component
9. Frontend: LoginPage
10. Frontend: ProfilePage + FullWidthLayout
11. Frontend: AdminPage
12. Frontend: UI overhaul (theme, CSS)
13. Frontend: authStore, ProtectedRoute, App.tsx routing
14. Frontend: Topbar profile dropdown
15. Update .env.example
16. Build verification
