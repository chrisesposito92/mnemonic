# Phase 25: Config Redaction Fix & Tech Debt Cleanup - Research

**Researched:** 2026-03-21
**Domain:** Rust CLI output redaction, dead code elimination, YAML frontmatter hygiene
**Confidence:** HIGH

---

## Summary

Phase 25 closes three discrete tech-debt items surfaced by the v1.4 milestone audit. All three are
mechanical fixes with zero ambiguity: exact file locations and line numbers are known from the audit
report.

**Item 1 — CONF-03 gap (primary requirement):** `mnemonic config show` redacts `openai_api_key`,
`llm_api_key`, and `qdrant_api_key` via `redact_option()` but outputs `postgres_url` in plain text
at `src/cli.rs` lines 199 (JSON path) and 217 (human-readable path). Postgres DSN URLs embed
credentials (`postgres://user:password@host/db`). Fixing both output paths is a two-line change.

**Item 2 — Dead code annotation (info-level tech debt):** `now_iso8601()` in
`src/storage/postgres.rs` (line 131) carries `#[allow(dead_code)]` because the function is only
called from a unit test, not from production code. The correct resolution is to move the function
inside `#[cfg(test)]` so the compiler can verify it is reachable, eliminating the suppress-warning
annotation. A companion test (`test_now_iso8601_format`) already exists and will remain.

**Item 3 — SUMMARY.md metadata gaps (info-level tech debt):** The v1.4 milestone audit found 13
requirements whose `requirements_completed` (or `requirements-completed`) frontmatter field is
missing from SUMMARY.md files in phases 21, 22, and 23. Phase 24 (both plans) already populates
this field correctly. The fix is surgical: add the missing YAML key to the correct frontmatter
sections in the affected files.

**Primary recommendation:** Address all three items in a single plan with three tasks; none
requires architectural decisions or new dependencies.

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CONF-03 | `mnemonic config show` displays current config with secret redaction for ALL secret fields | Bug is at `src/cli.rs:199` (JSON path) and `src/cli.rs:217` (human-readable path). Fix: apply `redact_option(&config.postgres_url)` in JSON output; apply `****` display-only pattern in human-readable output. Tests added in `#[cfg(test)]` block following existing `test_redact_option_*` pattern. |

</phase_requirements>

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| cargo test (built-in) | Rust 1.x | Test runner for unit tests | Already used; no new dependencies |
| serde_json | already in Cargo.toml | JSON serialization for `redact_option()` return value | Already used in `run_config_show()` |

### Supporting
No new dependencies required for this phase.

**Version verification:** No new packages — all dependencies are already present in `Cargo.toml`.

---

## Architecture Patterns

### Recommended Project Structure

No structural changes. All changes are within existing files:

```
src/
├── cli.rs               # Fix: postgres_url redaction in run_config_show()
└── storage/
    └── postgres.rs      # Fix: move now_iso8601() inside #[cfg(test)]
.planning/phases/
├── 21-storage-trait-and-sqlite-backend/
│   ├── 21-01-SUMMARY.md   # Add: requirements_completed frontmatter
│   └── 21-02-SUMMARY.md   # (already has requirements-completed)
├── 22-config-extension-backend-factory-and-config-cli/
│   ├── 22-01-SUMMARY.md   # Add: requirements_completed frontmatter
│   └── 22-02-SUMMARY.md   # Add: requirements_completed frontmatter
└── 23-qdrant-backend/
    ├── 23-01-SUMMARY.md   # Add: requirements_completed frontmatter
    └── 23-02-SUMMARY.md   # Add: requirements_completed frontmatter
```

### Pattern 1: postgres_url Redaction Fix

**What:** Apply `redact_option()` to `postgres_url` in `run_config_show()` matching the pattern
already used for `openai_api_key`, `llm_api_key`, and `qdrant_api_key`.

**JSON output path** (`src/cli.rs` line 199):
```rust
// BEFORE (plain text leak):
"postgres_url": config.postgres_url,

// AFTER (redacted):
"postgres_url": redact_option(&config.postgres_url),
```

**Human-readable output path** (`src/cli.rs` lines 216-218):
```rust
// BEFORE (plain text leak):
if let Some(ref url) = config.postgres_url {
    println!("  postgres_url     {}", url);
}

// AFTER (redacted):
if config.postgres_url.is_some() {
    println!("  postgres_url     ****");
}
```

This follows the identical pattern used for `qdrant_api_key` (lines 213-215) and `openai_api_key`
(lines 222-223).

**Test additions** — add to the `#[cfg(test)]` block in `cli.rs` following the existing
`test_redact_option_*` pattern but calling `run_config_show()` output is not directly testable.
Instead, add Nyquist tests that directly call `redact_option()` for a postgres-url-shaped value,
confirming the helper behaves correctly regardless of content format.

A more meaningful Nyquist test: construct a `Config` with `postgres_url = Some(...)`, confirm
`redact_option()` returns `"****"` for it. Since `run_config_show()` writes to stdout (side effect),
testing the redaction helper directly is the correct unit test strategy — same approach as the
existing `test_redact_option_some_returns_stars` and `test_redact_option_some_hides_actual_value`
tests.

For CONF-03 specifically, a test named
`test_conf03_postgres_url_redacted_same_as_api_keys` that:
1. Calls `redact_option(&Some("postgres://user:password@host/db".to_string()))`
2. Asserts result equals `serde_json::Value::String("****".to_string())`
3. Asserts result does not contain `"password"`

### Pattern 2: Move now_iso8601() into #[cfg(test)]

**What:** Remove `#[allow(dead_code)]` annotation and move function definition inside the
`#[cfg(test)] mod tests` block so it is only compiled in test builds.

**Why:** The function is only called by `test_now_iso8601_format` — it has no production call site.
Production code uses `DEFAULT NOW()` server-side for timestamps, not `now_iso8601()`. Moving it
inside the test module eliminates the dead code and removes the need for the suppression attribute.

```rust
// BEFORE (in production scope, lines 130-161):
#[allow(dead_code)]
fn now_iso8601() -> String { ... }

// AFTER (inside #[cfg(test)] mod tests block):
#[cfg(test)]
mod tests {
    use super::*;

    fn now_iso8601() -> String { ... }  // no allow needed — test-only

    #[test]
    fn test_now_iso8601_format() {
        let ts = now_iso8601();
        // ... assertions unchanged
    }
}
```

**Verification:** `cargo test --features backend-postgres --lib storage::postgres::tests::test_now_iso8601_format` must still pass.

### Pattern 3: SUMMARY.md requirements_completed Frontmatter

**What:** Add `requirements_completed` (or `requirements-completed` per the hyphen variant used in
existing files) to each affected SUMMARY.md YAML frontmatter.

**Correct hyphen variant:** Existing populated files (21-02 and 24-02) use `requirements-completed`
(hyphen). Follow that convention for consistency.

**Mapping (what to add where):**

| File | Field to Add |
|------|-------------|
| `21-01-SUMMARY.md` | `requirements-completed: [STOR-01, STOR-02]` |
| `21-02-SUMMARY.md` | Already has `requirements-completed: [STOR-03, STOR-04, STOR-05]` — no change needed |
| `22-01-SUMMARY.md` | `requirements-completed: [CONF-01, CONF-02]` |
| `22-02-SUMMARY.md` | `requirements-completed: [CONF-04]` (CONF-03 was partial and is closed in Phase 25) |
| `23-01-SUMMARY.md` | `requirements-completed: [QDRT-01]` (Plan 01 delivers store/get_by_id/delete — QDRT-01 foundation) |
| `23-02-SUMMARY.md` | `requirements-completed: [QDRT-01, QDRT-02, QDRT-03, QDRT-04]` (Plan 02 completes all 4) |

**Note on CONF-03:** The Phase 25 success criteria says CONF-03 is completed in Phase 25. Once
Phase 25 is verified, the traceability table in REQUIREMENTS.md should be updated to show CONF-03
as Complete. The Phase 22 SUMMARY files should NOT retroactively claim CONF-03 as complete since it
was only partial at that time.

### Anti-Patterns to Avoid

- **Re-testing redact_option():** Do not duplicate the three existing `test_redact_option_*` tests.
  Add one new Nyquist test specific to CONF-03 that targets the postgres-URL scenario.
- **Removing the test for now_iso8601():** The test `test_now_iso8601_format` verifies the JDN
  algorithm is correct. Keep the test; only move the function into the test module.
- **Using `requirements_completed` (underscore) in new SUMMARY files:** Existing files use the
  hyphen variant `requirements-completed`. Maintain consistency with the established convention.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Credential masking | Custom regex/URL parser | `redact_option()` helper already in cli.rs | Helper already handles Option<String> → Value correctly |
| Frontmatter update | YAML library | Direct text edit to SUMMARY.md files | These are Markdown docs, not parsed configs |

**Key insight:** All three items are surgical line-level edits to existing files. No new
infrastructure, helpers, or patterns are needed.

---

## Common Pitfalls

### Pitfall 1: Only Fixing One of the Two postgres_url Output Paths
**What goes wrong:** Fixing the JSON path but leaving the human-readable path shows the URL in
`mnemonic config show` (without `--json`) and vice versa.
**Why it happens:** The two output paths are in separate `if json_mode` branches at lines 185-237.
**How to avoid:** Fix both: line 199 (JSON `serde_json::json!` block) AND lines 216-218
(human-readable `println!` block).
**Warning signs:** Test `mnemonic config show` without `--json` flag after the fix.

### Pitfall 2: Moving now_iso8601() but Leaving Production Code Reference
**What goes wrong:** If any production code calls `now_iso8601()` after it moves into `#[cfg(test)]`,
the build will fail in release mode.
**Why it happens:** Assumption that the function is test-only without verifying call sites.
**How to avoid:** Grep confirms `now_iso8601` appears only at lines 131 (definition) and 532-533
(test) in postgres.rs. No production call sites exist. Verify with `cargo build` after the change.
**Warning signs:** `cargo build` (without `--tests`) fails after the move.

### Pitfall 3: requirements-completed Hyphen vs Underscore Inconsistency
**What goes wrong:** Using `requirements_completed` (underscore) in new entries when existing files
use `requirements-completed` (hyphen), creating two different field names in the same codebase.
**Why it happens:** The success criteria description uses underscore; existing files use hyphen.
**How to avoid:** Check `24-02-SUMMARY.md` (the reference file) — it uses `requirements-completed`.
Use hyphens in all new entries.
**Warning signs:** Audit tooling or search for `requirements-completed` misses the underscore variant.

### Pitfall 4: Claiming CONF-03 Complete in Phase 22 SUMMARY Files
**What goes wrong:** Adding CONF-03 to Phase 22 SUMMARY frontmatter retroactively, which misrepresents
when the requirement was completed.
**Why it happens:** CONF-03 was partially implemented in Phase 22 but the gap (postgres_url) was
only closed in Phase 25.
**How to avoid:** Phase 22-02-SUMMARY.md should list only CONF-04. Phase 25 closes CONF-03 and the
traceability table in REQUIREMENTS.md reflects Phase 25 as the completing phase.

---

## Code Examples

### Exact Bug Location (src/cli.rs)

```rust
// JSON output — BUGGY: postgres_url not redacted (line 199)
let obj = serde_json::json!({
    "port": config.port,
    "db_path": config.db_path,
    "storage_provider": config.storage_provider,
    "embedding_provider": config.embedding_provider,
    "openai_api_key": redact_option(&config.openai_api_key),   // correct
    "llm_provider": config.llm_provider,
    "llm_api_key": redact_option(&config.llm_api_key),          // correct
    "llm_base_url": config.llm_base_url,
    "llm_model": config.llm_model,
    "qdrant_url": config.qdrant_url,
    "qdrant_api_key": redact_option(&config.qdrant_api_key),    // correct
    "postgres_url": config.postgres_url,                         // BUG — plain text
});

// Human-readable output — BUGGY: postgres_url not redacted (lines 216-218)
if let Some(ref url) = config.postgres_url {
    println!("  postgres_url     {}", url);    // BUG — exposes credentials
}
```

### Fixed Code

```rust
// JSON output — FIXED:
"postgres_url": redact_option(&config.postgres_url),

// Human-readable — FIXED (matches qdrant_api_key pattern at lines 213-215):
if config.postgres_url.is_some() {
    println!("  postgres_url     ****");
}
```

### now_iso8601() Move Pattern

```rust
// REMOVE from production scope (delete lines 125-161 including #[allow(dead_code)]):
//   #[allow(dead_code)]
//   fn now_iso8601() -> String { ... }

// ADD inside #[cfg(test)] mod tests block (before test_now_iso8601_format):
#[cfg(test)]
mod tests {
    use super::*;

    /// ISO 8601 UTC timestamp for test assertions. Not used in production —
    /// production code relies on Postgres NOW() server-side.
    fn now_iso8601() -> String {
        use std::time::SystemTime;
        // ... (identical body to the removed function)
    }

    #[test]
    fn test_now_iso8601_format() {
        let ts = now_iso8601();
        // ... (unchanged)
    }
}
```

### Nyquist Test for CONF-03

```rust
// Add to #[cfg(test)] mod tests block in src/cli.rs:
#[test]
fn test_conf03_postgres_url_redacted_in_json() {
    // Verify that a postgres DSN (which may contain a password) is redacted
    let dsn = Some("postgres://user:secret@localhost/mnemonic".to_string());
    let result = redact_option(&dsn);
    assert_eq!(
        result,
        serde_json::Value::String("****".to_string()),
        "postgres_url must be redacted as ****"
    );
    let serialized = result.to_string();
    assert!(
        !serialized.contains("secret"),
        "redacted output must not contain password; got: {}",
        serialized
    );
}
```

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | Cargo.toml |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test` |
| Postgres-unit run | `cargo test --features backend-postgres --lib storage::postgres::tests` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CONF-03 (JSON) | `postgres_url` is `"****"` in JSON output | unit | `cargo test --lib cli::tests::test_conf03_postgres_url_redacted_in_json` | Wave 0 gap |
| CONF-03 (human) | `postgres_url` prints `****` in human-readable output | unit | `cargo test --lib cli::tests::test_conf03_postgres_url_redacted_in_json` (same test covers the helper) | Wave 0 gap |
| Dead code fix | `cargo build` succeeds after removing `#[allow(dead_code)]` | build | `cargo build` | N/A (build check) |
| now_iso8601 test | Function works correctly after move to test scope | unit | `cargo test --features backend-postgres --lib storage::postgres::tests::test_now_iso8601_format` | ✅ exists |

### Sampling Rate
- **Per task commit:** `cargo test --lib`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `test_conf03_postgres_url_redacted_in_json` in `src/cli.rs` — covers CONF-03 redaction behavior

---

## Sources

### Primary (HIGH confidence)

- Direct code inspection: `src/cli.rs` lines 186-237 (`run_config_show` function) — bug confirmed
- Direct code inspection: `src/storage/postgres.rs` lines 130-161 (`now_iso8601` with `#[allow(dead_code)]`) — dead code confirmed
- `.planning/v1.4-MILESTONE-AUDIT.md` — authoritative list of tech debt items and exact file locations
- `.planning/REQUIREMENTS.md` — CONF-03 requirement definition and traceability

### Secondary (MEDIUM confidence)

- Existing SUMMARY.md files (21-02, 24-02) — establish `requirements-completed` hyphen convention
- v1.4 milestone audit frontmatter — 13/17 requirements missing from SUMMARY.md confirmed by grep

### Tertiary (LOW confidence)

None — all findings are based on direct code inspection of the repository.

---

## Metadata

**Confidence breakdown:**
- Bug location: HIGH — exact lines confirmed by direct file reads
- Fix pattern: HIGH — identical pattern already used for other secrets (qdrant_api_key, openai_api_key)
- Dead code fix: HIGH — grep confirms only one call site (test), no production callers
- SUMMARY.md mapping: HIGH — requirements mapping cross-referenced against audit report and REQUIREMENTS.md traceability table

**Research date:** 2026-03-21
**Valid until:** Indefinite — findings are based on static analysis of the codebase, not external documentation
