---
phase: 30
reviewers: [gemini, codex]
reviewed_at: "2026-03-22T20:15:00Z"
plans_reviewed: [30-01-PLAN.md, 30-02-PLAN.md]
---

# Cross-AI Plan Review — Phase 30

## Gemini Review

Phase 30 provides a robust, non-intrusive foundation for embedding a web-based dashboard into the Mnemonic binary. By leveraging Rust feature flags and `rust-embed`, the implementation ensures zero overhead for the core "Redis of agents" use case while enabling a modern Preact-based UI for users who opt-in. The plan is technically sound, follows contemporary Rust/Vite best practices, and includes a rigorous CI/CD strategy that protects existing stability through mandatory regression gates and dual-artifact releases.

### Strengths
- **Compile-Time Safety:** Utilizing `rust-embed`'s compile-time folder validation (D-13) directly satisfies the requirement to fail the build if assets are missing, preventing "empty UI" runtime bugs.
- **Minimalist Core Preservation:** The use of `dep:` syntax and conditional module inclusion ensures that the dashboard dependencies do not bloat the default binary or slow down standard compilation.
- **Modern Frontend Stack:** Choosing Tailwind v4 and Vite with `singlefile` optimization aligns with the project's "single binary, zero dependency" ethos, making the UI as portable as the backend.
- **Developer Experience (DX):** The Vite dev server proxy setup (Plan 30-01, Task 1) ensures a seamless workflow for frontend development without needing to recompile the Rust binary for every UI change.
- **Rigorous Validation:** The CI strategy (Plan 30-02, Task 2) of running a separate regression job for the default features is excellent for maintaining Mnemonic's reputation as a stable, lightweight tool.

### Concerns
- **Asset Synchronization (LOW):** Plan 30-01 assumes `dashboard/dist/` exists. If a local developer runs `cargo build --features dashboard` without having run `npm run build` first, the build will fail. This is technically a "Success Criterion" (BUILD-05), but it may cause friction for contributors.
- **Axum Route Matching (MEDIUM):** Axum's `nest_service` at `/ui` can sometimes be sensitive to trailing slashes (e.g., `/ui` vs `/ui/`). While Task 2 mentions testing both, `axum-embed` behavior with `nest_service` needs to be verified to ensure that assets like `/ui/index.js` (if single-file fails) are resolved correctly relative to the nested path.
- **SPA Fallback and Asset Paths (LOW):** When using `FallbackBehavior::Ok` for SPA routing, if the app is served at `/ui/`, the HTML must use relative paths (e.g., `./assets/...`) or Vite's `base: '/ui/'` config to ensure assets load correctly when the user is at `/ui/settings`. `vite-plugin-singlefile` largely mitigates this by inlining everything, but the "fallback" multi-file mode (D-09) would be vulnerable.

### Suggestions
- **Explicit Vite Base:** In Plan 30-01, Task 1, ensure `vite.config.ts` sets `base: './'` or `base: '/ui/'`. This ensures that if `vite-plugin-singlefile` is disabled or fails, the generated `index.html` correctly references its assets when nested behind Axum's `/ui` prefix.
- **Build Script Safety:** Consider adding a simple `build.rs` that prints a warning (or `cargo:warning`) if the `dashboard` feature is enabled but `dashboard/dist/index.html` is older than the files in `dashboard/src/`. This helps local developers realize their UI is out of sync with their source.
- **Health Check Timeout:** In `HealthCard.tsx`, ensure the fetch to `/health` has a reasonable timeout. Since this is the "proof of life," a hanging backend should be reflected as an "Error/Timeout" state rather than infinite loading.
- **Standardized CI Node Version:** Specify the Node version in a `.nvmrc` or `.node-version` file within the `dashboard/` directory to ensure the CI and local developers are perfectly aligned on the environment.

### Risk Assessment
**Overall Risk: LOW**

The plan is well-contained and utilizes proven libraries (`rust-embed`, `axum`). The most significant risk—regressions in the core binary—is mitigated by the feature-gate architecture and the dedicated CI regression job. The frontend complexity is kept at a "proof-of-life" level, reducing the surface area for integration issues. The "Fallback to multi-file" (D-09) provides a safe exit ramp if the single-file inlining proves brittle with Tailwind v4.

---

## Codex Review

### Key Findings
- The biggest gap in 30-01 is that "single-file first" and "multi-file fallback accepted" are treated as compatible without adding asset-serving coverage for the fallback path.
- The biggest gap in 30-02 is that testing `dashboard::router()` alone does not prove the real contract: `/ui` mounted correctly inside `build_router(...)`.
- Neither plan explicitly closes success criterion 5 beyond relying on `rust-embed` behavior.

### Plan 30-01: Frontend scaffold + Rust feature gate wiring

**Summary:** Directionally solid and matches the current architecture well. The repo already uses optional features in Cargo.toml, and the public/protected router split in src/server.rs makes a top-level `/ui` mount reasonable. The main risk is not the feature gate itself; it is the unresolved asset model if `vite-plugin-singlefile` fails and the plan falls back to multi-file output.

**Strengths:**
- Reuses the existing optional dependency pattern already used for `interface-grpc`.
- Keeps dashboard wiring isolated behind a dedicated Cargo feature, which supports BUILD-02 cleanly.
- Mounting `/ui` outside the protected router is consistent with the current top-level router composition.
- Using `rust-embed` is a good fit for criterion 5 because missing asset folders fail at compile time.

**Concerns:**
- **HIGH:** The plan says "no external .css/.js files" and also "accept multi-file output if singlefile fails." Those are conflicting success conditions. If fallback is allowed, Rust serving and test coverage must explicitly support `/ui/assets/*`.
- **MEDIUM:** `/ui` versus `/ui/` behavior is left ambiguous. With nested services and prefix stripping, that detail can become user-visible and test-visible.
- **MEDIUM:** The build flow assumes `dashboard/dist/` exists before `cargo build --features dashboard`, but the plan does not include a developer-facing workflow or docs update explaining that prerequisite.
- **LOW:** Tailwind dark-theme variables and extra dev proxies for `/memories` and `/keys` are beyond the locked proof-of-life scope and add small scope creep.
- **LOW:** CI later depends on `npm ci`, but this plan does not explicitly call out committing `package-lock.json`.

**Suggestions:**
- Make the single-file path and fallback path explicit: if single-file works, assert there are no external asset references; if fallback is accepted, add explicit Rust-side and test-side verification for emitted asset files under `/ui/...`.
- Define one contract for `/ui`: either direct `200` or redirect to `/ui/`, then test that exact behavior.
- Add a small docs task for the `dashboard` feature build prerequisite and release artifact behavior.
- Keep Phase 30 frontend scope to `GET /health` only; defer extra proxied endpoints until they are used.

**Risk Assessment: MEDIUM** — The feature-gating approach is good, but the asset strategy is under-specified enough to allow a build that compiles and still ships a broken UI.

### Plan 30-02: Integration tests + CI release workflow

**Summary:** The CI intent is correct and aligned with the phase goal. The weak point is the test boundary. The plan currently validates the dashboard service too far down-stack and may miss the actual integration failure modes introduced by mounting it into the existing axum router.

**Strengths:**
- Uses the same `oneshot` testing style already established in tests/integration.rs.
- Adds a separate regression gate, which directly supports BUILD-03 and protects the default binary.
- Dual slim/dashboard artifacts per platform fit the locked release decision.
- Placing the Node build before the dashboard cargo build matches the compile-time embed requirement.

**Concerns:**
- **HIGH:** Testing `dashboard::router()` directly does not prove `/ui` is actually mounted correctly in `build_router(state)` from src/server.rs. The real contract is the merged top-level router, not the isolated dashboard module.
- **HIGH:** There is no test coverage for the multi-file fallback case. `/ui/` can return `200 text/html` while referenced JS/CSS assets still fail.
- **MEDIUM:** Criterion 5 is not explicitly verified. The plan relies on `rust-embed` behavior but does not include any smoke check or stated acceptance mechanism for "dist missing."
- **MEDIUM:** The regression job may be less stable than it looks because default-feature tests still include real `LocalEngine` coverage in tests/integration.rs, which can trigger first-run HuggingFace downloads.
- **LOW:** `cargo build --release + cargo test` is heavier than the stated gate and duplicates work already done in the release matrix.
- **LOW:** Checking for `"app"` in the HTML is brittle. A stable heading or root id is a better assertion.
- **LOW:** "200 or redirect" for `/ui` weakens the interface contract.

**Suggestions:**
- Make the dashboard tests exercise `mnemonic::server::build_router(test_state)` under `#[cfg(feature = "dashboard")]`, using the existing in-memory test-state pattern.
- Add one asset test tied to the actual build mode: single-file mode asserts no external asset references; multi-file mode extracts one emitted asset path from index.html and requests it through `/ui/...`.
- Add an explicit note or separate smoke check for the missing-`dist` compile failure requirement.
- Consider CI caching for npm, Cargo, and HuggingFace artifacts to reduce release-tag fragility.
- Use `cargo build` for the regression gate unless release-mode compilation is specifically required there too.

**Risk Assessment: MEDIUM-HIGH** — The workflow direction is right, but the test plan currently proves the wrong layer and leaves the main asset-serving failure mode partially untested.

---

## Consensus Summary

### Agreed Strengths
- **Feature gate architecture is solid** — Both reviewers agree the `dep:` syntax and `#[cfg(feature = "dashboard")]` pattern correctly follows established codebase conventions and cleanly isolates the dashboard from the default binary (BUILD-02).
- **CI regression gate is well-designed** — Both reviewers praise the separate regression job blocking the release, directly supporting BUILD-03.
- **Compile-time asset validation via rust-embed** — Both note that rust-embed's panic on missing folders naturally satisfies criterion 5 without custom code.
- **Minimal proof-of-life scope reduces risk** — Both agree the limited Phase 30 frontend complexity keeps integration risk low.

### Agreed Concerns
1. **Multi-file fallback path is under-specified (HIGH)** — Both reviewers flag that the plan accepts D-09 fallback but provides no asset-serving verification or test coverage for multi-file output. If singlefile fails, `/ui/` could return 200 text/html but reference broken asset URLs. **Action needed:** Either add explicit fallback-path tests or define a Vite `base` config that works in both modes.

2. **`/ui` vs `/ui/` trailing slash behavior is ambiguous (MEDIUM)** — Both flag that `nest_service` prefix stripping makes this detail user-visible. **Action needed:** Define and test the exact expected behavior for both paths.

3. **Developer workflow docs missing (MEDIUM)** — Both note there's no documentation explaining the `npm run build` prerequisite before `cargo build --features dashboard`. **Action needed:** Add a brief note to README or a developer guide section.

### Divergent Views
1. **Test boundary** — Gemini considers the test plan adequate at LOW overall risk. Codex rates it MEDIUM-HIGH, arguing that testing `dashboard::router()` in isolation misses the actual integration point (`build_router(state)`) and that at least one test should exercise the full merged router. **Worth investigating:** Codex's suggestion to test via `build_router(test_state)` would provide stronger coverage without much additional complexity.

2. **Scope creep** — Codex flags the Tailwind theme variables and extra proxy endpoints as minor scope creep beyond proof-of-life. Gemini does not flag these. **Assessment:** The theme variables are reused in Phase 31, so defining them now is reasonable forward investment. The extra proxy endpoints (`/memories`, `/keys`) could be deferred but are harmless.

3. **Overall risk** — Gemini rates LOW, Codex rates MEDIUM to MEDIUM-HIGH. The divergence stems from Codex's focus on the multi-file fallback gap and test-layer concern. **Assessment:** The actual risk is closer to LOW-MEDIUM given the D-09 fallback is explicitly defined and rust-embed handles multi-file serving correctly by default.
