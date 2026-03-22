# Architecture Research

**Domain:** Rust single-binary agent memory server — v1.6 embedded web dashboard integration
**Researched:** 2026-03-22
**Confidence:** HIGH (direct codebase inspection of v1.5 source + verified against axum-embed docs, rust-embed official examples, axum Router docs)

---

## Context: What Already Exists (v1.5)

The v1.5 binary is ~11,940 lines of Rust. The current server architecture is:

```
AppState {
    service:      Arc<MemoryService>,        // holds Arc<dyn StorageBackend>
    compaction:   Arc<CompactionService>,    // holds Arc<dyn StorageBackend> + audit_db
    key_service:  Arc<KeyService>,           // holds Arc<Connection> (SQLite-only)
    backend_name: String,                    // display string from config.storage_provider
}
```

`server::build_router(state)` returns a `Router` with two sub-routers merged together:
- `protected`: `/memories*`, `/keys*` — wrapped with `route_layer(auth_middleware)`
- `public`: `/health`

`server::serve()` binds a TCP listener on `config.port` (default 8080) and runs the axum Router.
`grpc::serve()` binds a separate TCP listener on `config.grpc_port` (default 50051), started via `tokio::try_join!` when `interface-grpc` feature is enabled.

**The key question for v1.6:** How does the dashboard attach to this router without modifying existing route behavior?

---

## v1.6 System Overview

The recommended approach is **same-port, nested router, /ui prefix, feature-gated**. The dashboard router merges into the existing `build_router()` function behind a `#[cfg(feature = "dashboard")]` block. No new port, no new listener, no `tokio::try_join!` change.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Entry Point (main.rs)                             │
│                                                                              │
│  AppState (unchanged struct, no new fields)                                  │
│                                                                              │
│  server::serve(&config, state)   ← single REST server, unchanged call site  │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│              server::build_router(state) — MODIFIED                          │
│                                                                              │
│  let protected = Router::new()                                               │
│      .route("/memories*")                                                    │
│      .route("/keys*")                                                        │
│      .route_layer(auth_middleware);    ← UNCHANGED                           │
│                                                                              │
│  let public = Router::new()                                                  │
│      .route("/health");               ← UNCHANGED                            │
│                                                                              │
│  #[cfg(feature = "dashboard")]                                               │
│  let dashboard = dashboard::build_dashboard_router();   ← NEW               │
│                                                                              │
│  Router::new()                                                               │
│      .merge(protected)                                                       │
│      .merge(public)                                                          │
│      .merge(dashboard)              ← cfg-gated merge, /ui prefix inside    │
│      .with_state(state)                                                      │
└─────────────────────────────────────────────────────────────────────────────┘
              │                    │
              ▼                    ▼
  /memories*, /keys*,          /ui, /ui/*
  /health                      (static SPA assets)
  (unchanged)                  (dashboard only)
```

---

## Router Integration: Nested vs Merge vs Separate Service

| Approach | How It Works | Verdict |
|----------|-------------|---------|
| `Router::merge()` at top level with `/ui` prefix inside `build_dashboard_router()` | Dashboard router is self-contained with its own `nest_service("/ui", ...)` internally; merged into main router | **RECOMMENDED** |
| `Router::nest("/ui", dashboard_router)` in `build_router()` | Strips `/ui` prefix before passing to nested router | Works but means assets are served without knowing their mount path |
| Separate listener on a third port | Separate `TcpListener`, another `tokio::try_join!` arm | Over-engineered for static asset serving; breaks single-origin assumption |
| Same router, routes added inline | Add `/ui` routes directly to `build_router()` with `#[cfg]` on each | Clutters existing function, harder to feature-gate cleanly |

**Use `merge()` with prefix self-contained in `dashboard::build_dashboard_router()`.**

The dashboard module builds its own `Router` that mounts assets at `/ui` using `nest_service`. This router is merged into the main router. The `build_router()` function gains a single cfg-gated `.merge(dashboard_router)` call — minimal diff to existing code.

**Why not `nest()`:** `Router::nest("/ui", r)` strips the `/ui` prefix before requests reach the nested router. `ServeEmbed` / `rust-embed` handlers need to see the full path to correctly serve assets. Using `nest_service("/ui", ServeEmbed::new())` directly inside the dashboard module avoids this — the prefix stripping is handled by `nest_service` exactly where the service needs it.

---

## Asset Serving: rust-embed + axum-embed

### Crate Decision

**Use `axum-embed` (wraps `rust-embed`) rather than implementing a custom `static_handler`.**

`axum-embed` provides `ServeEmbed<T>` which is a `tower::Service` that handles:
- ETag-based 304 responses (avoids re-serving unchanged assets on hot reload)
- Automatic content-type from file extension
- Brotli/gzip/deflate response compression when client supports it
- Configurable `FallbackBehavior` for SPA index.html fallback routing
- Directory redirect (adds trailing slash)

This eliminates ~50 lines of custom handler code and handles edge cases correctly.

**Confidence:** HIGH — verified against axum-embed docs.rs documentation.

### Asset Embedding Pattern

```rust
// src/dashboard/mod.rs

#[derive(rust_embed::Embed)]
#[folder = "dashboard/dist/"]   // Vite build output directory
struct DashboardAssets;

pub fn build_dashboard_router() -> axum::Router {
    use axum_embed::{FallbackBehavior, ServeEmbed};

    // SPA routing: unknown paths serve index.html so Preact router handles them
    let serve = ServeEmbed::<DashboardAssets>::with_parameters(
        "index.html",                        // index file
        FallbackBehavior::Ok,                // serve index.html with 200 for unknown paths
        Some("index.html".to_string()),      // fallback file
    );

    // nest_service handles prefix stripping: /ui/assets/main.js → assets/main.js
    axum::Router::new()
        .nest_service("/ui", serve)
}
```

### Why FallbackBehavior::Ok (Not 404)

The Preact SPA uses client-side routing. If a user navigates to `/ui/memories/abc123` and refreshes, the server must serve `index.html` (not a 404) so the SPA JavaScript can take over and render the correct view. `FallbackBehavior::Ok` returns `index.html` with HTTP 200 for any path that does not match a real embedded file. This is the standard SPA deployment pattern.

### Build Output Structure

Vite builds Preact apps to `dist/` by default:

```
dashboard/
├── dist/               ← Vite build output (embedded by rust-embed)
│   ├── index.html
│   └── assets/
│       ├── index-[hash].js
│       └── index-[hash].css
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   └── components/
├── package.json
├── vite.config.ts
└── tsconfig.json
```

`rust-embed` embeds everything in `dashboard/dist/` at compile time when building in release mode (or when `debug-embed` feature is set). In debug mode, it reads from disk — enabling faster iteration without recompilation.

**IMPORTANT:** The `dashboard/dist/` directory must exist and contain a built output before `cargo build` succeeds with the `dashboard` feature enabled. The build pipeline must run `npm run build` inside `dashboard/` before `cargo build --features dashboard`.

---

## Feature Gate Pattern

Mirrors the existing `interface-grpc` feature gate pattern already in use:

```toml
# Cargo.toml [features]
dashboard = ["dep:rust-embed", "dep:axum-embed"]
```

```rust
// src/main.rs — no change needed (build_router handles the cfg internally)

// src/server.rs — build_router() modification
#[cfg(feature = "dashboard")]
mod dashboard;  // or in main.rs: mod dashboard;

pub fn build_router(state: AppState) -> Router {
    let protected = /* ... unchanged ... */;
    let public = /* ... unchanged ... */;

    #[cfg(feature = "dashboard")]
    let dashboard_router = dashboard::build_dashboard_router();

    let mut router = Router::new()
        .merge(protected)
        .merge(public);

    #[cfg(feature = "dashboard")]
    { router = router.merge(dashboard_router); }

    router.with_state(state)
}
```

**Why not a `build_router_with_dashboard()` separate function:** The `#[cfg]` block inside the existing `build_router()` is the minimal diff approach. A separate function would require `main.rs` to branch on the feature flag, spreading the dashboard logic across two files. Keeping it inside `build_router()` means the serve() call site in `main.rs` is untouched.

---

## Auth Flow for the Dashboard

### The Problem

The existing `auth_middleware` only runs on routes covered by `route_layer`. Dashboard routes (`/ui/*`) are outside the protected router — they are static files and do not need server-side auth enforcement. However, the Preact SPA makes XHR/fetch calls to the existing `/memories*` and `/keys*` REST endpoints, which DO have auth enforcement.

### Solution: Token Stored in Browser, Passed as Header

The dashboard is an operational tool, not a public web app. Auth design is kept simple:

1. **Dashboard serves unauthenticated** (just HTML/JS/CSS). The page itself has no auth gate.
2. **The SPA prompts for an API key** on first load if it receives a 401 from any API call.
3. **The SPA stores the key in `localStorage`** (or `sessionStorage` for session-only) and sends it as `Authorization: Bearer mnk_...` on every fetch to `/memories`, `/keys`, etc.
4. **The existing `auth_middleware` enforces it** — no changes needed.

This means:
- No new auth middleware needed for `/ui` routes
- The existing protected endpoints enforce auth exactly as before
- The dashboard is "open" at the HTML level — this is acceptable because the API endpoints are still gated
- In "open mode" (no keys), the SPA works without any token

### Why Not Cookie Auth

Cookies would require:
- A login endpoint (`POST /ui/session`) that sets a `Set-Cookie` header
- CSRF protection (cookies on same origin are sent automatically, enabling CSRF)
- Session management (expiry, revocation)

This is significant scope expansion for what is explicitly an operational dashboard. The existing `mnk_...` bearer token model is sufficient — operators who can access the dashboard already have the API key.

### Auth Flow Diagram

```
Browser (Preact SPA at /ui)
    │
    │  1. GET /ui  → 200 index.html (no auth check)
    │  2. JS loads, SPA initializes
    │  3. GET /memories?limit=10
    │         │
    │         ▼
    │    auth_middleware (existing, unchanged)
    │         │  ← checks Authorization: Bearer header
    │         │  ← open mode (no keys): passes through
    │         │  ← auth mode (keys exist): validates token
    │         ▼
    │    list_memories_handler (existing, unchanged)
    │         │
    │         ▼
    │    200 {memories: [...]}
    │
    │  If 401 received:
    │    ├── SPA shows "Enter API Key" prompt
    │    ├── User enters mnk_xxx token
    │    ├── SPA stores in localStorage
    │    └── Retries request with Authorization header
```

---

## Data Flow: UI to API

The SPA communicates exclusively with the existing REST endpoints. No new API endpoints are needed for the v1.6 dashboard. All data flows through existing handlers.

### Dashboard Feature → REST Endpoint Mapping

| Dashboard Feature | REST Endpoint | Notes |
|-------------------|--------------|-------|
| Browse memories | `GET /memories?agent_id=&session_id=&limit=` | Uses existing ListParams |
| Search memories | `GET /memories/search?q=&agent_id=` | Uses existing SearchParams |
| Delete memory | `DELETE /memories/{id}` | Existing endpoint |
| Agent breakdown | `GET /memories?limit=1000` + client-side group by agent_id | No dedicated endpoint needed for v1.6 |
| Session breakdown | `GET /memories?agent_id=x` + group by session_id | Same |
| Storage overview | `GET /health` | Returns backend name; counts from list |
| Trigger compaction | `POST /memories/compact` | Uses existing CompactRequest |
| Dry-run preview | `POST /memories/compact` with `dry_run: true` | Existing dry_run field |
| View API keys | `GET /keys` | Existing endpoint |

**Key insight:** The dashboard adds no new API surface. All REST endpoints already exist and are already auth-gated. The frontend is a consumer of the existing API — identical to any external API client.

### SPA Request/Response Flow

```
Preact Component (e.g., MemoryList)
    │
    │  useEffect / fetch
    ▼
fetch("/memories?agent_id=my-agent&limit=50", {
    headers: { "Authorization": "Bearer " + storedToken }
})
    │
    ▼
axum Router (port 8080)
    │
    ▼
auth_middleware (route_layer on /memories*)
    │  ← validates token (unchanged behavior)
    ▼
list_memories_handler (unchanged handler)
    │
    ▼
Arc<MemoryService>::list_memories(params)
    │
    ▼
Arc<dyn StorageBackend>::list(params)
    │
    ▼
JSON response → browser → rendered by Preact component
```

---

## New File Structure

```
mnemonic/
├── dashboard/                    # NEW: Preact/Vite frontend project
│   ├── dist/                     # NEW: Vite build output (git-ignored, embedded at compile time)
│   │   ├── index.html
│   │   └── assets/
│   │       ├── index-[hash].js
│   │       └── index-[hash].css
│   ├── src/
│   │   ├── main.tsx              # Preact entry point
│   │   ├── App.tsx               # Root component with routing
│   │   ├── api/
│   │   │   └── client.ts         # Fetch wrapper: injects auth header, handles 401
│   │   └── components/
│   │       ├── MemoryList.tsx
│   │       ├── SearchBar.tsx
│   │       ├── AgentBreakdown.tsx
│   │       ├── CompactionPanel.tsx
│   │       └── KeyPrompt.tsx     # Modal: "Enter API key"
│   ├── package.json
│   ├── vite.config.ts
│   └── tsconfig.json
└── src/
    ├── dashboard/                # NEW: Rust module (cfg-gated)
    │   └── mod.rs                # build_dashboard_router(), EmbeddedAssets struct
    ├── server.rs                 # MODIFIED: cfg-gated merge in build_router()
    ├── config.rs                 # UNCHANGED (no new config fields needed)
    ├── main.rs                   # UNCHANGED (serve() call is unchanged)
    └── ...                       # All other files UNCHANGED
```

**Structure rationale:**
- `dashboard/` at project root keeps the frontend entirely separate from Rust source. It is a standalone Node project with its own `package.json`, `node_modules`, and build toolchain.
- `src/dashboard/mod.rs` is the minimal Rust bridge: `#[derive(Embed)]` on the dist folder + `build_dashboard_router()` function. All Rust-side logic lives here so `server.rs` changes are a one-liner.
- `dashboard/dist/` is git-ignored. The build pipeline produces it; it is never committed.

---

## Architectural Patterns

### Pattern 1: Feature-Gated Router Extension via merge()

**What:** The dashboard module exposes a single public function `build_dashboard_router() -> Router`. The caller (`server::build_router`) conditionally merges it. No axum state is passed to the dashboard router — it serves static files only.

**When to use:** Any feature-gated capability that extends the axum router without needing access to AppState. Static file serving is the canonical case.

**Trade-offs:**
- Pro: Minimal diff to `server.rs`. Zero risk to existing routes.
- Pro: Feature can be removed by dropping the feature flag — no code rot.
- Con: The dashboard module is structurally separate from the API. Any future dashboard endpoints that need AppState (e.g., a `/ui/api/` prefix) require passing state to the dashboard router.

**Example:**
```rust
// src/dashboard/mod.rs
#[cfg(feature = "dashboard")]
#[derive(rust_embed::Embed)]
#[folder = "dashboard/dist/"]
struct DashboardAssets;

#[cfg(feature = "dashboard")]
pub fn build_dashboard_router() -> axum::Router {
    use axum_embed::{FallbackBehavior, ServeEmbed};
    let serve = ServeEmbed::<DashboardAssets>::with_parameters(
        "index.html",
        FallbackBehavior::Ok,
        Some("index.html".to_string()),
    );
    axum::Router::new().nest_service("/ui", serve)
}
```

### Pattern 2: SPA with index.html Fallback via FallbackBehavior::Ok

**What:** All requests to `/ui/*` that do not match an embedded asset file are served `index.html` with HTTP 200. The Preact router in JavaScript handles the route client-side.

**When to use:** Every SPA deployment. Without this, navigating to `/ui/memories` after a page refresh returns a 404 because the server doesn't know about client-side routes.

**Trade-offs:**
- Pro: Standard pattern, works with all client-side routers.
- Pro: `axum-embed`'s `FallbackBehavior` handles this correctly without custom handler code.
- Con: Server cannot distinguish a real 404 (missing asset) from a valid SPA route — all unknown paths return 200. This is acceptable for a dashboard served at a prefix, not the root.

### Pattern 3: Stateless Auth via localStorage + Existing Middleware

**What:** The SPA reads an API key from `localStorage` and sends it as `Authorization: Bearer` on every fetch. The existing axum `auth_middleware` handles validation — no new server-side code.

**When to use:** Operational dashboards where the audience is developers/operators who already have API keys. Not suitable for end-user-facing apps.

**Trade-offs:**
- Pro: Zero new server-side auth code. No sessions, no CSRF, no new endpoints.
- Pro: Works identically to CLI and agent usage — operators use the same keys.
- Pro: In open mode (no keys), the SPA works with zero configuration.
- Con: `localStorage` tokens are accessible to JavaScript (XSS risk). Acceptable for a local/intranet operational tool; not acceptable for a public-facing app.
- Con: No logout mechanism (token persists until cleared manually). Acceptable given the use case.

---

## Component Responsibilities

| Component | Status | Responsibility | Touches |
|-----------|--------|---------------|---------|
| `dashboard/` (Node project) | New | Preact SPA: browse/search memories, agent breakdown, compaction trigger | Frontend only |
| `src/dashboard/mod.rs` | New | rust-embed `#[derive(Embed)]` on `dashboard/dist/`; `build_dashboard_router()` | `axum-embed`, `rust-embed` |
| `src/server.rs` | Modified | `#[cfg(feature="dashboard")]` merge in `build_router()` | 3-line change |
| `Cargo.toml` | Modified | `dashboard` feature flag with `rust-embed`, `axum-embed` deps | Build config |
| `.gitignore` | Modified | Add `dashboard/dist/`, `dashboard/node_modules/` | Build artifacts |
| `src/main.rs` | Unchanged | `serve()` call is unchanged | None |
| `src/config.rs` | Unchanged | No new config fields needed | None |
| `src/auth.rs` | Unchanged | Existing auth_middleware handles dashboard API requests | None |
| All other `src/` files | Unchanged | REST/gRPC/storage/embedding logic unaffected | None |

---

## Build Order

Implementation phases, each producing a testable increment:

| Phase | What Gets Built | Dependencies | Testable Outcome |
|-------|-----------------|-------------|------------------|
| 1 | `dashboard/` Vite+Preact+Tailwind scaffold; `vite.config.ts` with `base: "/ui"`; `dist/` as build output; `package.json` scripts | None (frontend only) | `npm run build` produces `dashboard/dist/index.html` |
| 2 | `Cargo.toml` dashboard feature flag + `rust-embed`/`axum-embed` deps; `src/dashboard/mod.rs` with `build_dashboard_router()`; `build_router()` cfg-gated merge | Phase 1 dist exists | `cargo build --features dashboard` succeeds; `GET /ui` returns index.html |
| 3 | `dashboard/src/api/client.ts` fetch wrapper with auth header injection + 401 handling; `KeyPrompt` component; integration with `GET /health` | Phase 2 | Dashboard loads, prompts for key in auth mode, calls `/health` successfully |
| 4 | Memory list view: `MemoryList` component, pagination, agent/session/tag filter UI calling `GET /memories` | Phase 3 | Can browse memories from dashboard |
| 5 | Search view: `SearchBar` + results calling `GET /memories/search` | Phase 4 | Semantic search from dashboard |
| 6 | Agent + session breakdown views: group memory list by agent_id/session_id, show counts | Phase 4 | Activity overview visible |
| 7 | Compaction panel: dry-run preview + trigger calling `POST /memories/compact` | Phases 4-6 | Visual compaction workflow complete |
| 8 | CI integration: `npm run build` step before `cargo build --features dashboard` in release workflow | Phases 1-7 | GitHub Actions produces dashboard-enabled binary |

**Phase ordering rationale:**
- Phase 1 first: `dashboard/dist/` must exist for `cargo build --features dashboard` to succeed. The Rust build fails if the embedded folder is missing.
- Phase 2 before Phase 3: Verify the embedded serving works (index.html loads) before building any interactive components. Static serving is the riskiest integration point.
- Phase 3 establishes the API client pattern: all subsequent phases reuse the same fetch wrapper, so it must be solid before data-fetching components are built.
- Phases 4-7 are independent of each other in terms of Rust code — they only add Preact components. Order follows logical user flow: browse → search → analyze → act.
- Phase 8 last: CI integration is a wrapper around the completed build process.

---

## Anti-Patterns

### Anti-Pattern 1: Adding the Dashboard Router to the Protected Sub-Router

**What people do:** Add `.nest_service("/ui", ...)` inside the `protected` router block in `build_router()`.

**Why it's wrong:** The `protected` router is wrapped with `route_layer(auth_middleware)`. Every `/ui/*` request would then require a valid API key — including the initial page load that fetches `index.html`. This breaks the open-access model for the dashboard HTML itself.

**Do this instead:** Add the dashboard router to the top-level router via `.merge(dashboard_router)` alongside `protected` and `public`, not inside either of them.

### Anti-Pattern 2: Serving Dashboard from a New Binary Feature Port

**What people do:** Add a third port and a third `tokio::try_join!` arm for the dashboard.

**Why it's wrong:** The dashboard's only function is to consume the existing REST API. Serving its static files from a different port creates cross-origin requests (CORS), breaks same-origin cookies (if ever needed), and complicates firewall rules. Static file serving is an axum one-liner — it does not need its own listener.

**Do this instead:** Serve dashboard assets on the same port as the REST API via `nest_service("/ui", ...)`. The SPA makes same-origin requests to `/memories`, `/keys`, etc. — no CORS needed.

### Anti-Pattern 3: Committing dashboard/dist/ to Git

**What people do:** Run `npm run build`, then `git add dashboard/dist/`, and commit the built output.

**Why it's wrong:** Built artifacts change on every build (content hashes in filenames). They inflate the repository, cause noisy diffs, and create merge conflicts. The dist output should be reproducible from source.

**Do this instead:** Add `dashboard/dist/` and `dashboard/node_modules/` to `.gitignore`. The CI pipeline runs `npm run build` before `cargo build --features dashboard`. Local development does the same. The `debug-embed` feature of `rust-embed` can be omitted so in debug mode, assets are read from disk (no recompile needed when the frontend changes).

### Anti-Pattern 4: New Endpoints Specifically for the Dashboard

**What people do:** Create a `/ui/api/` prefix with new, dashboard-specific API endpoints that return pre-shaped data (e.g., "agent summary" aggregate endpoint).

**Why it's wrong:** This duplicates API surface, creates two sets of endpoints to maintain, and diverges from the principle that the dashboard is "just another API client." Aggregations that are cheap enough to run in JavaScript (grouping a few hundred memories by agent_id) should be done client-side.

**Do this instead:** The SPA calls the existing REST API and performs any client-side aggregation in JavaScript. If a query genuinely requires server-side aggregation for performance (e.g., COUNT by agent_id over 100K memories), add a proper endpoint to the main REST API behind its own feature discussion — not a dashboard-specific one.

### Anti-Pattern 5: Vite Dev Server Proxying to Different Port

**What people do:** Configure Vite's dev server proxy to point to `http://localhost:8080` and serve the SPA from `http://localhost:5173` during development. This works locally but does not test the actual embedded serving path.

**Why it's wrong:** The production code path serves assets at `/ui` from the same process as the API. The dev proxy setup means assets are served from a different origin than the API — the SPA is never tested as it will run in production (same origin, `/ui` prefix, embedded in binary).

**Do this instead:** For development, use `rust-embed`'s disk-read behavior (default in debug mode) so `cargo run --features dashboard` serves `dashboard/dist/` from disk without embedding. Run `npm run dev -- --base /ui` in one terminal and `cargo run --features dashboard` in another — the Rust server serves the Vite dev output. OR use the Vite proxy approach but add an integration test that exercises the embedded path.

---

## Integration Points: New vs Modified Components

| Component | New or Modified | Touch Surface |
|-----------|----------------|---------------|
| `dashboard/` (Node project) | New | No Rust files touched |
| `src/dashboard/mod.rs` | New | Only imported from `server.rs` behind `#[cfg]` |
| `src/server.rs` | Modified | `build_router()` gains ~5 lines behind `#[cfg(feature="dashboard")]` |
| `Cargo.toml` | Modified | New `dashboard` feature entry, 2 optional deps |
| `.gitignore` | Modified | Add `dashboard/dist/`, `dashboard/node_modules/` |
| `src/main.rs` | Unchanged | `serve()` call is identical |
| `src/config.rs` | Unchanged | No config fields needed |
| `src/auth.rs` | Unchanged | Existing `auth_middleware` handles dashboard API calls |
| `src/server.rs` `AppState` | Unchanged | No new fields; dashboard has no AppState dependency |
| All gRPC code | Unchanged | Dashboard is REST-only; gRPC untouched |
| All storage backends | Unchanged | Dashboard is a consumer, not a storage layer concern |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|--------------|-------|
| `server::build_router` ↔ `dashboard::build_dashboard_router` | Direct function call, returns `axum::Router` | No AppState passed; dashboard router is stateless |
| Preact SPA ↔ REST API | HTTP fetch on same origin | `Authorization: Bearer` header injected by `api/client.ts` |
| `api/client.ts` ↔ `localStorage` | Browser API | Token persisted across page loads |
| `auth_middleware` ↔ dashboard API calls | Unchanged — SPA calls are ordinary HTTP requests | No code change needed |

---

## Vite Configuration Detail

The SPA must be built with `base: "/ui"` so asset paths in `index.html` are prefixed correctly:

```typescript
// dashboard/vite.config.ts
import { defineConfig } from 'vite'
import preact from '@preact/preset-vite'

export default defineConfig({
    plugins: [preact()],
    base: '/ui',              // CRITICAL: asset paths must match the mount point
    build: {
        outDir: 'dist',
        emptyOutDir: true,
    }
})
```

Without `base: "/ui"`, Vite generates `<script src="/assets/index.js">` (absolute from root). When served at `/ui`, the browser requests `/assets/index.js` which hits the axum router and returns 404. With `base: "/ui"`, Vite generates `<script src="/ui/assets/index.js">` which resolves correctly.

**Confidence:** HIGH — verified against Vite `base` option documentation behavior.

---

## Scaling Considerations

| Scale | Architecture Notes |
|-------|-------------------|
| Single developer | Debug mode: `rust-embed` reads from disk. `npm run build` then `cargo run --features dashboard`. Fast iteration. |
| Production deployment | Release binary with `--features dashboard`. Assets embedded at ~100-300KB total (Preact + Tailwind purged). No disk access for static files. |
| High request volume | Dashboard is static files — axum-embed returns embedded bytes from memory with ETag caching. Zero I/O, zero database calls for asset serving. |
| CI pipeline | `npm ci && npm run build` → `cargo build --release --features dashboard`. Both steps needed. |

---

## Sources

- [rust-embed official axum example](https://docs.rs/crate/rust-embed/latest/source/examples/axum.rs) — EmbeddedAssets pattern, StaticFile handler
- [axum-embed docs.rs](https://docs.rs/axum-embed/latest/axum_embed/) — ServeEmbed, FallbackBehavior enum, SPA configuration
- [axum-embed crates.io](https://crates.io/crates/axum-embed) — ETag, compression, fallback features
- [memory-serve docs.rs](https://docs.rs/memory-serve/latest/memory_serve/) — Alternative with fallback/index_file pattern (compared, not used)
- [axum Router docs.rs](https://docs.rs/axum/latest/axum/struct.Router.html) — nest_service, merge, route_layer behavior
- [marending.dev: How to host single-page applications with Rust](https://www.marending.dev/notes/rust-spa/) — rust-embed performance rationale (3x faster than disk)
- [nguyenhuythanh.com: Using Rust Backend To Serve An SPA](https://nguyenhuythanh.com/posts/rust-backend-spa/) — static_handler fallback pattern for SPA routing
- [GitHub tokio-rs/axum discussion #1309](https://github.com/tokio-rs/axum/discussions/1309) — serving SPA files and embed files in executable
- [Vite build configuration docs](https://vite.dev/guide/build) — base option, outDir, asset path generation
- [axum-extra CookieJar docs](https://docs.rs/axum-extra/latest/axum_extra/extract/cookie/struct.CookieJar.html) — cookie auth comparison (decided against)
- [GitHub gist: axum auth accepts Authorization header or cookie](https://gist.github.com/ezesundayeze/c0dd6471b2aed1199feff187b485fb02) — header-or-cookie pattern (decided not needed)

---
*Architecture research for: Mnemonic v1.6 embedded web dashboard*
*Researched: 2026-03-22*
