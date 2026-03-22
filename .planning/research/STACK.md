# Stack Research

**Domain:** Embedded web dashboard in Rust binary (Preact + Tailwind, served from axum)
**Researched:** 2026-03-22
**Confidence:** HIGH (Rust crates verified via docs.rs; JS tooling verified via official docs and npm)

## Context

This is a **subsequent milestone** stack for v1.6. The existing validated stack (axum 0.8, tokio 1, tonic 0.13, prost 0.13, rusqlite 0.37, sqlx 0.8, candle, etc.) is unchanged. This document covers only the **new dependencies required for the embedded dashboard**.

Existing validated capabilities that are NOT re-researched here: axum REST API, tonic gRPC, SQLite/Qdrant/Postgres backends, API key auth, CLI subcommands.

---

## New Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `rust-embed` | `8.11.0` | Embeds compiled frontend assets into binary at compile time | Industry standard for this pattern. Dev mode reads from filesystem (no rebuild for HTML/CSS tweaks); release mode inlines all bytes into the binary. Ships with optional `features = ["axum"]` which provides ETag generation and MIME detection — matching existing axum 0.8. Released 2026-01-14. |
| `axum-embed` | `0.1.0` | Axum `Service` wrapper over rust-embed | Implements `ServeEmbed<T>` — a proper `nest_service`-compatible tower `Service`. Adds ETag-based 304 caching, `Accept-Encoding`-aware Brotli/gzip/deflate serving, SPA fallback, and directory redirect. Requires `rust-embed ^8`. Zero handler boilerplate. |
| Preact | `10.x` (stable; 11.0.0-beta.1 not yet stable) | UI framework | 3–4 KB min+gzip vs React's 42 KB. Identical JSX/hooks API — no developer context switch. Vite provides a first-class official preset. Correct choice for an embedded operational dashboard where bundle size matters. |
| Vite | `6.x` (current) | Frontend bundler + dev server | Official `@preact/preset-vite` plugin. HMR during development without binary recompiles. Production builds via Rollup/Rolldown produce optimized, tree-shaken output. The standard choice for Preact projects in 2026. |
| Tailwind CSS | `v4.2` (current) | Utility-first CSS | v4 dropped `tailwind.config.js`; uses `@import "tailwindcss"` in CSS only. Standalone binary available from GitHub releases — no Node.js required on machines that don't already have it. |

---

## New Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `@preact/preset-vite` | `2.10.3` | Vite plugin for Preact | Always — configures JSX transform, Preact Devtools bridge, tree-shaking. Official Preact-maintained. |
| `vite-plugin-singlefile` | latest | Inline all JS/CSS into `index.html` | Always for this use case — produces one `index.html` with all assets inline. Eliminates hashed filenames (`main.a1b2c3.js`), removes need for wildcard asset routing in rust-embed, and reduces rust-embed embedding to a single file. |
| `@tailwindcss/vite` | `4.x` | Tailwind v4 Vite integration | Preferred over PostCSS approach for Vite projects — direct Vite plugin, no separate PostCSS config. |
| `mime_guess` | `2.0` | MIME type detection | Transitive dependency of rust-embed's axum feature. Only add directly if writing a custom handler instead of axum-embed. |

---

## Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| Node.js (LTS 22) | npm + Vite build | Required on developer machines and CI when building with `--features dashboard`. Not needed for default binary builds. |
| `npm` / `npm ci` | Dependency installation | Standard. Use `npm ci` in CI for deterministic installs. |
| Tailwind standalone CLI | CSS build without Node.js | Optional alternative to `@tailwindcss/vite`. Download binary from GitHub releases. Useful for minimal CI setups, but Vite integration is simpler when Node.js is already present. |

---

## Cargo.toml Changes

```toml
[features]
dashboard = ["dep:rust-embed", "dep:axum-embed"]

[dependencies]
# --- NEW for v1.6 dashboard ---
rust-embed = { version = "8.11", optional = true, features = ["axum"] }
axum-embed = { version = "0.1", optional = true }
```

The `features = ["axum"]` on rust-embed enables built-in Content-Type headers and ETag generation. `axum-embed` wraps this into `ServeEmbed<T>` — a single line to mount on the router.

Both are `optional = true` and behind the `dashboard` feature. Default binary carries zero new dependencies.

---

## Build Pipeline: `build.rs`

```rust
fn main() {
    // Only run npm build when dashboard feature is enabled
    #[cfg(feature = "dashboard")]
    build_dashboard();

    println!("cargo:rerun-if-changed=build.rs");
}

#[cfg(feature = "dashboard")]
fn build_dashboard() {
    let status = std::process::Command::new("npm")
        .current_dir("ui")
        .args(["run", "build"])
        .status()
        .expect("npm not found — install Node.js 22+ to build the dashboard");
    assert!(status.success(), "Dashboard UI build failed");
    println!("cargo:rerun-if-changed=ui/src");
    println!("cargo:rerun-if-changed=ui/package.json");
    println!("cargo:rerun-if-changed=ui/vite.config.ts");
}
```

Note: `#[cfg(...)]` in `build.rs` reads `CARGO_FEATURE_*` environment variables set by Cargo. The idiomatic alternative is checking `std::env::var("CARGO_FEATURE_DASHBOARD").is_ok()`.

---

## Frontend Source Layout

```
mnemonic/
  ui/                         # Frontend source (outside src/)
    package.json
    vite.config.ts
    src/
      main.tsx                # Entry point
      app.tsx
      components/
      ...
    dist/                     # Vite output — gitignored; consumed by rust-embed
      index.html              # Single inlined file (via vite-plugin-singlefile)
```

`ui/dist/` is gitignored. It is generated by `npm run build` during `cargo build --features dashboard`.

---

## Vite Configuration

**`ui/vite.config.ts`:**

```typescript
import { defineConfig } from 'vite'
import preact from '@preact/preset-vite'
import tailwindcss from '@tailwindcss/vite'
import { viteSingleFile } from 'vite-plugin-singlefile'

export default defineConfig({
  plugins: [
    preact(),
    tailwindcss(),
    viteSingleFile(),       // Must be last — inlines after Vite finishes bundling
  ],
  build: {
    outDir: 'dist',
    target: 'esnext',       // Modern browsers only — agents use current tooling
    emptyOutDir: true,
  },
})
```

**`ui/src/index.css`:**

```css
@import "tailwindcss";
```

No `tailwind.config.js` needed for v4.

---

## Axum Router Integration

```rust
// In router setup (feature-gated)
#[cfg(feature = "dashboard")]
use crate::dashboard::mount_dashboard;

pub fn build_router(state: AppState) -> Router {
    let router = Router::new()
        .route("/memories", post(store_memory))
        // ... existing routes ...
        .with_state(state);

    #[cfg(feature = "dashboard")]
    let router = mount_dashboard(router);

    router
}
```

```rust
// src/dashboard.rs (only compiled with dashboard feature)
#[cfg(feature = "dashboard")]
pub fn mount_dashboard(router: axum::Router) -> axum::Router {
    use axum_embed::ServeEmbed;
    use rust_embed::Embed;

    #[derive(Embed, Clone)]
    #[folder = "ui/dist/"]
    struct DashboardAssets;

    let serve = ServeEmbed::<DashboardAssets>::with_parameters(
        Some("index.html"),  // index file
        axum_embed::FallbackBehavior::Ok,  // SPA: return index.html for unknown paths
        Some("index.html"),  // 404 fallback
    );

    router.nest_service("/ui", serve)
}
```

The `/ui` prefix keeps dashboard routes separate from the API namespace. The existing API key auth middleware is applied at the `route_layer()` level on API routes — the dashboard can be configured to be either:
- **Open** (no auth on `/ui`) — useful for read-only status views
- **Auth-required** (add the same middleware to the `/ui` route) — use if dashboard includes write operations (compaction trigger)

Given the dashboard includes a compaction trigger, add the auth middleware to `/ui` when keys are active.

---

## CI / Release Workflow Changes

Add a `dashboard` build matrix entry:

```yaml
- name: Set up Node.js (dashboard builds only)
  if: matrix.features == 'dashboard'
  uses: actions/setup-node@v4
  with:
    node-version: '22'
    cache: 'npm'
    cache-dependency-path: ui/package-lock.json

- name: Install frontend dependencies (dashboard builds only)
  if: matrix.features == 'dashboard'
  run: npm ci
  working-directory: ui
```

All other build matrix entries (default, backend-qdrant, backend-postgres, interface-grpc) do not require Node.js — the binary is unchanged.

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `rust-embed 8.11` | `include_dir` | Only if you want a simpler crate with fewer features. `include_dir` lacks MIME detection, ETags, and dev-mode filesystem fallback — acceptable only for trivial single-file serving. |
| `axum-embed 0.1` | Custom handler with `Asset::get()` | If you need non-standard routing logic. The custom handler is ~40 lines and works, but you lose 304 caching, compression negotiation, and directory handling for free. |
| `axum-embed 0.1` | `memory-serve 2.1` | `memory-serve` also embeds + compresses at compile time and integrates with axum. It requires a `build.rs` `load_directory()` call and a `load!()` macro — slightly tighter coupling. Either is viable; `axum-embed` is chosen because it sits atop `rust-embed` which is already needed for the feature gate. |
| Vite + `@preact/preset-vite` | esbuild standalone script | esbuild requires manual Preact JSX factory config (`--jsx-factory=h --jsx-import-source=preact`), provides no dev HMR, and has no first-party CSS pipeline. Vite wraps esbuild internally and adds the full preset for free. Use esbuild directly only if you want zero Node.js tooling (an explicit non-goal here since Vite is already the standard). |
| `vite-plugin-singlefile` | Multi-file Vite output | Multi-file output produces hashed filenames (`main.a1b2c3.js`) requiring wildcard rust-embed routing and SPA fallback configuration. Single-file output means one `index.html` — trivial to embed and unambiguous to serve. |
| Tailwind v4 | Tailwind v3 | v3 is in maintenance mode. v4 is current, requires no config file, and has a Vite plugin. No reason to use v3 for a new project in 2026. |
| Preact 10.x | React 18/19 | React adds ~42 KB to the bundle. This is an operational dashboard served from a Rust binary — every KB is felt. Preact is API-compatible and the unambiguous correct choice for embedded use. |
| Preact 10.x | HTMX + Askama | HTMX requires server-side HTML rendering (Askama/Tera templates in Rust), adding SSR complexity and a Rust template layer. The existing REST API already returns JSON; Preact consumes it directly. HTMX would be appropriate only if avoiding JavaScript entirely was a goal. |
| Preact 10.x | Vue 3 / Svelte | Both are fine choices. Preact is chosen because its React API compatibility is well-known, its bundle is the smallest, and Vite has a first-class preset. |

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| React (`react` + `react-dom`) | 42 KB min+gzip — 10x larger than Preact for identical API surface | Preact 10.x |
| Next.js / Remix / SvelteKit | Server-side rendering frameworks that require a Node.js process at runtime — directly contradicts single-binary goal | Vite + Preact (static SPA) |
| `tower-http::ServeDir` | Reads from the filesystem at runtime — cannot load embedded assets; requires files on disk at the running server's path | `axum-embed` + `rust-embed` |
| Webpack | Enormous configuration overhead vs. near-zero with Vite; builds in seconds vs. tens of seconds | Vite |
| `include_bytes!` / manual inlining | Works for a single file; unmanageable for a multi-file SPA even after `vite-plugin-singlefile` reduces to one file; no MIME detection | `rust-embed` derive macro |
| `vite-rs` crate | 107 stars, Windows support WIP, sparse documentation, experimental API — too risky compared to the proven `build.rs` + `rust-embed` pattern | `build.rs` calling `npm run build` + `rust-embed` |
| Same-port REST+dashboard multiplexing | Documented body-type mismatch bugs (axum #2825); already ruled out for gRPC in v1.5 (tonic #1964) decision | `nest_service("/ui", ...)` on the existing axum router port |
| `@preact/signals` (preact signals state) | Adds ~1.6 KB; not needed for a simple dashboard — Preact's built-in `useState`/`useReducer` hooks are sufficient for v1.6 scope | Built-in Preact hooks |
| DaisyUI | CSS component library adds ~50 KB of CSS. v1.6 is an operational tool — hand-crafted Tailwind utility classes are appropriate at this scale. Add DaisyUI only if the UI complexity grows significantly. | Tailwind utilities directly |
| Server-side rendering (SSR) | Adds Rust template engine (Askama/Tera), template compilation, shared state serialization — enormous complexity increase for zero benefit given the existing REST API | Client-side SPA consuming existing JSON API |

---

## Stack Patterns by Variant

**Default build (`cargo build`, no features):**
- `build.rs` skips npm entirely (`CARGO_FEATURE_DASHBOARD` not set)
- `rust-embed` and `axum-embed` not in dependency tree
- Binary size unchanged from v1.5
- No Node.js required at build time

**Dashboard build (`cargo build --features dashboard`):**
- `build.rs` runs `npm ci && npm run build` in `ui/`
- `ui/dist/index.html` embedded via rust-embed
- `/ui` route mounted on axum router
- Auth middleware applies at `/ui` if keys are active
- Node.js 22 LTS required at build time

**Development workflow (dashboard developer):**
1. `npm run dev` in `ui/` — starts Vite dev server on port 5173 with HMR
2. Point browser to `http://localhost:5173` — fetches API from `http://localhost:8080`
3. When satisfied: `npm run build` then `cargo build --features dashboard`
4. Visit `http://localhost:8080/ui` — production embedded build

---

## Version Compatibility

| Package | Version | Compatible With | Notes |
|---------|---------|-----------------|-------|
| `rust-embed 8.11` | `axum ^0.8` | rust-embed's `axum` feature explicitly pins `axum ^0.8`; matches mnemonic's existing axum version exactly |
| `axum-embed 0.1` | `rust-embed ^8` | Compatible with 8.11; requires rust-embed 8.x |
| `@preact/preset-vite 2.10.3` | Vite 5.x and 6.x | Current stable; verify against preset-vite releases if upgrading Vite to 7.x+ |
| `@tailwindcss/vite 4.x` | Vite 5.x and 6.x, Tailwind v4 | Official Tailwind Labs package; version-matched to Tailwind v4 |
| `vite-plugin-singlefile` | Vite 4.x–6.x | Actively maintained; check compatibility on Vite major upgrades |
| Node.js 22 LTS | All npm packages above | LTS until 2027; aligns with GitHub Actions `actions/setup-node@v4` defaults |

---

## Frontend Package Installation

```bash
# Run once to scaffold (if starting from scratch)
cd ui
npm create vite@latest . -- --template preact-ts

# Or add to existing package.json:
npm install preact
npm install -D vite @preact/preset-vite @tailwindcss/vite tailwindcss vite-plugin-singlefile

# Verify build
npm run build    # produces ui/dist/index.html (single inlined file)
```

---

## Sources

- [docs.rs/rust-embed latest (8.11.0)](https://docs.rs/crate/rust-embed/latest) — version 8.11.0 (2026-01-14), feature flags (`axum`, `debug-embed`, `compression`), dev vs release loading behavior (HIGH confidence)
- [docs.rs/axum-embed 0.1.0](https://docs.rs/axum-embed/latest/axum_embed/) — version 0.1.0, rust-embed ^8 requirement, compression support, ServeEmbed API (HIGH confidence)
- [preactjs.com/guide/v10/getting-started](https://preactjs.com/guide/v10/getting-started/) — Vite as recommended tool, 3 KB bundle size, 10.x stable / 11.0.0-beta.1 unstable (HIGH confidence)
- [tailwindcss.com/docs/installation/tailwind-cli](https://tailwindcss.com/docs/installation/tailwind-cli) — v4.2 current, standalone CLI confirmed available, `@import "tailwindcss"` pattern (HIGH confidence)
- [esbuild.github.io/getting-started](https://esbuild.github.io/getting-started/) — v0.27.3 current (evaluated as alternative to Vite) (HIGH confidence)
- [github.com/preactjs/preset-vite](https://github.com/preactjs/preset-vite) — @preact/preset-vite 2.10.3 (MEDIUM confidence — from npm registry data in search results)
- [npmjs.com/package/vite-plugin-singlefile](https://www.npmjs.com/package/vite-plugin-singlefile) — single-file inlining, useRecommendedBuildConfig option, actively maintained (MEDIUM confidence — npm registry)
- [docs.rs/memory-serve 2.1.0](https://docs.rs/memory-serve/latest/memory_serve/) — alternative considered, version 2.1.0, build.rs requirement confirmed (HIGH confidence)
- [itmecho.com/blog/rust-embedded-client](https://itmecho.com/blog/rust-embedded-client) — build.rs + rust-embed + axum integration pattern; `cargo:rerun-if-changed` directives (MEDIUM confidence — community blog, pattern is widely replicated)
- WebSearch (rust-embed axum example) — confirmed `features = ["axum"]` enables built-in axum integration in 8.x (MEDIUM confidence — cross-referenced with docs.rs)

---

*Stack research for: Mnemonic v1.6 embedded web dashboard (Preact + Tailwind)*
*Researched: 2026-03-22*
