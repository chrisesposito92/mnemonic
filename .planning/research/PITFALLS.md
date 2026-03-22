# Pitfalls Research

**Domain:** Embedded web dashboard added to existing Rust binary (v1.6 milestone — Preact + Tailwind + rust-embed + axum)
**Researched:** 2026-03-22
**Confidence:** HIGH — core pitfalls verified against official axum/rust-embed docs and multiple community sources; auth and security pitfalls verified against authoritative references; Tailwind v4 behavior confirmed against tailwindlabs GitHub discussions

---

## Critical Pitfalls

### Pitfall 1: SPA Client-Side Routes Return 404 on Hard Reload

**What goes wrong:**
The browser navigates to `/ui/search` (a Preact router path). User refreshes. axum receives `GET /ui/search`, finds no file at that path, returns 404. The SPA never boots — the user sees a blank error page.

**Why it happens:**
rust-embed serves static files. axum matches routes literally. Preact's router generates URL paths that only exist in JavaScript state, not on disk. Developers test by clicking links (which trigger JS navigation, never touching the server) and miss that direct URL access or hard reloads break.

**How to avoid:**
Two patterns work. Choose one before writing any routing code.

Option A — Hash routing: Use `/#/search` style URLs in the Preact router. The hash fragment is never sent to the server. axum always serves `index.html` for `/ui`. No fallback logic needed. Simpler. Slightly dated URL aesthetics — acceptable for an operational dashboard.

Option B — History routing with fallback: Add a catch-all handler under the `/ui` prefix that returns `index.html` for any path that does not match a known static asset (distinguished by file extension). Rule: if the path has no extension, serve `index.html`; if it has an extension, serve the file or 404.

For mnemonic's operational dashboard with no deep linking requirements, **hash routing is the right choice** — it eliminates this entire failure class at zero cost.

**Warning signs:**
- Testing only by clicking links in the running app, never hard-refreshing a page
- No automated test that hits `/ui/some-path` directly and asserts 200 + `text/html`
- axum router uses `route("/ui/*path", ...)` without a fallback handler returning `index.html`

**Phase to address:**
Phase 1 (build pipeline + static asset serving scaffold). Decide hash vs. history routing before writing any Preact router code.

---

### Pitfall 2: rust-embed Debug/Release Behavioral Divergence

**What goes wrong:**
In debug builds, rust-embed reads files from the filesystem at runtime relative to the current working directory. In release builds, files are embedded at compile time. A developer runs `cargo run` from the workspace root — files resolve. CI runs the binary from a different directory — 404 on every asset. Or: assets serve fine in dev but are missing in the release binary because the frontend build (`npm run build`) was not run before `cargo build --release`.

**Why it happens:**
This is intentional rust-embed design: fast iteration in dev via filesystem reads, zero overhead in release via embedding. Developers forget that the release build requires the frontend to be built first. The asset directory must exist and be populated before `cargo build --release` runs, or rust-embed embeds an empty directory.

**How to avoid:**
- Make the frontend build a prerequisite of the Rust release build. Document this prominently in `CONTRIBUTING.md`.
- Add a `build.rs` `println!("cargo:rerun-if-changed=ui/dist")` directive so Cargo re-embeds on any frontend change.
- Consider using the `debug-embed` feature during CI to enforce identical behavior between debug and release during testing.
- Add a smoke test that requests `/ui/` on the release binary and asserts a non-empty `text/html` response.

**Warning signs:**
- Dev build works but CI build produces a binary where `/ui/` returns 404 or empty body
- Developers never run `cargo build --release` locally before shipping
- No integration test covers the `/ui/` endpoint against the release binary

**Phase to address:**
Phase 1 (CI build pipeline). The `cargo build --release` step in `.github/workflows/release.yml` must run `npm ci && npm run build` in the `ui/` directory before the `cargo build` step.

---

### Pitfall 3: Binary Size Bloat from Unoptimized Frontend Assets

**What goes wrong:**
The unoptimized Vite development build (no minification, no tree-shaking, source maps included) gets embedded into the release binary. The Rust binary grows by 5-10 MB beyond what the optimized build would add. Worse: uncompressed assets are served from memory with no `Content-Encoding` negotiation, inflating transfer size to the browser on every page load.

**Why it happens:**
Developers run `npm run build` without verifying it is a production build. Vite defaults to development mode when `NODE_ENV` is not set. Source maps may be embedded by default depending on vite.config.ts. rust-embed embeds whatever is in `dist/` — it does not validate asset quality.

**How to avoid:**
- `vite.config.ts` should explicitly set `build.minify: true`, `build.sourcemap: false`, `build.cssMinify: true`.
- CI `npm run build` must set `NODE_ENV=production` (Vite respects this).
- Use `rollup-plugin-visualizer` once to understand bundle composition; ensure Preact + all UI code stays under ~100 KB gzipped.
- Set `build.chunkSizeWarningLimit` to a low value (e.g., 50 KB) to catch accidental heavy dependency imports early.
- Alternatively, use `memory-serve` instead of plain rust-embed — it compresses assets with brotli at compile time and serves with proper `Content-Encoding` headers automatically.

**Warning signs:**
- Binary grows by more than 2-3 MB when enabling the `dashboard` feature
- `dist/` directory contains `.map` files
- Response headers from `/ui/` lack `Content-Encoding: br` or `Content-Encoding: gzip`
- `ls -la dist/index.js` shows file size over 200 KB

**Phase to address:**
Phase 1 (build pipeline) and Phase 2 (Vite configuration). Lock down `vite.config.ts` production build settings before embedding anything.

---

### Pitfall 4: Dynamic Tailwind Classes Purged in Production

**What goes wrong:**
A table status column renders `text-green-500` or `text-red-500` based on a runtime condition. The class name is constructed as a string: `` `text-${isActive ? 'green' : 'red'}-500` ``. Tailwind's purger never sees these class names in a static scan of the source files. Production build strips them. The dashboard renders without color — the bug only appears after `npm run build`, not during `vite dev`.

**Why it happens:**
Tailwind's JIT/purge scanner works by extracting string literals that look like class names from source files. It does not execute code. String concatenation at runtime generates class names that do not appear literally in any source file, so they are excluded from the production CSS bundle. This applies to Tailwind v3 and v4 alike — the v4 Rust-based engine uses the same static scanning approach.

**How to avoid:**
- Never construct Tailwind class names through string interpolation or concatenation.
- Always write full class names in source: `isActive ? 'text-green-500' : 'text-red-500'` (both literals present in the source file).
- Add to `tailwind.config.ts` safelist for any unavoidably dynamic patterns:
  ```ts
  safelist: ['text-green-500', 'text-red-500', 'bg-green-100', 'bg-red-100']
  ```
- Run a visual smoke test of the production build before shipping: `npx serve dist` and verify all conditional color states render correctly.

**Warning signs:**
- UI looks correct in `vite dev` but colors or spacing break after `npm run build`
- Any JSX like `` className={`${prefix}-${value}`} ``
- The Tailwind config has no `content` globs covering all `.tsx` files in the project
- The `@layer components` directive is used in Tailwind v4 instead of the v4-native `@utility` directive

**Phase to address:**
Phase 2 (Preact + Tailwind setup). Establish the full-literal class naming convention before writing any conditional styling.

---

### Pitfall 5: API Key Token Stored in localStorage — XSS Extraction Risk

**What goes wrong:**
The dashboard prompts the user to enter their `mnk_...` API key on first load. The key is saved to `localStorage`. Any injected script (XSS, browser extension with content script permissions, compromised CDN asset) can read `localStorage` and exfiltrate the key. Because the mnemonic API key is a bearer token with full read/write access to all memories, leaking it exposes all agent memory data.

**Why it happens:**
`localStorage` is the simplest persistence mechanism in the browser. Developers reach for it first without considering that it is accessible to any JavaScript running in the same origin. For localhost-only deployments the practical risk is low, but the habit is dangerous if users start running mnemonic on network-accessible ports.

**How to avoid:**
For v1.6, use one of:
- **In-memory only (recommended):** Store the key in Preact component state or a Preact signal. The key is forgotten on page refresh — user re-enters it. Acceptable for a developer-facing operational tool where the session is intentionally short.
- **sessionStorage:** Cleared on tab close. Marginally better than localStorage. Still accessible to any same-origin JS — not a security boundary, just a convenience boundary.
- **Never store in localStorage** for a bearer token unless the user explicitly opts in with a visible warning.

Additional mitigation: serve all `/ui/` responses with `Content-Security-Policy` headers that block inline scripts and restrict script sources to `'self'`. This prevents the class of XSS that would read localStorage or component state in the first place.

**Warning signs:**
- `localStorage.setItem('mnk_token', ...)` anywhere in the frontend source (`git grep localStorage`)
- No CSP header on `/ui/` responses
- API key is visible in the Application > Local Storage tab in browser devtools after a page refresh

**Phase to address:**
Phase 3 (auth flow integration). Make the storage decision explicit in the first implementation — not a "we'll revisit later" item.

---

### Pitfall 6: CORS Misconfiguration on the axum Router Breaks Credentialed Requests

**What goes wrong:**
The dashboard SPA is served at `http://localhost:8080/ui/`. It calls the REST API at `http://localhost:8080/memories`. This is same-origin — CORS does not apply, and there is no issue. However: a developer adds a global CORS layer to axum with `Access-Control-Allow-Origin: *` to enable external tool access. If the dashboard's fetch code uses `credentials: 'include'`, browsers reject responses that pair wildcard origin with credentials. The fetch fails silently with a CORS error.

**Why it happens:**
The CORS spec prohibits `Access-Control-Allow-Origin: *` combined with `Access-Control-Allow-Credentials: true`. Developers add a permissive CORS layer for legitimate external API access without realizing the dashboard's fetch code uses credential-mode requests. The mismatch only surfaces as a browser console error, not a Rust compile error.

**How to avoid:**
- Since the dashboard is same-origin, do not use `credentials: 'include'` in any fetch call. Use explicit `Authorization: Bearer ${token}` headers instead. Same-origin fetch sends cookies automatically without the credentials flag anyway.
- If a CORS layer is added to the axum router, reflect the specific request origin back instead of using wildcard — or list allowed origins explicitly.
- The `Authorization: Bearer mnk_...` header pattern is the correct and consistent auth mechanism for the dashboard. Cookies are not part of the mnemonic auth model.

**Warning signs:**
- Browser console: `CORS error: The value of the 'Access-Control-Allow-Origin' header must not be the wildcard '*' when the request's credentials mode is 'include'`
- `credentials: 'include'` in any dashboard fetch utility
- `tower_http::cors::CorsLayer::very_permissive()` or `allow_origin(Any)` in the axum router

**Phase to address:**
Phase 3 (API integration). Document the auth header pattern in the first fetch utility written for the dashboard.

---

### Pitfall 7: Cache Busting Broken for Embedded Assets After Binary Upgrade

**What goes wrong:**
A user upgrades mnemonic (new binary). Their browser still has the old `index.html` cached from the previous version. The old `index.html` references hashed asset filenames from the old build (e.g., `index.AbCd1234.js`). The new binary embeds new hashed filenames (e.g., `index.EfGh5678.js`). The old cached `index.html` references files that no longer exist in the new binary. The dashboard shows a blank page or JS errors after every upgrade.

**Why it happens:**
Vite correctly adds content hashes to JS/CSS filenames, but `index.html` itself has no hash in its filename — it is always served at `/ui/index.html`. If the browser caches `index.html` aggressively, it will keep serving the old HTML pointing to old hashed filenames that are no longer available after a binary upgrade.

**How to avoid:**
- Set `Cache-Control: no-cache` on `index.html` responses specifically. This forces re-validation on every page load.
- Set `Cache-Control: max-age=31536000, immutable` on hashed JS/CSS assets — they are content-addressed and safe to cache forever since the filename changes when the content changes.
- Add custom response headers in the axum embedded-asset handler:
  - Paths ending with `.html` or `index.html` specifically → `Cache-Control: no-cache`
  - Paths matching `*.js`, `*.css` with a content hash in the filename → `Cache-Control: max-age=31536000, immutable`

**Warning signs:**
- All embedded files served with default browser caching (no explicit `Cache-Control` header)
- `index.html` served with `Cache-Control: max-age=3600` or any positive max-age
- After binary upgrade, dashboard shows JS errors about missing chunk files in the browser console

**Phase to address:**
Phase 2 (Vite build config + static asset serving). Set the cache policy in the same phase as writing the embedded-asset handler.

---

### Pitfall 8: `dashboard` Feature Flag Creates a Non-Additive Build Dependency

**What goes wrong:**
The `dashboard` Cargo feature is added to `Cargo.toml`. It gates `rust-embed` which has a proc-macro that runs at compile time. When the `ui/dist/` directory is missing, the proc-macro panics: `thread 'main' panicked at 'rust-embed: folder 'ui/dist' does not exist'`. This breaks `cargo build --features dashboard` for every contributor who has not run the frontend build — including CI that adds the feature flag without building the frontend first.

**Why it happens:**
rust-embed's `#[derive(RustEmbed)]` macro calls `include_dir!` or equivalent at compile time. If the target directory does not exist, the macro errors at build time with a message that appears in generated code, making it confusing to diagnose.

**How to avoid:**
- Gate the embedded assets struct behind `#[cfg(feature = "dashboard")]`. The struct must not exist when the feature is disabled — no struct definition, no derive macro invocation.
- The `ui/dist/` directory must be created and populated as a prerequisite whenever building with `--features dashboard`. Document this in `CONTRIBUTING.md`.
- Consider a `build.rs` guard: if `CARGO_FEATURE_DASHBOARD` is set but `ui/dist/index.html` does not exist, emit `cargo:warning=...` and `compile_error!("Run npm run build before cargo build --features dashboard")`.
- The `dashboard` feature must be additive in the Cargo sense — enabling it adds behavior, disabling it never removes existing functionality.

**Warning signs:**
- `cargo build` (default features, no `--features dashboard`) fails after a contributor has checked in dashboard-related code
- The `#[derive(RustEmbed)]` struct is outside a `#[cfg(feature = "dashboard")]` block
- CI fails with a rust-embed proc-macro panic on default builds

**Phase to address:**
Phase 1 (feature flag structure). Get the `#[cfg(feature = "dashboard")]` gate right before writing any embedded-assets code.

---

### Pitfall 9: axum Routes for Dashboard Registered Without a Feature Gate

**What goes wrong:**
The `/ui` route is registered in `main.rs` or `serve.rs` regardless of whether `--features dashboard` was compiled. In default builds (no dashboard feature), the route handler references a type that only exists under the feature flag. The build fails with a confusing type-not-found error on default `cargo build`.

**Why it happens:**
Route registration and the asset-serving handler are written together in a single refactor. The developer adds the route to the router but does not wrap it in `#[cfg(feature = "dashboard")]`. The error only appears when someone builds the non-dashboard variant — which in practice means in CI, not locally.

**How to avoid:**
- Wrap both the route definition and all referenced dashboard types in `#[cfg(feature = "dashboard")]`.
- Extract all dashboard-specific code into `src/dashboard/mod.rs` and gate the entire module: `#[cfg(feature = "dashboard")] mod dashboard;` in `main.rs`.
- Add a CI step that explicitly builds the release binary without `--features dashboard` and runs the existing test suite. This is the regression gate for the default binary.

**Warning signs:**
- `cargo build` (default features) fails after any dashboard-related commit
- Dashboard route is in the main router initialization function without a `#[cfg(feature = "dashboard")]` guard
- No CI job separately builds the default binary alongside the dashboard build

**Phase to address:**
Phase 1 (feature flag + axum router integration). The conditional compilation boundaries must be in place before any handler code is written.

---

### Pitfall 10: Frontend Build Pipeline Not Integrated Into the Release CI Workflow

**What goes wrong:**
The GitHub Actions release workflow builds and publishes the `mnemonic` binary for linux-x86_64, macos-x86_64, macos-aarch64. The workflow does not run `npm ci && npm run build` before `cargo build --release --features dashboard`. The released binary has an empty embedded UI directory. Users who download the release binary open `/ui/` and see a 404 or blank page. This is only discovered after the release tag is published.

**Why it happens:**
The release workflow was written for a pure Rust project. Adding a frontend build step requires modifying YAML that most contributors do not touch. The failure only manifests on release tag pushes — the CI path that triggers the release workflow — not on normal PRs.

**How to avoid:**
- Add a `setup-node` step and `npm ci && npm run build` step in the CI release matrix before the `cargo build` step. Key it to the `ui/` working directory.
- Add a `build.rs` `println!("cargo:rerun-if-changed=ui/dist/index.html")` so incremental builds detect frontend changes.
- Test the full release CI path using `act` or by pushing a pre-release tag before the milestone release.
- After the release CI runs, verify binary size has grown by the expected amount (a meaningful, non-zero delta when dashboard feature is included).

**Warning signs:**
- The release workflow YAML has no `actions/setup-node` step
- `ui/dist/` is in `.gitignore` but no CI step generates it before embedding
- Release binary responds to `GET /ui/` with 404 or a 0-byte body

**Phase to address:**
Phase 1 (CI integration). The release workflow update belongs in the same phase as the rust-embed integration, not deferred.

---

### Pitfall 11: Vite Base Path Mismatch for Assets Under a Subpath

**What goes wrong:**
Vite builds the SPA with the assumption it is served at `/` (root). It generates asset references like `<script src="/assets/index.AbCd.js">`. The binary serves the SPA at `/ui/`. The browser loads `/ui/index.html` but then fetches `/assets/index.AbCd.js` (without the `/ui/` prefix) — a 404 because the binary serves assets under `/ui/assets/`. The dashboard renders as a blank page.

**Why it happens:**
Vite's default `base` configuration is `/`. Developers test with `vite dev` which serves from root, and everything works. The mismatch only appears when the built output is served from a subpath.

**How to avoid:**
Set `base: '/ui/'` in `vite.config.ts`. This makes all asset references in the Vite build output relative to `/ui/`:
```ts
export default defineConfig({
  base: '/ui/',
  // ...
})
```
Verify by inspecting the generated `dist/index.html` — `<script src="/ui/assets/index.*.js">` should appear, not `<script src="/assets/index.*.js">`.

**Warning signs:**
- Assets load in `vite dev` but fail after embedding in the Rust binary
- Browser network tab shows requests to `/assets/...` returning 404 when the app is served at `/ui/`
- `dist/index.html` references absolute paths that do not include `/ui/` prefix

**Phase to address:**
Phase 1 (Vite configuration). This must be set before the first embedded build — it affects every generated asset path.

---

### Pitfall 12: Auth Middleware Applied to Static Asset Routes

**What goes wrong:**
The existing axum `route_layer()` auth middleware is applied too broadly and intercepts requests to `/ui/` static assets. The browser requests `index.html` or `index.js` — both have no way to pass an `Authorization: Bearer` header (the browser fetches them as plain resource loads). Every request to the dashboard gets a 401. The dashboard is inaccessible even when no API keys are configured (open mode).

**Why it happens:**
The existing auth middleware pattern in mnemonic is applied via `route_layer()` which protects `/memories/*` and `/keys/*`. When routing is refactored for v1.6, it is easy to accidentally apply the middleware to a router that now includes `/ui/*` routes as well.

**How to avoid:**
- Do not apply the auth middleware to any `/ui/*` routes — static assets have no mechanism to present credentials.
- Nest the dashboard routes in a separate router that has no auth layer applied.
- Auth for API calls made by the dashboard JavaScript is enforced in the JS fetch layer (explicit `Authorization` header), not via Rust middleware on the asset delivery routes.
- Add a test that requests `/ui/` with no `Authorization` header and asserts a 200 response regardless of auth mode.

**Warning signs:**
- `GET /ui/` returns 401 when auth is enabled
- The router setup applies `route_layer()` to a router that includes both API routes and dashboard routes
- Dashboard is inaccessible in any auth-enabled deployment

**Phase to address:**
Phase 1 (axum router integration). The routing structure must separate static asset delivery from API endpoints before either is implemented.

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Serving uncompressed assets from plain rust-embed | Zero extra deps, simple handler | No brotli/gzip; poor performance on slow connections; no automatic ETags | Acceptable for v1.6 MVP on localhost; add compression or switch to memory-serve before any network-exposed deployment |
| No CSP header on `/ui/` responses | Simpler handler code | XSS can read any in-memory token state | Never acceptable — add CSP even in MVP; it is a one-line addition to the response handler |
| In-memory token state only (no persistence) | No localStorage risk | User must re-enter API key on every page refresh | Acceptable for v1.6 developer-focused tool; reassess if users request persistence |
| Hash routing instead of history routing | No server-side fallback needed | Slightly dated URL aesthetics | Acceptable — operational dashboard, not a consumer product |
| Skip Tailwind production build verification in CI | Faster CI iteration | Dynamic class names silently disappear in production CSS | Never acceptable — add a `NODE_ENV=production npm run build` step in CI from day one |
| Defer cache header configuration | Ship faster | Browser serves stale dashboard after binary upgrade | Never acceptable — the cache policy is three lines of code and prevents a class of user-visible breakage |

---

## Integration Gotchas

Common mistakes when connecting the embedded dashboard to the existing mnemonic axum stack.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| axum router + dashboard routes | Registering `/ui/*` as a `nest()` without fallback causes 404 on SPA sub-routes | Use hash routing (no fallback needed) OR add a fallback handler returning `index.html` for extensionless paths under `/ui/` |
| rust-embed + `#[cfg(feature)]` | Placing the `#[derive(RustEmbed)]` struct outside the feature gate | Wrap the entire `Assets` struct in `#[cfg(feature = "dashboard")]` |
| axum auth middleware + `/ui/` | Applying the existing `route_layer()` to a router that also includes `/ui/*` | Separate the dashboard router from the API router; never apply auth middleware to static asset routes |
| Bearer token in fetch requests | Using `credentials: 'include'` (cookies) instead of explicit header | Use explicit `Authorization: Bearer ${token}` in every fetch call from the dashboard; cookies are not part of the mnemonic auth model |
| Vite base path | Building Vite with default base `/` and serving from `/ui/` | Set `base: '/ui/'` in `vite.config.ts`; verify `dist/index.html` references include the prefix |
| ETag headers | rust-embed returns identical bytes with no ETag; browsers re-download unchanged assets on every load | Add ETag headers derived from a content hash, or use `memory-serve` which does this automatically |
| Embedding API route calling full-init path | `GET /ui/` or a dashboard fetch to `GET /health` triggers the embedding model init path | Ensure dashboard-serving code and stats API calls use the DB-only init tier, not the full embedding+LLM init tier |

---

## Performance Traps

Patterns that work at small scale but degrade under usage.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| All memories loaded client-side for display | Dashboard fetch returns thousands of rows; browser hangs rendering the list | Add server-side pagination from the first implementation; never fetch unbounded lists | Breaks visibly at ~500 memories in SQLite; earlier for Qdrant/Postgres with large payloads |
| Auto-polling for live updates | Dashboard polls `/memories` every second to show recent activity; hammers the embedding model hot path | Never auto-poll at less than a 10-second interval; use a manual refresh button instead | Immediate — any sub-5s polling interval creates noticeable server load |
| Dashboard triggers embedding model init | Requesting `/health` or a stats endpoint causes the full init path (embedding model load) on every page load | Route dashboard stats API calls through the DB-only init path (`init_db`); the existing `init_recall()` fast-path already exists for this purpose | Immediate — 2-3 second latency on every page load if the wrong init tier is triggered |
| Uncompressed assets served from memory | Every page load transfers 500 KB+ of uncompressed JS/CSS | Use `memory-serve` for automatic brotli/gzip, or manually compress assets and set correct `Content-Encoding` response headers | Noticeable on any connection slower than gigabit LAN |

---

## Security Mistakes

Domain-specific security issues for an embedded dashboard serving a data API.

| Mistake | Risk | Prevention |
|---------|------|------------|
| No Content-Security-Policy on `/ui/` responses | XSS can read in-memory token state or inject fetch calls to exfiltrate data | Add `Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'` to all `/ui/` asset responses in the axum handler |
| Applying API auth middleware to static asset routes | Dashboard is inaccessible to any user, including when auth is disabled | Do not apply `route_layer()` auth to static asset routes; auth for API calls is enforced in the JS fetch layer, not on asset delivery |
| Wildcard CORS + fetch credentials | `Access-Control-Allow-Origin: *` combined with any credentialed request mode causes browser rejection | Use explicit `Authorization` headers; if CORS is needed, reflect the specific request origin rather than using wildcard |
| Dashboard accessible but API returns 401 with no UI guidance | Users with auth enabled reach `/ui/` but have no feedback about needing an API key | Handle 401 responses explicitly in all fetch calls; display a human-readable "enter your API key" prompt |
| Bearer token exfiltrated via localStorage | Attacker reads `localStorage` on compromise of any same-origin content | Store token in component state (in-memory only); add CSP headers; never `localStorage.setItem` with the bearer token |

---

## UX Pitfalls

Common user experience mistakes specific to embedded operational dashboards.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| No loading state during API fetch | Dashboard appears frozen for 200-500ms on first load | Add skeleton loaders or a spinner; never show a blank UI while data is loading |
| Flat list of all memories without grouping | 500+ memories render as an unnavigable scrollable wall | Group by agent, then by session; add pagination or virtual scrolling from the start |
| Compaction trigger with no confirmation | User accidentally triggers compaction, which is a destructive data mutation | Require explicit confirmation modal; show the dry-run diff before offering the commit button |
| No error state when API key is wrong or missing | Blank dashboard with no explanation of why data is not loading | Catch 401/403 responses and display a human-readable "Enter your API key" prompt |
| Dashboard crashes on empty database | Fresh install has zero memories; JS crashes on `undefined.map(...)` | Design and test empty states first; check for empty lists before rendering any collection |

---

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **rust-embed integration:** Compiles in debug mode — verify the release binary actually serves `/ui/`: `cargo build --release --features dashboard && ./target/release/mnemonic serve`, then `curl -s http://localhost:8080/ui/ | head -5` and confirm HTML is returned
- [ ] **Tailwind purge:** Styles look correct in dev — verify production build: `NODE_ENV=production npm run build`, then visually inspect all conditional color and state styles in the `dist/` output
- [ ] **Feature flag isolation:** Dashboard builds — verify default binary still builds and passes tests: `cargo build` (no `--features dashboard`) and `cargo test` must both pass
- [ ] **CI pipeline:** Build works locally — verify the release CI workflow builds and embeds the frontend by checking binary size increase and smoke-testing `/ui/` against the CI artifact
- [ ] **Cache headers:** Assets are served — verify `Cache-Control` headers: `curl -I http://localhost:8080/ui/` should show `Cache-Control: no-cache` for `index.html`, and `Cache-Control: max-age=31536000, immutable` for hashed JS/CSS
- [ ] **Auth flow:** API calls return data — verify 401 handling: test with no API key configured (open mode) AND with a key configured but not provided to the dashboard; both must show a usable state
- [ ] **SPA routing:** Navigation works via link clicks — verify hard reload on a non-root route works (or confirm hash routing is in use everywhere and no history routing exists)
- [ ] **Vite base path:** Assets load in dev — verify asset paths work when deployed at `/ui/`: inspect generated `dist/index.html` to confirm all asset references include the `/ui/` prefix
- [ ] **Static assets not behind auth middleware:** `curl http://localhost:8080/ui/` with no `Authorization` header returns 200 even when API keys are configured
- [ ] **No localStorage token storage:** `git grep -r "localStorage" ui/` returns no matches

---

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| SPA 404 on reload | LOW | Switch to hash routing in the Preact router config; no Rust changes needed; rebuild frontend; re-embed |
| Tailwind classes purged | LOW | Add missing class names to safelist in `tailwind.config.ts`; re-run production build; rebuild binary |
| Binary bloat from unoptimized assets | LOW | Fix `vite.config.ts` production build settings; re-run `npm run build`; rebuild binary; binary size delta should drop |
| Cache busting broken after upgrade | MEDIUM | Add `Cache-Control: no-cache` to `index.html` responses in the axum handler; re-release binary; instruct users to hard-refresh |
| Feature flag breaks default build | MEDIUM | Move the `#[derive(RustEmbed)]` struct inside `#[cfg(feature = "dashboard")]`; audit all dashboard types for leakage outside the gate |
| API key stored in localStorage, exposure suspected | HIGH | The `mnk_...` key is revocable — revoke via `mnemonic keys revoke <id>` and issue a new key immediately; document key rotation procedure in the dashboard UI |
| Release binary missing UI (CI missing frontend build step) | LOW | Add `npm ci && npm run build` to the release workflow YAML; re-trigger the release; no source code changes needed |
| Vite base path wrong after first release | MEDIUM | Fix `base` in `vite.config.ts`; rebuild frontend and binary; existing bookmarks to `/ui/` will still work since `index.html` path is unchanged |

---

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| SPA 404 on reload | Phase 1 — routing decision | Automated: `GET /ui/some-path` returns 200 with `text/html` content type |
| rust-embed debug/release divergence | Phase 1 — CI pipeline | Automated: release CI builds and smoke-tests `GET /ui/` on the release binary |
| Binary size bloat | Phase 1 — Vite config, Phase 2 — Preact setup | Manual: `ls -lh target/release/mnemonic` delta when adding dashboard feature is under 3 MB |
| Tailwind class purge | Phase 2 — component development | Manual: visual check of production build conditional styles; all state-conditional colors render correctly |
| localStorage token storage | Phase 3 — auth flow | Code review: `git grep localStorage ui/` returns zero hits |
| CORS misconfiguration | Phase 3 — API integration | Automated: fetch calls in production-built dashboard loaded from the binary succeed without CORS errors |
| Cache busting | Phase 2 — asset serving | Manual: `curl -I http://localhost:8080/ui/` shows `no-cache` for HTML; `immutable` for hashed assets |
| Feature flag non-additive | Phase 1 — feature flag structure | Automated CI: `cargo build` (no feature flag) passes after every dashboard-related commit |
| Route registered without cfg gate | Phase 1 — router integration | Automated CI: same `cargo build` default-features CI check |
| Build pipeline not in CI | Phase 1 — CI workflow | Automated: release CI job produces a binary that serves a non-empty `/ui/` response |
| Vite base path mismatch | Phase 1 — Vite configuration | Manual: inspect generated `dist/index.html` for `/ui/` prefixed asset paths |
| Auth middleware on static assets | Phase 1 — router structure | Automated: `GET /ui/` with no auth header returns 200 in auth-enabled mode |

---

## Sources

- [rust-embed docs.rs — debug vs release behavior](https://docs.rs/rust-embed/latest/rust_embed/trait.RustEmbed.html) — confirmed debug reads from filesystem, release embeds at compile time (HIGH confidence)
- [pyrossh/rust-embed GitHub issues — debug-embed feature](https://github.com/pyrossh/rust-embed/issues/50) — `debug-embed` feature forces compile-time embedding even in debug builds (MEDIUM confidence)
- [axum discussion — SPA hosting with embedded files](https://github.com/tokio-rs/axum/discussions/1309) — ServeDir cannot load embedded files; custom handler required for SPA fallback (HIGH confidence)
- [marending.dev — How to host SPA files in Rust](https://www.marending.dev/notes/rust-spa/) — rust-embed + axum SPA pattern; binary size warning for large sites (MEDIUM confidence)
- [Effective Rust, Item 26 — Feature flag pitfalls](https://effective-rust.com/features.html) — feature flags must be additive; non-additive features cause downstream build failures (HIGH confidence)
- [Cargo Book — Features](https://doc.rust-lang.org/cargo/reference/features.html) — feature unification, additive requirement, combinatorial explosion (HIGH confidence)
- [tailwindlabs/tailwindcss GitHub discussion #7568](https://github.com/tailwindlabs/tailwindcss/discussions/7568) — dynamic class generation causes purge to strip classes; full-literal class names required; safelist pattern (HIGH confidence)
- [tailwindlabs/tailwindcss GitHub discussion #17526](https://github.com/tailwindlabs/tailwindcss/discussions/17526) — Tailwind v4 `@layer components` vs `@utility` difference; static scanning applies in v4 as in v3 (MEDIUM confidence)
- [memory-serve docs.rs](https://docs.rs/memory-serve) — brotli compression at compile time, automatic ETag headers, Cache-Control support; superior to plain rust-embed for production web serving (HIGH confidence)
- [Auth0 Token Storage best practices](https://auth0.com/docs/secure/security-guidance/data-security/token-storage) — localStorage XSS risk; in-memory storage recommended for tokens (HIGH confidence)
- [MDN CORS guide](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CORS) — wildcard origin incompatible with credentials mode; explicit origin required (HIGH confidence)
- [Vite build docs](https://vite.dev/guide/build) — production build settings, sourcemap config, `base` path for subdirectory deployment (HIGH confidence)
- [Vite discussion — cache busting with hashed chunks](https://github.com/vitejs/vite/issues/6773) — content-hash filenames for assets; `index.html` must not be long-cached (MEDIUM confidence)
- [nickb.dev — Trade-offs in embedding data in Rust](https://nickb.dev/blog/a-quick-tour-of-trade-offs-embedding-data-in-rust/) — compile time impact of proc-macro-based embedding approaches (MEDIUM confidence)
- [preactjs/preset-vite GitHub](https://github.com/preactjs/preset-vite) — official Vite preset for Preact; current recommended scaffolding (HIGH confidence)

---
*Pitfalls research for: Embedded web dashboard — Rust binary (mnemonic v1.6)*
*Researched: 2026-03-22*
