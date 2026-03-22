# Project Research Summary

**Project:** Mnemonic v1.6 — Embedded Web Dashboard
**Domain:** Embedded operational web dashboard in a Rust single-binary agent memory server
**Researched:** 2026-03-22
**Confidence:** HIGH

## Executive Summary

Mnemonic v1.6 adds a feature-gated operational web dashboard to an already mature Rust binary (v1.5: axum 0.8, tonic, SQLite/Qdrant/Postgres backends, REST + gRPC, API key auth). The domain is well-understood: embed a compiled Preact SPA into the binary at compile time via `rust-embed`, serve it at `/ui` on the existing axum port using `axum-embed`, and wire the frontend exclusively to the existing 9-endpoint REST API. The single-binary constraint is non-negotiable — any pattern that requires a separate Node.js process, separate port, or runtime filesystem access is disqualified. The entire dashboard must be an additive Cargo feature (`--features dashboard`) with zero impact on the default binary.

The recommended approach is `rust-embed 8.11` + `axum-embed 0.1` on the Rust side, and Preact 10.x + Vite 6.x + Tailwind v4 + `vite-plugin-singlefile` on the frontend side. `vite-plugin-singlefile` collapses the entire SPA into a single `index.html`, eliminating asset-path routing complexity and making rust-embed integration trivial. The existing REST API surface covers all dashboard features without any new endpoints except one: `GET /stats` for per-agent memory counts and last-active timestamps (required for the agent breakdown view). The dashboard's auth model is stateless — the SPA detects 401 responses, prompts for the `mnk_...` bearer token, stores it in-memory (not localStorage), and injects it as an `Authorization: Bearer` header on every fetch.

The primary risks are build pipeline ordering (frontend must be built before `cargo build --features dashboard`), conditional compilation boundaries leaking between feature variants, and a class of hard-to-spot UI bugs (Tailwind class purging of dynamically constructed names, Vite base-path misconfiguration, SPA hard-reload 404s). All of these are preventable with explicit setup choices made in Phase 1. None require novel patterns — they are well-documented Rust+SPA integration pitfalls with clear mitigations.

---

## Key Findings

### Recommended Stack

The existing v1.5 stack (axum 0.8, tokio, tonic, rusqlite, sqlx, candle, etc.) is unchanged. V1.6 introduces exactly two new Rust crate dependencies, both optional behind the `dashboard` feature flag: `rust-embed 8.11` (compile-time asset embedding with axum feature for ETag/MIME support) and `axum-embed 0.1` (tower Service wrapping rust-embed with SPA fallback, brotli/gzip compression negotiation, and directory redirect). On the JavaScript side, Preact 10.x is the unambiguous choice — 3 KB gzipped vs React's 42 KB, identical JSX/hooks API. Vite 6.x with `@preact/preset-vite` and `vite-plugin-singlefile` produces a single `index.html` with all JS and CSS inlined, which is the simplest possible input for rust-embed. Tailwind v4 (no config file, `@import "tailwindcss"` only) handles styling.

**Core technologies:**
- `rust-embed 8.11`: compile-time asset embedding — industry standard, `features = ["axum"]` for MIME/ETag, compatible with axum 0.8
- `axum-embed 0.1`: axum `ServeEmbed<T>` service — eliminates ~50 lines of custom handler code, handles SPA fallback and compression
- `Preact 10.x`: UI framework — 3 KB gzipped, React-compatible API, first-class Vite preset
- `Vite 6.x + @preact/preset-vite`: build + dev server — HMR, optimized production output, Preact-maintained preset
- `vite-plugin-singlefile`: single-file output — eliminates hashed filenames, simplifies rust-embed integration to one file
- `Tailwind v4 + @tailwindcss/vite`: utility CSS — v4 is current, no config file required, Vite plugin available
- Node.js 22 LTS: build-time only — required only when building with `--features dashboard`, not in the final binary

### Expected Features

The feature set maps cleanly onto mnemonic's existing REST API. Every table-stakes feature is achievable with existing endpoints. The only new server-side work is `GET /stats` (a single GROUP BY query).

**Must have (table stakes) — v1.6 launch:**
- Memory list table with content preview, agent_id, session_id, tags, created_at, and pagination — core browser value
- Filter bar: agent_id, session_id, tag — drives 80% of actual usage
- Semantic search bar (`GET /memories/search`) — showcases mnemonic's core capability
- Memory detail view (row expand) — required for full content inspection
- Delete memory action with confirmation modal — needed to clean up test data
- Health indicator + active backend badge in header (`GET /health`) — operational awareness at a glance
- Agent breakdown table (per-agent count + last-active) — requires new `GET /stats` endpoint
- Compaction panel: dry-run preview then execute (`POST /memories/compact`) — only write-side operational action in scope
- Auth-aware: detect 401, prompt for `mnk_...` key, store in-memory, persist per-session — required for any auth-enabled deployment
- Feature gate behind `dashboard` Cargo feature — architectural requirement, zero binary impact when off

**Should have (differentiators) — competitive advantage:**
- Compaction dry-run diff view (before/after N→M mapping) — no peer tool surfaces this visually
- Zero-config UI access (no separate installation, just `/ui`) — genuine single-binary differentiator vs Qdrant/RedisInsight
- Content preview in table (first 80 chars) — eliminates click-expand round trip for quick scanning
- Tag badges in list rows — colored pills make filtered browsing faster
- Backend badge in header as persistent pill — unambiguous backend display at all times

**Defer (v2+):**
- Vector visualization (2D UMAP projection) — heavyweight JS + server-side compute, no demand yet
- Dark/light theme toggle — operational tool, not consumer product; ship one default theme
- Memory edit form — high corruption risk; CLI/API are the correct write paths
- Bulk delete — dangerous without undo; individual delete + compaction covers the use case

### Architecture Approach

The dashboard integrates via a single `#[cfg(feature = "dashboard")]`-gated `.merge(dashboard_router)` call inside the existing `build_router()` function in `server.rs`. The dashboard module (`src/dashboard/mod.rs`) is entirely self-contained: it holds the `#[derive(Embed)]` struct over `dashboard/dist/` and exposes a single `build_dashboard_router() -> Router` function that mounts assets at `/ui` using `nest_service`. No new port, no new `tokio::try_join!` arm, no new AppState fields, and no changes to `main.rs`. The frontend (`dashboard/` at project root) is a standalone Node project. The Rust side is a minimal bridge (~20 lines). All other v1.5 files are untouched.

**Major components:**
1. `dashboard/` (Node project) — Preact SPA consuming existing REST API; `dist/` is git-ignored, generated at build time
2. `src/dashboard/mod.rs` (new Rust module) — `#[derive(Embed)]` on `dashboard/dist/`, `build_dashboard_router()` returning `axum::Router` with `nest_service("/ui", ServeEmbed<Assets>)`
3. `src/server.rs` (3-line modification) — `#[cfg(feature="dashboard")]`-gated merge of dashboard router into existing `build_router()`
4. `dashboard/src/api/client.ts` (new frontend module) — fetch wrapper injecting `Authorization: Bearer` header, catching 401 and triggering `KeyPrompt` modal
5. `GET /stats` endpoint (new, minimal) — single GROUP BY SQL query returning per-agent count + last_active; gated behind same auth middleware as `/memories`

### Critical Pitfalls

1. **Vite base path misconfiguration** — Set `base: '/ui/'` in `vite.config.ts` from day one; without it, asset paths in `dist/index.html` reference `/assets/...` instead of `/ui/assets/...` and every JS/CSS load returns 404 when served from the binary.

2. **Feature flag non-additivity (rust-embed proc-macro panics on missing dist/)** — The `#[derive(RustEmbed)]` struct must be inside `#[cfg(feature = "dashboard")]`; `dashboard/dist/` must exist before `cargo build --features dashboard` runs; add a `build.rs` guard that emits `compile_error!` if the feature is active but `dist/index.html` is missing.

3. **Auth middleware applied to static asset routes** — Dashboard routes must live in a separate router that is merged at the top level (not nested inside the `protected` router); applying `route_layer()` auth to `/ui/*` breaks the initial page load, which cannot send bearer tokens.

4. **Tailwind class purging of dynamic names** — Never construct class names via string interpolation (`` `text-${color}-500` ``); always write full literal names; add a safelist for unavoidably dynamic patterns; verify production build visually before shipping.

5. **SPA hard-reload 404 on sub-routes** — Use hash routing (`/#/memories`) rather than history routing; this eliminates the entire failure class at zero cost and is appropriate for an operational dashboard with no deep-linking requirements.

6. **API key in localStorage** — Store the bearer token in Preact component state only (in-memory); never `localStorage.setItem` with an `mnk_...` token; add a CSP header (`default-src 'self'; script-src 'self'`) to all `/ui/` responses.

7. **Frontend build not in CI release workflow** — Add `actions/setup-node` + `npm ci && npm run build` steps (working-directory: `dashboard/`) to the release matrix before the `cargo build` step; missing this produces a release binary with a 0-byte embedded UI.

---

## Implications for Roadmap

Based on combined research, a 3-phase structure is recommended. All phases are purely additive — each leaves the default binary untouched.

### Phase 1: Foundation — Build Pipeline, Embedding, Router Integration

**Rationale:** rust-embed's proc-macro embeds files at compile time; `dashboard/dist/` must exist before any Rust dashboard code compiles. The Vite base path, feature flag boundaries, CI pipeline, SPA routing strategy, and router structure must all be correct before writing a single UI component. Getting these wrong after UI development is underway is expensive to fix. All 12 critical pitfalls identified in PITFALLS.md have their root cause in Phase 1 decisions.

**Delivers:**
- `dashboard/` Node project scaffolded (Vite + Preact + Tailwind + `vite-plugin-singlefile`)
- `vite.config.ts` with `base: '/ui/'`, production build settings, `NODE_ENV=production`
- `Cargo.toml` `dashboard` feature gate with `rust-embed` + `axum-embed` optional deps
- `src/dashboard/mod.rs` with `#[cfg(feature="dashboard")]`-gated `ServeEmbed` and `build_dashboard_router()`
- `src/server.rs` 3-line `#[cfg]`-gated merge
- `build.rs` prerequisite guard (emit `compile_error!` if feature active but `dist/` missing)
- `.gitignore` additions: `dashboard/dist/`, `dashboard/node_modules/`
- CI release workflow: `setup-node` + `npm ci && npm run build` before `cargo build --release --features dashboard`
- Separate CI job: `cargo build` (default features) + `cargo test` — regression gate for default binary
- Smoke tests: `GET /ui/` returns 200 + `text/html`; `GET /ui/` with no auth header returns 200 even in auth mode; `cargo build` without feature passes

**Avoids:** Pitfalls 2, 3, 8, 9, 10, 11, 12 (rust-embed divergence, auth middleware on assets, feature flag leakage, missing CI build step, Vite base path, middleware mis-scoping)

**Research flag:** Standard patterns — no further research needed. rust-embed + axum-embed integration is well-documented with official examples and working code in STACK.md and ARCHITECTURE.md.

### Phase 2: Core UI — Data Browsing, Search, Auth Flow

**Rationale:** Once embedded serving is verified (Phase 1), the SPA is built against real API data. Auth flow is first because all data-fetching components depend on it. Memory list + search are the primary value delivery — they cover the core use case and exercise the full frontend-to-API data path. Agent breakdown and the `GET /stats` endpoint belong here because they share the same data-fetching patterns as memory list.

**Delivers:**
- `dashboard/src/api/client.ts` — fetch wrapper with `Authorization: Bearer` injection, 401 detection
- `KeyPrompt` component — modal for entering `mnk_...` key, stores in Preact state (in-memory only, never localStorage)
- CSP header on `/ui/` responses: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'`
- Health indicator + backend badge in header (`GET /health`)
- Memory list table: content preview, agent_id, session_id, tags, created_at, pagination
- Filter bar: agent_id, session_id, tag
- Semantic search bar (`GET /memories/search`)
- Memory detail view (row expand or side panel)
- Delete memory action with confirmation modal (`DELETE /memories/{id}`)
- `GET /stats` REST endpoint on the Rust side (GROUP BY agent_id query)
- Agent breakdown table (per-agent count + last-active, from `GET /stats`)
- Cache-Control headers: `no-cache` for `index.html`, `max-age=31536000, immutable` for hashed assets
- Production Tailwind build verification (all conditional styles render correctly in `npm run build` output)

**Avoids:** Pitfalls 1, 4, 5, 6 (SPA routing, Tailwind purge, localStorage token, CORS)

**Research flag:** Standard patterns — Preact + Tailwind component development is well-documented. The `GET /stats` SQL query is a single GROUP BY; no research needed.

### Phase 3: Operational Actions — Compaction Panel and Polish

**Rationale:** Compaction is the only write-side operational action appropriate for a dashboard. It depends on the agent breakdown view (Phase 2) because compaction requires an agent_id scope. This phase is last because it is the most sensitive user-facing flow (destructive action with dry-run preview) and benefits from having the full data-browsing context in place. Polish items (empty states, loading skeletons, error boundaries) are included here to ensure launch quality.

**Delivers:**
- Compaction panel: agent_id selector, dry-run preview (`POST /memories/compact?dry_run=true`), diff display (N memories → M compacted), confirmation modal, execute (`POST /memories/compact`)
- Empty state handling (zero memories, zero agents, zero search results)
- Loading skeleton states for all async data fetches
- Error boundary for unhandled API failures
- Final integration test pass: all features exercised against the release binary with `--features dashboard`

**Avoids:** UX pitfalls — no loading state on first render, blank page crash on empty database, compaction executed without confirmation

**Research flag:** Standard patterns — no further research needed.

### Phase Ordering Rationale

- Phase 1 must precede all others: rust-embed compile-time embedding creates a hard dependency order (frontend build → Cargo build). The CI pipeline and feature flag boundaries must be correct before any UI code is written or they create difficult-to-untangle regressions.
- Phase 2 before Phase 3: the auth flow and data-browsing client pattern must be stable and trusted before building the compaction workflow, which is the only destructive action in the dashboard.
- Grouping Phase 2 as a unit: memory list, search, agent breakdown, and the new `GET /stats` endpoint all share the same fetch pattern and can be reviewed as a cohesive data layer.
- The gRPC interface (v1.5) and all storage backends are untouched across all phases. The minimal Rust surface change (3 lines in `server.rs`) reduces review burden.

### Research Flags

Phases with standard patterns (skip `/gsd:research-phase`):
- **Phase 1:** rust-embed + axum-embed + Vite integration is fully documented in official sources; working code samples for every integration point are provided in STACK.md and ARCHITECTURE.md.
- **Phase 2:** Preact + Tailwind component patterns are standard; the GROUP BY SQL query is trivial; no novel patterns.
- **Phase 3:** Compaction flow uses the existing REST endpoint with `dry_run: true`; no novel patterns.

No phase requires deeper research during planning. The research files contain verified configuration values and working code snippets for every integration point.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | rust-embed 8.11 and axum-embed 0.1 verified against docs.rs; Preact 10.x + Vite 6.x verified against official docs; version compatibility between rust-embed `features = ["axum"]` and mnemonic's axum 0.8 explicitly confirmed; only `vite-plugin-singlefile` is MEDIUM (npm registry source) |
| Features | HIGH | Feature set grounded in competitor analysis (Prometheus UI, Qdrant Web UI, RedisInsight, VectorAdmin, Agent Zero); all API dependencies map to existing endpoints except `GET /stats`; anti-features clearly justified with rationale |
| Architecture | HIGH | Based on direct codebase inspection of v1.5 source (~11,940 lines); integration pattern verified against axum Router docs and axum-embed docs.rs; build order is deterministic and captured in ARCHITECTURE.md build phases |
| Pitfalls | HIGH | 12 critical pitfalls documented; each has authoritative source citation; prevention, warning signs, and recovery strategies all provided; pitfall-to-phase mapping is explicit |

**Overall confidence:** HIGH

### Gaps to Address

- **`GET /stats` with non-SQL backends:** The per-agent count + last_active GROUP BY query works for SQLite/Postgres. Qdrant is a vector DB, not a SQL DB — it will require a different aggregation path (likely N queries or a scroll+group client-side in the storage adapter). This should be handled during Phase 2 implementation by inspecting `src/storage/qdrant.rs` and adding backend-specific logic in the stats handler.

- **`vite-plugin-singlefile` compatibility:** Confidence on this package is MEDIUM (npm registry source only). Verify it produces correct single-file output for the `@preact/preset-vite` + `@tailwindcss/vite` combination before committing to it in Phase 1. The fallback (multi-file output with `axum-embed`'s built-in asset routing) is viable and well-documented if the plugin causes issues.

- **Auth mode detection signal:** `GET /health` alone cannot detect whether auth is currently active (it does not expose key configuration status). The correct signal is a 401 response from `GET /memories` on first load. This is documented in FEATURES.md but must be the explicit first behavior tested in Phase 2 auth flow implementation — not assumed.

- **Binary size delta target:** The expected overhead when enabling `--features dashboard` is under 3 MB. This should be measured after Phase 1 completes with a minimal frontend stub, before Phase 2 adds component complexity, to confirm the `vite-plugin-singlefile` + Preact + Tailwind combination stays within this bound.

---

## Sources

### Primary (HIGH confidence)
- [docs.rs/rust-embed 8.11.0](https://docs.rs/crate/rust-embed/latest) — version, axum feature, debug/release behavior, ETag generation
- [docs.rs/axum-embed 0.1.0](https://docs.rs/axum-embed/latest/axum_embed/) — ServeEmbed API, FallbackBehavior, compression, SPA configuration
- [preactjs.com/guide/v10/getting-started](https://preactjs.com/guide/v10/getting-started/) — bundle size, Vite integration, 10.x stable status
- [tailwindcss.com/docs/installation](https://tailwindcss.com/docs/installation/tailwind-cli) — v4.2 current, `@import "tailwindcss"` pattern, Vite plugin
- [Vite build documentation](https://vite.dev/guide/build) — base option, production build settings, asset path generation
- [Cargo Book — Features](https://doc.rust-lang.org/cargo/reference/features.html) — additive feature requirement, feature unification
- [Effective Rust, Item 26](https://effective-rust.com/features.html) — feature flag pitfalls, additive requirement
- [tailwindlabs/tailwindcss GitHub discussion #7568](https://github.com/tailwindlabs/tailwindcss/discussions/7568) — dynamic class purge behavior in v3 and v4
- [Auth0 Token Storage best practices](https://auth0.com/docs/secure/security-guidance/data-security/token-storage) — localStorage XSS risk, in-memory recommendation
- [MDN CORS guide](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CORS) — wildcard origin + credentials incompatibility
- [axum Router docs.rs](https://docs.rs/axum/latest/axum/struct.Router.html) — nest_service, merge, route_layer behavior
- [memory-serve docs.rs](https://docs.rs/memory-serve) — compile-time brotli, ETag headers (evaluated as alternative)

### Secondary (MEDIUM confidence)
- [github.com/preactjs/preset-vite](https://github.com/preactjs/preset-vite) — @preact/preset-vite 2.10.3 compatibility
- [npmjs.com/package/vite-plugin-singlefile](https://www.npmjs.com/package/vite-plugin-singlefile) — single-file inlining behavior
- [pyrossh/rust-embed GitHub issues #50](https://github.com/pyrossh/rust-embed/issues/50) — debug-embed feature behavior
- [itmecho.com/blog/rust-embedded-client](https://itmecho.com/blog/rust-embedded-client) — build.rs + rust-embed + axum integration pattern
- [marending.dev — How to host SPA files in Rust](https://www.marending.dev/notes/rust-spa/) — rust-embed performance rationale
- [Qdrant Web UI documentation](https://qdrant.tech/documentation/web-ui/) — competitor feature analysis
- [VectorAdmin GitHub](https://github.com/Mintplex-Labs/vector-admin) — competitor feature analysis
- [Agent Zero Memory Dashboard (DeepWiki)](https://deepwiki.com/agent0ai/agent-zero/5.6-memory-dashboard) — competitor feature analysis
- [RedisInsight documentation](https://redis.io/docs/latest/develop/tools/insight/) — competitor feature analysis
- [Dashboard UX patterns — Pencil & Paper](https://www.pencilandpaper.io/articles/ux-pattern-analysis-data-dashboards) — filter-then-paginate, status-first layout
- [GitHub tokio-rs/axum discussion #1309](https://github.com/tokio-rs/axum/discussions/1309) — SPA hosting with embedded files, confirmed axum 0.7+ pattern

---
*Research completed: 2026-03-22*
*Ready for roadmap: yes*
