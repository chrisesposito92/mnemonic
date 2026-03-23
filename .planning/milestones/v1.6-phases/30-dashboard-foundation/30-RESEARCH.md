# Phase 30: Dashboard Foundation - Research

**Researched:** 2026-03-22
**Domain:** Rust feature gates, rust-embed, axum-embed, Vite + Preact + Tailwind v4, GitHub Actions CI
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Minimal proof-of-life page — "Mnemonic Dashboard" heading + version + health status from GET /health. Phase 31 replaces it entirely.
- **D-02:** Basic Tailwind styling (dark background, centered card, monospace font) to confirm the full Preact + Tailwind + embed pipeline works.
- **D-03:** Live fetch to GET /health on mount, displaying backend name + status. Validates the full SPA → API round-trip.
- **D-04:** Tailwind v4 with `@tailwindcss/vite` plugin. CSS-first config, no PostCSS or tailwind.config.js needed.
- **D-05:** npm as package manager. Aligns with CI success criteria (`npm ci && npm run build`).
- **D-06:** TypeScript for the Preact frontend.
- **D-07:** Separate Vite dev server for frontend development. `npm run dev` on :5173 with HMR, API proxy to :8080.
- **D-08:** Try vite-plugin-singlefile first to produce a single index.html with inlined JS/CSS.
- **D-09:** If vite-plugin-singlefile fails with Preact + Tailwind v4, fall back to multi-file output with axum-embed serving the directory.
- **D-10:** Node.js setup + `npm ci && npm run build` runs as a step within each matrix job (not a separate prerequisite job). Self-contained per platform.
- **D-11:** Separate `regression` CI job runs `cargo build` (default features, no dashboard) + `cargo test` in parallel with dashboard builds. Failure blocks release.
- **D-12:** Release produces both variants per platform — `mnemonic` (slim, no dashboard) and `mnemonic-dashboard` (with embedded UI).
- **D-13:** rust-embed 8.11 + axum-embed 0.1 for compile-time asset embedding, both optional deps behind `dashboard` feature.
- **D-14:** Hash routing (`#/path`) over history routing — avoids SPA hard-reload 404s at zero cost.
- **D-15:** Dashboard router merged at top level in build_router() (not inside protected router) to prevent auth middleware blocking asset loads.

### Claude's Discretion

- Exact dashboard/ directory structure and file layout
- Vite configuration details
- Preact project scaffolding approach
- Exact Cargo.toml dependency versions for rust-embed/axum-embed
- Compile-time error implementation for missing dashboard/dist/index.html

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| BUILD-01 | Dashboard assets embedded into binary at compile time via rust-embed, served at `/ui` via axum-embed with SPA fallback | rust-embed 8.11 derive macro + axum-embed 0.1 `with_parameters()` with `FallbackBehavior::Ok` + `fallback_file = "index.html"` |
| BUILD-02 | Dashboard feature-gated behind `dashboard` Cargo feature with zero impact on default binary | `dep:` syntax in Cargo.toml features table + `#[cfg(feature = "dashboard")]` blocks in src/main.rs and src/server.rs, matching existing `interface-grpc` pattern |
| BUILD-03 | CI release workflow updated with Node.js build step before cargo build; separate job verifies default binary still passes all tests | `actions/setup-node@v4` step within each matrix job + new `regression` job running `cargo build` + `cargo test` in parallel with build matrix |
</phase_requirements>

---

## Summary

Phase 30 is a build-pipeline and scaffolding phase. It creates three interconnected deliverables: (1) a `dashboard/` directory containing a Preact + TypeScript + Tailwind v4 SPA scaffolded with Vite, (2) a `dashboard` Cargo feature gate wiring rust-embed and axum-embed as optional dependencies so `cargo build --features dashboard` embeds the built SPA at `/ui`, and (3) CI changes that run the npm build before the Rust build in each matrix job plus a parallel regression job that verifies the default binary.

The existing codebase provides a clear pattern to follow: the `interface-grpc` feature gate in `src/main.rs` and `Cargo.toml` uses `dep:` syntax for optional dependencies and `#[cfg(feature = "interface-grpc")]` blocks in Rust source. The dashboard gate follows the exact same pattern. The `build_router()` function in `src/server.rs` already uses axum's `.merge()` composition model — the dashboard routes add a third `.merge()` call behind a `#[cfg(feature = "dashboard")]` block, outside the protected router (per D-15).

The primary technical risk is vite-plugin-singlefile compatibility with `@preact/preset-vite` + `@tailwindcss/vite`. No documented incompatibility was found in current GitHub issues (the only open issues are Vite 8 deprecations), but this is noted as MEDIUM confidence because it has not been tested in combination. The fallback path (multi-file dist with axum-embed serving the directory) is well-defined and carries no risk.

**Primary recommendation:** Scaffold `dashboard/` with `npm create vite@latest . --template preact-ts`, install `tailwindcss @tailwindcss/vite vite-plugin-singlefile`, wire Cargo.toml with `dep:` syntax, add `#[cfg(feature = "dashboard")]` blocks, and extend the CI workflow — in that order.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rust-embed | 8.11.0 | Compile-time asset embedding in Rust binary | Locked decision D-13; confirmed current via `cargo search rust-embed` |
| axum-embed | 0.1.0 | Serve rust-embed assets via axum with SPA fallback | Locked decision D-13; confirmed current via `cargo search axum-embed` |
| preact | 10.29.0 | Lightweight React-compatible UI library | Locked decision (D-06 TypeScript + Preact) |
| @preact/preset-vite | 2.10.5 | Preact HMR + devtools + JSX transform in Vite | Standard Preact+Vite integration; actively maintained (2026-03-20 release) |
| vite | 8.0.1 | Frontend build tool | Standard; vite-plugin-singlefile supports ^5.4 &#124;&#124; ^6 &#124;&#124; ^7 &#124;&#124; ^8 |
| tailwindcss | 4.2.2 | Utility-first CSS | Locked decision D-04 |
| @tailwindcss/vite | 4.2.2 | Tailwind v4 Vite plugin — no PostCSS required | Locked decision D-04; CSS-first config |
| vite-plugin-singlefile | 2.3.2 | Inlines JS+CSS into single index.html | Locked decision D-08; fallback defined in D-09 |
| typescript | 5.9.3 | TypeScript language | Locked decision D-06 |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| @preact/signals | 2.8.2 | Fine-grained reactivity for Preact | If state management grows beyond useState — not needed for Phase 30 minimal shell |

### Alternatives Considered (Already Decided Against)

| Instead of | Could Use | Why Decided Against |
|------------|-----------|---------------------|
| vite-plugin-singlefile | multi-file axum-embed (D-09) | Use D-09 fallback only if singlefile fails in practice |
| hash routing | history routing | History routing causes 404 on hard-reload without server-side catch-all (D-14) |
| dashboard feature gate | always-on | Would bundle dashboard assets into slim binary, violating zero-impact requirement (BUILD-02) |

**Verified package versions (2026-03-22):**
```bash
npm view vite version            # 8.0.1
npm view vite-plugin-singlefile version  # 2.3.2
npm view @preact/preset-vite version     # 2.10.5
npm view tailwindcss version             # 4.2.2
npm view @tailwindcss/vite version       # 4.2.2
cargo search rust-embed                  # 8.11.0
cargo search axum-embed                  # 0.1.0
```

**Installation:**
```bash
# Frontend (inside dashboard/)
npm create vite@latest . --template preact-ts
npm install tailwindcss @tailwindcss/vite vite-plugin-singlefile
```

```toml
# Cargo.toml additions
[dependencies]
rust-embed = { version = "8.11", optional = true }
axum-embed = { version = "0.1", optional = true }

[features]
dashboard = ["dep:rust-embed", "dep:axum-embed"]
```

---

## Architecture Patterns

### Recommended Project Structure

```
dashboard/                    # Frontend root (D-07 Vite project)
├── package.json
├── package-lock.json         # Required for npm ci in CI
├── tsconfig.json
├── vite.config.ts
├── index.html                # Vite entry point
├── src/
│   ├── main.tsx              # Preact entry — mounts <App />
│   ├── App.tsx               # AppShell component (UI-SPEC §Component 1)
│   ├── index.css             # @import "tailwindcss" + @theme block with CSS vars
│   └── components/
│       └── HealthCard.tsx    # Health card component (UI-SPEC §Component 2)
└── dist/                     # Build output — gitignored, created by npm run build
    └── index.html            # Embedded by rust-embed at compile time

src/
├── dashboard.rs              # New module — #[cfg(feature = "dashboard")] only
├── server.rs                 # build_router() extended with dashboard merge
├── main.rs                   # mod dashboard behind #[cfg(feature = "dashboard")]
└── ...                       # All existing files unchanged

.github/workflows/
└── release.yml               # Extended with Node.js step + regression job
```

### Pattern 1: Cargo Feature Gate with `dep:` Syntax

**What:** Optional dependencies behind a named feature using the `dep:` prefix to prevent namespace collision with the feature name itself.

**When to use:** Any optional Cargo dependency. The existing `interface-grpc`, `backend-qdrant`, and `backend-postgres` features in this project all use this pattern.

**Example:**
```toml
# Cargo.toml — following existing pattern
[dependencies]
rust-embed = { version = "8.11", optional = true }
axum-embed = { version = "0.1", optional = true }

[features]
dashboard = ["dep:rust-embed", "dep:axum-embed"]
```

```rust
// src/main.rs — following existing interface-grpc pattern
#[cfg(feature = "dashboard")]
mod dashboard;

// In server startup block:
#[cfg(feature = "dashboard")]
{
    tracing::info!("Dashboard enabled — serving at /ui");
}
```

### Pattern 2: rust-embed Derive Macro

**What:** `#[derive(RustEmbed)]` on a struct with `#[folder = "..."]` embeds all files from the named directory into the binary at compile time. The folder path is resolved relative to `Cargo.toml` in release mode.

**When to use:** Any compile-time asset embedding. Panics at compile time if the folder is absent (unless `#[allow_missing = true]` is set).

**Example:**
```rust
// src/dashboard.rs
use rust_embed::RustEmbed;

#[derive(RustEmbed, Clone)]
#[folder = "dashboard/dist/"]
struct DashboardAssets;
```

**Compile-time error for missing dist/:** The default behavior (without `#[allow_missing = true]`) causes `cargo build --features dashboard` to fail with a `proc-macro derive panicked` error if `dashboard/dist/index.html` is absent. This satisfies the success criterion "Build fails with a clear compile-time error if --features dashboard is set but dashboard/dist/index.html is missing." **Do NOT add `#[allow_missing = true]`** — the failure is the desired behavior.

### Pattern 3: axum-embed SPA Serving

**What:** `ServeEmbed::with_parameters()` configures the fallback behavior for missing paths. For SPA routing, `FallbackBehavior::Ok` + `fallback_file = "index.html"` returns the app shell for any unrecognized path — the SPA's hash router handles the rest.

**When to use:** Any SPA served from axum with client-side routing.

**Example:**
```rust
// src/dashboard.rs
use axum_embed::{FallbackBehavior, ServeEmbed};
use axum::Router;

pub fn dashboard_router() -> Router {
    let serve = ServeEmbed::<DashboardAssets>::with_parameters(
        Some("index.html".to_owned()),   // fallback file
        FallbackBehavior::Ok,            // serve fallback with 200 (not 404)
        Some("index.html".to_owned()),   // index file for directory requests
    );
    Router::new().nest_service("/ui", serve)
}
```

```rust
// src/server.rs — build_router() extension
#[cfg(feature = "dashboard")]
let router = router.merge(crate::dashboard::dashboard_router());
```

**Note on axum 0.8 + nest_service:** The `nest_service` call strips the `/ui` prefix before passing to `ServeEmbed`. This means `DashboardAssets` must embed files at their relative paths (e.g., `index.html`, not `/ui/index.html`). This is the standard axum behavior and works correctly with rust-embed's folder-relative paths.

### Pattern 4: Tailwind v4 CSS-First Configuration

**What:** Tailwind v4 drops `tailwind.config.js`. All configuration lives in a single CSS file using `@theme` blocks. The `@tailwindcss/vite` plugin processes this automatically — no PostCSS pipeline needed.

**When to use:** All Tailwind v4 projects with Vite.

**Example:**
```css
/* dashboard/src/index.css */
@import "tailwindcss";

@theme {
  --color-bg: #0a0a0a;
  --color-surface: #1a1a1a;
  --color-border: #2a2a2a;
  --color-text: #e5e5e5;
  --color-text-muted: #6b7280;
  --color-accent: #22d3ee;
  --color-error: #ef4444;
}
```

```typescript
// dashboard/vite.config.ts
import { defineConfig } from 'vite'
import preact from '@preact/preset-vite'
import tailwindcss from '@tailwindcss/vite'
import { viteSingleFile } from 'vite-plugin-singlefile'

export default defineConfig({
  plugins: [
    preact(),
    tailwindcss(),
    viteSingleFile(),
  ],
  build: {
    outDir: 'dist',
  },
})
```

### Pattern 5: CI Matrix with Node.js Step + Regression Job

**What:** Node.js setup is added as steps within each existing matrix job (not a separate job). A new parallel `regression` job runs the default binary build + test suite. Both the matrix jobs and the regression job must succeed before the release job.

**When to use:** Any release workflow that produces both a slim binary and a dashboard binary.

**Example:**
```yaml
# .github/workflows/release.yml additions

jobs:
  build:
    # existing matrix ...
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      # ADD: Node.js setup before Rust build
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '22'
          cache: 'npm'
          cache-dependency-path: dashboard/package-lock.json

      - name: Build dashboard
        run: |
          cd dashboard
          npm ci
          npm run build

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      # existing protoc step...

      # CHANGE: build with dashboard feature, produce TWO artifacts
      - name: Build slim binary
        run: cargo build --release --target ${{ matrix.target }}

      - name: Build dashboard binary
        run: cargo build --release --target ${{ matrix.target }} --features dashboard

      - name: Stage artifacts
        run: |
          mkdir -p dist
          cp target/${{ matrix.target }}/release/mnemonic dist/${{ matrix.artifact }}
          cp target/${{ matrix.target }}/release/mnemonic dist/${{ matrix.artifact }}-dashboard
          tar -czf dist/${{ matrix.artifact }}.tar.gz -C dist ${{ matrix.artifact }}
          tar -czf dist/${{ matrix.artifact }}-dashboard.tar.gz -C dist ${{ matrix.artifact }}-dashboard

  # ADD: Parallel regression job
  regression:
    name: Default Binary Regression
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Install protoc
        uses: arduino/setup-protoc@v3
        with:
          repo-token: ${{ secrets.GITHUB_TOKEN }}

      - name: Build default binary
        run: cargo build --release

      - name: Run tests
        run: cargo test

  release:
    name: Publish GitHub Release
    needs: [build, regression]    # CHANGE: both must pass
    # ... rest unchanged
```

### Anti-Patterns to Avoid

- **Using `#[allow_missing = true]` on DashboardAssets:** This defeats the compile-time error requirement. The missing-dist panic IS the safety gate.
- **Placing dashboard routes inside the protected router:** Auth middleware would block `GET /ui/` and all asset loads. The merge must happen at the top-level router (D-15).
- **History routing instead of hash routing:** Hard reloads on deep paths (e.g., `/ui/settings`) cause the server to return 404 before the SPA loads. Hash routing (`#/settings`) keeps all routing in the browser (D-14).
- **Installing PostCSS alongside @tailwindcss/vite:** Tailwind v4 with the Vite plugin does not use PostCSS. Adding PostCSS can cause conflicts.
- **Running `npm install` in CI instead of `npm ci`:** `npm ci` uses `package-lock.json` for deterministic installs. `npm install` may update the lockfile and is slower.
- **Using a single job for both Node and Rust on separate jobs:** D-10 specifies Node steps within each matrix job to keep each job self-contained per platform.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Static asset serving with MIME types | Custom axum handler | axum-embed 0.1 | ETag caching, gzip/brotli negotiation, directory redirects built-in |
| Compile-time binary embedding | `include_bytes!` per file | rust-embed 8.11 derive macro | Handles directory traversal, path normalization, release vs debug modes |
| SPA fallback routing | Custom axum fallback handler | axum-embed `FallbackBehavior::Ok` | Correctly handles the path stripping from `nest_service` |
| CSS bundling/purging | Custom Vite plugin | `@tailwindcss/vite` | Handles purging, CSS custom properties, and v4's @theme blocks correctly |
| JS + CSS inlining | Custom rollup plugin | `vite-plugin-singlefile` | Handles Rollup chunk inlining, module loader stripping, and file cleanup |

**Key insight:** The entire asset pipeline (npm build → binary embed → axum serve) is a solved problem with the chosen stack. Any custom code in this pipeline is a maintenance burden with no upside.

---

## Common Pitfalls

### Pitfall 1: rust-embed Folder Path Resolution in Release vs Debug

**What goes wrong:** In release mode (and when `debug-embed` feature is enabled), folder paths are resolved relative to `Cargo.toml`. In debug mode without `debug-embed`, paths are resolved relative to the binary's working directory. This causes the embed to work in release CI but fail locally during dev.

**Why it happens:** rust-embed embeds files at compile time in release mode but reads from disk at runtime in debug mode (unless `debug-embed` feature is active).

**How to avoid:** Always specify `#[folder = "dashboard/dist/"]` as a path relative to `Cargo.toml` (the project root). Do not use absolute paths. Ensure `dashboard/dist/` exists before running `cargo build --features dashboard` in any mode.

**Warning signs:** `GET /ui/` returns 404 in local `cargo run --features dashboard` even though CI builds succeeded.

### Pitfall 2: vite-plugin-singlefile CSS Not Inlined

**What goes wrong:** The built `dist/index.html` has a `<link rel="stylesheet">` tag pointing to an external CSS file instead of inlined `<style>` tags. The CSS file is not embedded, causing the SPA to render without styles.

**Why it happens:** Some Vite plugin combinations may prevent vite-plugin-singlefile from intercepting the CSS output. The `@tailwindcss/vite` plugin generates CSS as a separate chunk by default.

**How to avoid:** Verify that `viteSingleFile()` is listed AFTER `tailwindcss()` in the plugins array. Inspect `dist/index.html` after `npm run build` — it should be a single file with no external references. If external `<link>` or `<script src>` tags remain, switch to the D-09 fallback (multi-file axum-embed).

**Warning signs:** `dashboard/dist/` contains `.css` files after `npm run build`; `dist/index.html` is smaller than ~10KB.

### Pitfall 3: `nest_service("/ui", ...)` Strips the `/ui` Prefix

**What goes wrong:** A `GET /ui/` request reaches `ServeEmbed` asking for path `/` (not `/ui/`). If the embedded assets use full paths (e.g., `ui/index.html`), the request misses and returns 404.

**Why it happens:** axum's `nest_service` strips the matched prefix before forwarding to the nested service. This is correct axum behavior.

**How to avoid:** rust-embed embeds files by their path relative to the `#[folder]`. So `#[folder = "dashboard/dist/"]` makes `dist/index.html` available as `index.html` — exactly what `ServeEmbed` looks up after the prefix strip. Do not put files in a subdirectory of `dist/` expecting them to be addressable at `/ui/subdir/`.

**Warning signs:** `GET /ui/` returns 404 with `--features dashboard`; `GET /` returns 404 from the `ServeEmbed` service.

### Pitfall 4: Auth Middleware Blocking Dashboard Assets

**What goes wrong:** `GET /ui/` returns 401 even though the dashboard should be publicly accessible.

**Why it happens:** If the dashboard `nest_service` call is placed inside the `protected` router (which has `route_layer(middleware::from_fn_with_state(..., auth_middleware))`), ALL requests to merged routes inherit the auth middleware.

**How to avoid:** Merge the dashboard router at the top-level `Router::new()` in `build_router()`, not inside the `protected` variable. This is D-15.

**Warning signs:** Browser shows 401 on `GET /ui/`; curl returns `{"error":"unauthorized"}` for asset requests.

### Pitfall 5: Regression Job Not Blocking Release

**What goes wrong:** The `regression` CI job runs and fails, but the release still publishes.

**Why it happens:** The `release` job's `needs:` array only includes `[build]`, not `[build, regression]`.

**How to avoid:** The `release` job must specify `needs: [build, regression]`. Verify this in the final YAML.

**Warning signs:** CI shows green release despite red regression job (look carefully at job dependency graph in Actions UI).

### Pitfall 6: vite-plugin-singlefile Open Issues with Vite 8

**What goes wrong:** `npm run build` succeeds but emits a deprecation warning: "inlineDynamicImports is deprecated on vite 8". In future versions this may become an error.

**Why it happens:** vite-plugin-singlefile 2.3.2 uses `inlineDynamicImports` which Vite 8 deprecated (per open GitHub issue).

**How to avoid:** This is a known open issue in vite-plugin-singlefile as of 2026-03-22. Treat warnings as acceptable noise for Phase 30; if it errors, switch to D-09 fallback. Watch for vite-plugin-singlefile 2.4.x which should address Vite 8 compatibility.

---

## Code Examples

### rust-embed + axum-embed SPA Module

```rust
// src/dashboard.rs — full module
// Source: axum-embed docs, informationsea/axum-embed examples/serve.rs

use axum::Router;
use axum_embed::{FallbackBehavior, ServeEmbed};
use rust_embed::RustEmbed;

#[derive(RustEmbed, Clone)]
#[folder = "dashboard/dist/"]
struct DashboardAssets;

/// Returns a Router serving the embedded SPA at all paths.
/// Caller nests this at /ui via Router::merge().
pub fn router() -> Router {
    let serve = ServeEmbed::<DashboardAssets>::with_parameters(
        Some("index.html".to_owned()),
        FallbackBehavior::Ok,
        Some("index.html".to_owned()),
    );
    Router::new().nest_service("/ui", serve)
}
```

### Cargo.toml Feature Gate

```toml
# Following the exact dep: pattern used by interface-grpc, backend-qdrant, backend-postgres
[dependencies]
rust-embed = { version = "8.11", optional = true }
axum-embed = { version = "0.1", optional = true }

[features]
dashboard = ["dep:rust-embed", "dep:axum-embed"]
```

### build_router() Extension

```rust
// src/server.rs — inside build_router()
// Source: existing interface-grpc pattern in src/main.rs

pub fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        // ... existing routes unchanged ...
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    let public = Router::new()
        .route("/health", get(health_handler));

    let mut router = Router::new()
        .merge(protected)
        .merge(public)
        .with_state(state);

    #[cfg(feature = "dashboard")]
    {
        router = router.merge(crate::dashboard::router());
    }

    router
}
```

### Tailwind v4 CSS-First Setup

```css
/* dashboard/src/index.css */
/* Source: tailwindcss.com/docs/installation/using-vite */
@import "tailwindcss";

@theme {
  --color-bg: #0a0a0a;
  --color-surface: #1a1a1a;
  --color-border: #2a2a2a;
  --color-text: #e5e5e5;
  --color-text-muted: #6b7280;
  --color-accent: #22d3ee;
  --color-error: #ef4444;
}
```

### Vite Config with All Three Plugins

```typescript
// dashboard/vite.config.ts
import { defineConfig } from 'vite'
import preact from '@preact/preset-vite'
import tailwindcss from '@tailwindcss/vite'
import { viteSingleFile } from 'vite-plugin-singlefile'

export default defineConfig({
  plugins: [
    preact(),
    tailwindcss(),
    viteSingleFile(),        // must come after tailwindcss
  ],
  server: {
    port: 5173,
    proxy: {
      '/health': 'http://localhost:8080',
      '/memories': 'http://localhost:8080',
    },
  },
  build: {
    outDir: 'dist',
    target: 'esnext',      // vite-plugin-singlefile recommended setting
  },
})
```

### Minimal Proof-of-Life App Shell

```tsx
// dashboard/src/App.tsx
import { useEffect, useState } from 'preact/hooks'
import HealthCard from './components/HealthCard'

export default function App() {
  return (
    <div class="min-h-screen flex flex-col items-center justify-center gap-8"
         style="background: var(--color-bg)">
      <h1 class="text-xl font-semibold" style="color: var(--color-text)">
        Mnemonic Dashboard
      </h1>
      <HealthCard />
    </div>
  )
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Tailwind config via tailwind.config.js | CSS-first `@theme` block, `@import "tailwindcss"` | Tailwind v4.0 (Jan 2025) | No JS config file; PostCSS optional |
| PostCSS-based Tailwind Vite integration | `@tailwindcss/vite` plugin | Tailwind v4.0 | Tighter Vite integration, faster HMR |
| axum-extract SpaRouter (removed) | `nest_service` + ServeEmbed fallback | axum 0.7+ | SpaRouter removed; use tower Service pattern |
| `dep:` crate name conflicts | explicit `dep:` prefix in features | Cargo 1.60 | Feature name no longer auto-activates optional dep |

**Deprecated/outdated:**
- `axum-extra::routing::SpaRouter`: Removed in axum 0.7. Do not use.
- `tailwind.config.js` with PostCSS: Still works for v3 but is the v3 approach. Phase 30 uses v4 CSS-first.
- `npm install` in CI: Use `npm ci` for deterministic, lock-file-based installs.

---

## Open Questions

1. **vite-plugin-singlefile + Tailwind v4 CSS inlining in practice**
   - What we know: No documented incompatibility found; open issues on vite-plugin-singlefile are about Vite 8 deprecation of `inlineDynamicImports`, not CSS inlining
   - What's unclear: Whether `@tailwindcss/vite`'s CSS output is correctly intercepted by singlefile's Rollup plugin transform
   - Recommendation: Verify empirically during Wave 0 — run `npm run build` and check that `dist/index.html` has no external `<link>` or `<script src>` references; if any remain, switch to D-09 fallback immediately

2. **axum 0.8 `nest_service` prefix behavior with `ServeEmbed`**
   - What we know: axum `nest_service` strips the matched prefix before passing to the service; axum-embed example uses `nest_service("/", assets)` (root mount)
   - What's unclear: Whether mounting at `/ui` (non-root) has any edge cases with trailing-slash redirects or the ServeEmbed directory index logic
   - Recommendation: Test `GET /ui` (no trailing slash), `GET /ui/` (trailing slash), and `GET /ui/nonexistent` after implementation; all three should return 200 with index.html content

3. **Dual-artifact naming in release matrix**
   - What we know: Current release workflow produces `mnemonic-linux-x86_64`, `mnemonic-macos-x86_64`, `mnemonic-macos-aarch64`
   - What's unclear: Exact naming convention for the dashboard variants (`mnemonic-dashboard-linux-x86_64` vs `mnemonic-linux-x86_64-dashboard`)
   - Recommendation: Use `{artifact}-dashboard` suffix (e.g., `mnemonic-linux-x86_64-dashboard`) to keep alphabetical grouping in the release asset list

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| cargo | Rust build | Yes | 1.94.0 (Homebrew) | — |
| node | npm frontend build | Yes | v24.13.0 | — |
| npm | Package management (D-05) | Yes | 11.6.2 | — |
| protoc | Existing build.rs (grpc) | Verified in CI | — | CI step already present |

All required tools are available locally. CI availability is verified by the existing workflow (protoc step is already in place for gRPC).

---

## Validation Architecture

Nyquist validation is enabled (`workflow.nyquist_validation: true`).

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (`cargo test`) |
| Config file | none (no pytest.ini / jest.config — Rust inline `#[cfg(test)]`) |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test` |
| Current passing tests | 292 (87 lib + 60 integration + 4 + 54 + others) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BUILD-01 | `cargo build --features dashboard` succeeds; `GET /ui/` returns 200 with HTML | smoke / integration | `cargo build --features dashboard && cargo test --features dashboard -- dashboard` | No — Wave 0 |
| BUILD-01 | `GET /ui/` returns `text/html` content-type | integration | `cargo test --features dashboard -- ui_serves_html` | No — Wave 0 |
| BUILD-02 | `cargo build` (default) produces identical binary to v1.5 behavior | regression | `cargo test` (existing suite, 292 tests) | Yes (292 existing) |
| BUILD-02 | No new code paths in default build (zero dashboard deps compiled in) | compile-check | `cargo build 2>&1 \| grep -v dashboard` | n/a — verified by clean build |
| BUILD-03 | CI `regression` job passes — all 292 tests green | CI gate | verified in CI only | CI-only |

**Note on BUILD-01 test:** An integration test that actually hits `GET /ui/` requires the binary to be running. This is a smoke test, not a unit test. It can be implemented as a `#[tokio::test]` using `axum::Router` directly via `tower::ServiceExt::oneshot` (the pattern already used in `tests/integration.rs`).

### Sampling Rate

- **Per task commit:** `cargo test --lib` (87 tests, ~0.05s)
- **Per wave merge:** `cargo test` (292 tests, ~30s)
- **Phase gate:** `cargo test` green + `cargo build --features dashboard` succeeds before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `tests/dashboard_integration.rs` — covers BUILD-01: `GET /ui/` returns 200 with `text/html` when built with `--features dashboard`
- [ ] `src/dashboard.rs` — module does not exist yet; must be created before the integration test can compile

*(All other test infrastructure is present — `tests/integration.rs` pattern is reusable)*

---

## Sources

### Primary (HIGH confidence)

- `cargo search rust-embed` — confirmed version 8.11.0 current on crates.io
- `cargo search axum-embed` — confirmed version 0.1.0 current on crates.io
- `npm view vite version`, `npm view vite-plugin-singlefile version`, etc. — all versions confirmed against npm registry 2026-03-22
- axum-embed GitHub source (`informationsea/axum-embed/blob/main/examples/serve.rs`) — `FallbackBehavior` enum and `with_parameters()` API confirmed
- tailwindcss.com/docs/installation/using-vite — CSS-first `@import "tailwindcss"` + `@tailwindcss/vite` plugin configuration confirmed
- Existing `src/main.rs` and `Cargo.toml` — `interface-grpc` pattern confirmed as the template for the `dashboard` feature gate
- Existing `src/server.rs` — `build_router()` `.merge()` pattern confirmed for dashboard router integration

### Secondary (MEDIUM confidence)

- axum-embed README (WebFetch) — `ServeEmbed::new()` defaults and `with_parameters()` signature confirmed; SPA fallback details from source inspection
- rust-embed derive macro docs (docs.rs) — `#[folder]`, `#[allow_missing]`, `#[prefix]` attributes; compile-time panic behavior confirmed
- vite-plugin-singlefile GitHub README + issues (WebFetch) — peer dependency `vite ^5.4 || ^6 || ^7 || ^8` confirmed; open Vite 8 deprecation issues noted
- @preact/preset-vite GitHub README (WebFetch) — version 2.10.5, basic config confirmed; Tailwind v4 compatibility not explicitly documented

### Tertiary (LOW confidence — flag for validation)

- vite-plugin-singlefile + @tailwindcss/vite CSS inlining behavior: No incompatibility found, but combination not explicitly tested/documented. Verify during Wave 0.
- axum 0.8 `nest_service("/ui", ...)` non-root mount edge cases with ServeEmbed: Behavior with trailing-slash redirects unconfirmed; test during implementation.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all versions confirmed against live registries
- Architecture patterns: HIGH — all patterns derived from existing codebase conventions and official library docs
- vite-plugin-singlefile + Tailwind v4 CSS inlining: MEDIUM — no incompatibility found but combination unverified; D-09 fallback is defined
- CI YAML structure: HIGH — based on existing release.yml plus documented GitHub Actions patterns
- Pitfalls: HIGH — most from official docs, issue trackers, or codebase inspection

**Research date:** 2026-03-22
**Valid until:** 2026-04-22 (stable ecosystem; rust-embed and axum-embed are slow-moving)
