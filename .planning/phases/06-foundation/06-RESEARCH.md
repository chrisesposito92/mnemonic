# Phase 6: Foundation - Research

**Researched:** 2026-03-20
**Domain:** Rust config extension, SQLite schema migration, error type hierarchy
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Config field naming and structure**
- Flat fields on Config struct: `llm_provider`, `llm_api_key`, `llm_base_url`, `llm_model` — mirrors existing `embedding_provider` / `openai_api_key` pattern
- All fields are `Option<String>` — LLM config is entirely opt-in
- Default `llm_provider` is `None` (no LLM) — Tier 1 algorithmic compaction works without any LLM config
- `llm_base_url` defaults to `https://api.openai.com/v1` when provider is `openai` — allows override for Azure/local endpoints
- `llm_model` defaults per-provider (e.g. `gpt-4o-mini` for `openai`) — overridable via `MNEMONIC_LLM_MODEL`
- Env var mapping: `MNEMONIC_LLM_PROVIDER`, `MNEMONIC_LLM_API_KEY`, `MNEMONIC_LLM_BASE_URL`, `MNEMONIC_LLM_MODEL`

**Config validation rules**
- `validate_config()` extended: if `llm_provider` is set to `"openai"`, require `llm_api_key` — same pattern as embedding_provider validation
- If `llm_provider` is `None`, all other llm_* fields are ignored (no error for orphaned keys)
- Unknown `llm_provider` values rejected at startup with clear error message

**Schema migration strategy**
- `source_ids` column added to memories table via `ALTER TABLE ADD COLUMN IF NOT EXISTS` in `db::open()` — inline with existing DDL
- `source_ids` is `TEXT NOT NULL DEFAULT '[]'` — JSON array of memory IDs, mirrors tags column pattern
- `compact_runs` table created with `CREATE TABLE IF NOT EXISTS` — idempotent
- No migration framework — inline DDL in `db::open()` is sufficient for this project's scale

**compact_runs table schema**
- Columns: `id TEXT PRIMARY KEY`, `agent_id TEXT NOT NULL`, `started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP`, `completed_at DATETIME`, `clusters_found INTEGER NOT NULL DEFAULT 0`, `memories_merged INTEGER NOT NULL DEFAULT 0`, `memories_created INTEGER NOT NULL DEFAULT 0`, `dry_run BOOLEAN NOT NULL DEFAULT 0`, `threshold REAL NOT NULL`, `status TEXT NOT NULL DEFAULT 'running'`
- `threshold` column captures the similarity threshold used for the run — enables auditing
- `status` tracks run state: 'running', 'completed', 'failed'
- Index on `agent_id` for efficient per-agent run history queries

**Error type additions**
- New `LlmError` enum in `error.rs` following existing pattern (ModelLoad, ApiCall, Timeout variants)
- Wire into `MnemonicError` as `#[error("llm error: {0}")] Llm(#[from] LlmError)`
- `CompactionError` deferred to Phase 8 — not needed for foundation

### Claude's Discretion
- Exact error message wording for LLM config validation failures
- Whether to add tracing::info for LLM config at startup (likely yes, mirrors embedding provider log)
- Internal ordering of ALTER TABLE statements in db::open()

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| LLM-01 | User can configure LLM provider via llm_provider and llm_api_key (mirrors embedding_provider pattern) | Config struct extension pattern confirmed in src/config.rs; figment Env provider handles MNEMONIC_ prefix automatically; validate_config() match-arm pattern confirmed; all four fields map cleanly onto existing Option<String> conventions |
</phase_requirements>

---

## Summary

Phase 6 is a pure Rust extension task with no new external dependencies. The work splits into three parallel tracks: (1) extend `src/config.rs` with four new `Option<String>` fields and a new validation arm, (2) extend `src/db.rs` with an `ALTER TABLE` and a `CREATE TABLE IF NOT EXISTS` block appended to the existing `execute_batch`, and (3) extend `src/error.rs` with a `LlmError` enum wired into `MnemonicError`. All three tracks touch different files with zero coupling between them.

The existing codebase provides complete reference implementations for every pattern needed. `Config` struct, `validate_config()`, `load_config()`, `db::open()` DDL, and the `thiserror` error hierarchy are all present and follow consistent conventions. This phase requires copying and adapting those patterns, not inventing anything new.

The critical constraint is idempotency: `db::open()` runs on every server start, so every schema change must use `ALTER TABLE ... IF NOT EXISTS` or `CREATE TABLE IF NOT EXISTS`. SQLite's `IF NOT EXISTS` clause on `ALTER TABLE` is supported since SQLite 3.37.0 (released 2021-11-27). The bundled SQLite version via `rusqlite = { version = "0.37", features = ["bundled"] }` is current enough to guarantee this.

**Primary recommendation:** Implement all three tracks in parallel (separate tasks), validate idempotency by calling `db::open()` twice on the same connection in tests, and verify config validation by adding unit tests mirroring the existing `test_validate_config_*` suite.

## Standard Stack

### Core (already in Cargo.toml — no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| figment | 0.10 | Config loading (Serialized defaults → Toml → Env) | Already used; handles MNEMONIC_ env prefix automatically |
| rusqlite | 0.37 (bundled) | SQLite DDL execution | Already used; bundled means no system lib dependency |
| thiserror | 2 | Derive macros for error enums | Already used for all existing error types |
| tokio-rusqlite | 0.7 | Async SQLite via conn.call() | Already used; all DDL runs through this wrapper |

### No New Dependencies

STATE.md and CONTEXT.md both confirm: no new external dependencies for this phase. reqwest 0.13 is already in Cargo.toml for Phase 7 LLM HTTP calls. This phase only adds fields, DDL, and error variants — zero Cargo.toml changes required.

**Installation:** None required.

## Architecture Patterns

### Recommended File Structure

This phase modifies three existing files only:

```
src/
├── config.rs    # Add 4 Option<String> fields + llm_provider validation arm
├── db.rs        # Append ALTER TABLE + CREATE TABLE + CREATE INDEX to execute_batch
└── error.rs     # Add LlmError enum + MnemonicError::Llm variant
```

### Pattern 1: Config Field Extension (figment flat-field pattern)

**What:** Add optional fields directly to the `Config` struct. `figment` maps `MNEMONIC_LLM_PROVIDER` → `llm_provider` automatically via the `Env::prefixed("MNEMONIC_")` layer.

**When to use:** Any new config value that can come from env var or TOML.

**Example (mirrors existing pattern exactly):**
```rust
// Source: src/config.rs (current)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub port: u16,
    pub db_path: String,
    pub embedding_provider: String,
    pub openai_api_key: Option<String>,
    // New fields — same pattern:
    pub llm_provider: Option<String>,
    pub llm_api_key: Option<String>,
    pub llm_base_url: Option<String>,
    pub llm_model: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 8080,
            db_path: "./mnemonic.db".to_string(),
            embedding_provider: "local".to_string(),
            openai_api_key: None,
            // LLM fields default to None
            llm_provider: None,
            llm_api_key: None,
            llm_base_url: None,
            llm_model: None,
        }
    }
}
```

**Key detail:** `llm_base_url` and `llm_model` defaults-per-provider are computed at use time (Phase 7), not stored in `Default`. The `Config` struct stores only what the user configured. Computing `https://api.openai.com/v1` as the effective URL is Phase 7's responsibility.

### Pattern 2: Config Validation Extension (match-arm pattern)

**What:** Extend the existing `validate_config()` match on `embedding_provider` with a separate guard for LLM fields. The LLM check is independent — it runs after the embedding check.

**When to use:** Whenever a new config dependency must be validated at startup before any I/O.

**Example:**
```rust
// Source: src/config.rs validate_config() (current pattern, extended)
pub fn validate_config(config: &Config) -> anyhow::Result<()> {
    // ... existing embedding_provider match ...

    // LLM validation (independent of embedding validation)
    if let Some(provider) = &config.llm_provider {
        match provider.as_str() {
            "openai" => {
                if config.llm_api_key.is_none() {
                    anyhow::bail!(
                        "llm_provider is \"openai\" but MNEMONIC_LLM_API_KEY is not set"
                    );
                }
            }
            other => {
                anyhow::bail!(
                    "unknown llm_provider {:?}: expected \"openai\"",
                    other
                );
            }
        }
    }
    Ok(())
}
```

### Pattern 3: Idempotent Schema Migration (inline DDL pattern)

**What:** Append `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` and `CREATE TABLE IF NOT EXISTS` to the existing `execute_batch` in `db::open()`. SQLite 3.37+ supports `IF NOT EXISTS` on `ALTER TABLE ADD COLUMN`.

**When to use:** Adding columns to existing tables or new tables that must survive repeated `db::open()` calls.

**Example:**
```rust
// Source: src/db.rs (current execute_batch, extended)
c.execute_batch(
    "
    PRAGMA journal_mode=WAL;
    PRAGMA foreign_keys=ON;

    CREATE TABLE IF NOT EXISTS memories (
        id TEXT PRIMARY KEY,
        content TEXT NOT NULL,
        agent_id TEXT NOT NULL DEFAULT '',
        session_id TEXT NOT NULL DEFAULT '',
        tags TEXT NOT NULL DEFAULT '[]',
        embedding_model TEXT NOT NULL DEFAULT '',
        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME
    );

    -- Phase 6: add source_ids column to existing memories table
    ALTER TABLE memories ADD COLUMN IF NOT EXISTS
        source_ids TEXT NOT NULL DEFAULT '[]';

    CREATE INDEX IF NOT EXISTS idx_memories_agent_id ON memories(agent_id);
    CREATE INDEX IF NOT EXISTS idx_memories_session_id ON memories(session_id);
    CREATE INDEX IF NOT EXISTS idx_memories_created_at ON memories(created_at);

    CREATE VIRTUAL TABLE IF NOT EXISTS vec_memories USING vec0(
        memory_id TEXT PRIMARY KEY,
        embedding float[384]
    );

    -- Phase 6: compaction audit log
    CREATE TABLE IF NOT EXISTS compact_runs (
        id TEXT PRIMARY KEY,
        agent_id TEXT NOT NULL,
        started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        completed_at DATETIME,
        clusters_found INTEGER NOT NULL DEFAULT 0,
        memories_merged INTEGER NOT NULL DEFAULT 0,
        memories_created INTEGER NOT NULL DEFAULT 0,
        dry_run BOOLEAN NOT NULL DEFAULT 0,
        threshold REAL NOT NULL,
        status TEXT NOT NULL DEFAULT 'running'
    );

    CREATE INDEX IF NOT EXISTS idx_compact_runs_agent_id ON compact_runs(agent_id);
    ",
)?;
```

### Pattern 4: Error Enum Extension (thiserror pattern)

**What:** Add a new `LlmError` enum following the exact shape of `EmbeddingError`, then add a `Llm` variant to `MnemonicError`.

**When to use:** Any new error domain needs its own enum so call sites get precise error types.

**Example:**
```rust
// Source: src/error.rs (current EmbeddingError pattern, mirrored for LLM)
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("LLM API call failed: {0}")]
    ApiCall(String),

    #[error("LLM request timed out")]
    Timeout,

    #[error("LLM response could not be parsed: {0}")]
    ParseError(String),
}

// Wire into MnemonicError:
pub enum MnemonicError {
    #[error("database error: {0}")]
    Db(#[from] DbError),

    #[error("config error: {0}")]
    Config(#[from] ConfigError),

    #[error("embedding error: {0}")]
    Embedding(#[from] EmbeddingError),

    // New:
    #[error("llm error: {0}")]
    Llm(#[from] LlmError),
}
```

### Anti-Patterns to Avoid

- **Nested LLM config struct:** User decision is flat fields on Config. Do not create `Config { llm: LlmConfig { ... } }` — figment flattening adds complexity and breaks the env var naming convention.
- **Versioned migration table:** No migration framework. The `IF NOT EXISTS` guards are sufficient for this project's scale.
- **ALTER TABLE in a transaction:** SQLite's `ALTER TABLE ADD COLUMN` cannot run inside an explicit transaction in some older versions. Running inside `execute_batch` without explicit `BEGIN` is safe — rusqlite's `execute_batch` uses implicit per-statement transactions for DDL.
- **Storing computed defaults in Config:** `llm_base_url` and `llm_model` effective values (e.g. `gpt-4o-mini`) are computed at Phase 7 call time, not in `Default::default()`. Storing opinionated defaults risks surprising users who set `llm_provider = "openai"` expecting to override the model.
- **Updating test_config() in integration tests without adding new fields:** The `test_config()` helper in `tests/integration.rs` constructs `Config` by name. After adding new fields to `Config`, the struct literal must include the new fields (all `None`). Missing fields will cause compile errors.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Env var → struct field mapping | Custom env var parser | figment `Env::prefixed("MNEMONIC_")` | Already wired; handles case conversion, missing vars as None |
| TOML config loading | Custom TOML parser | figment `Toml::file()` | Already wired; merge semantics handled |
| Error formatting | Custom Display impls | `thiserror::Error` derive | Consistent with all other errors in the project |
| Schema version tracking | Migration table | `IF NOT EXISTS` guards | Sufficient for single-file SQLite at this scale |

**Key insight:** Every mechanism for this phase already exists in the codebase. The task is extension, not invention.

## Common Pitfalls

### Pitfall 1: ALTER TABLE Column Count Assertion in Existing Test

**What goes wrong:** `test_schema_created` in `tests/integration.rs` line 93 asserts `column_names.len() == 8`. After adding `source_ids`, the count becomes 9. The test will fail with an assertion error.

**Why it happens:** The test was written against the v1.0 schema with exactly 8 columns.

**How to avoid:** Update the test to assert `column_names.len() == 9` and add `"source_ids"` to the `expected_columns` array.

**Warning signs:** `cargo test` fails on `test_schema_created` after the DDL change.

### Pitfall 2: SQLite ALTER TABLE IF NOT EXISTS Version Requirement

**What goes wrong:** `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` requires SQLite 3.37.0+. Older environments may silently fail or error.

**Why it happens:** `IF NOT EXISTS` clause on `ALTER TABLE` was added in SQLite 3.37.0 (2021-11-27).

**How to avoid:** The project uses `rusqlite = { version = "0.37", features = ["bundled"] }`. The bundled SQLite is current enough (3.47+). No action needed, but confirm via `SELECT sqlite_version()` in tests if there is any doubt.

**Warning signs:** `unknown syntax near IF` error on `ALTER TABLE` execution.

### Pitfall 3: validate_config() Test Coverage Gap

**What goes wrong:** Adding LLM validation without adding unit tests leaves the new code path untested.

**Why it happens:** Forgetting to mirror the existing `test_validate_config_*` suite pattern.

**How to avoid:** Add four tests in `src/config.rs` #[cfg(test)]:
- `test_validate_config_llm_openai_no_key` — expects error
- `test_validate_config_llm_openai_with_key` — expects Ok
- `test_validate_config_llm_unknown_provider` — expects error
- `test_validate_config_no_llm_ok` — expects Ok (llm_provider = None)

### Pitfall 4: test_config() Struct Literal Becomes Stale

**What goes wrong:** `tests/integration.rs` `test_config()` constructs `Config { port: 0, db_path: ..., embedding_provider: ..., openai_api_key: None }`. After adding four new fields, this struct literal fails to compile unless the new fields are added.

**Why it happens:** Rust struct literal syntax requires all fields when `..Default::default()` is not used.

**How to avoid:** Update `test_config()` to use `..Config::default()` spread syntax or explicitly add the four new `None` fields.

### Pitfall 5: MnemonicError From Impl Ordering

**What goes wrong:** Adding `#[from] LlmError` to `MnemonicError` and also implementing `From<LlmError> for ApiError` without routing through `MnemonicError` creates inconsistency with how `EmbeddingError` → `ApiError` is handled.

**Why it happens:** The existing `From<EmbeddingError> for ApiError` routes through `MnemonicError::Embedding`. New LLM errors should follow the same path: `LlmError` → `MnemonicError::Llm` → `ApiError::Internal`.

**How to avoid:** Do not add a direct `From<LlmError> for ApiError`. Let the existing `From<MnemonicError> for ApiError::Internal` handle it.

## Code Examples

Verified patterns from existing source code:

### Figment env var mapping (confirmed in src/config.rs)
```rust
// MNEMONIC_LLM_PROVIDER env var maps to llm_provider field automatically
Figment::from(Serialized::defaults(Config::default()))
    .merge(Toml::file(&toml_path))
    .merge(Env::prefixed("MNEMONIC_"))
    .extract::<Config>()
```
The `Env::prefixed("MNEMONIC_")` layer strips the prefix and lowercases the remainder. `MNEMONIC_LLM_PROVIDER` becomes `llm_provider`. No additional configuration required.

### Existing validate_config pattern (src/config.rs lines 29-47)
```rust
pub fn validate_config(config: &Config) -> anyhow::Result<()> {
    match config.embedding_provider.as_str() {
        "local" => Ok(()),
        "openai" => {
            if config.openai_api_key.is_none() {
                anyhow::bail!(
                    "embedding_provider is \"openai\" but MNEMONIC_OPENAI_API_KEY is not set"
                );
            }
            Ok(())
        }
        other => {
            anyhow::bail!(
                "unknown embedding_provider {:?}: expected \"local\" or \"openai\"",
                other
            );
        }
    }
}
```
LLM validation is a second independent block appended after this match, not a nested arm.

### Existing execute_batch schema pattern (src/db.rs lines 34-65)
```rust
conn.call(|c| -> Result<(), rusqlite::Error> {
    c.execute_batch("... DDL statements ...")?;
    Ok(())
})
.await
.map_err(|e| crate::error::DbError::Schema(format!("{}", e)))?;
```
New DDL is appended to the string. No structural change to `db::open()` needed.

### Integration test schema assertion (tests/integration.rs lines 74-93)
```rust
let expected_columns = [
    "id", "content", "agent_id", "session_id", "tags",
    "embedding_model", "created_at", "updated_at",
];
// ...
assert_eq!(column_names.len(), 8, "memories table should have exactly 8 columns");
```
This assertion MUST be updated to 9 columns when `source_ids` is added.

### tracing::info pattern for startup (src/main.rs lines 23-29)
```rust
tracing::info!(
    version = env!("CARGO_PKG_VERSION"),
    port = config.port,
    db_path = %config.db_path,
    embedding_provider = %config.embedding_provider,
    "mnemonic starting"
);
```
LLM provider startup log should follow this pattern (Claude's discretion per CONTEXT.md).

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual schema versioning | IF NOT EXISTS guards | This project from v1.0 | No migration framework needed at this scale |
| Nested config structs | Flat Option<String> fields | This project from v1.0 | Simpler figment env mapping |

**Deprecated/outdated:**
- Nothing deprecated in this phase. All patterns come from the v1.0 codebase and are current.

## Open Questions

1. **tracing::info for LLM config at startup**
   - What we know: CONTEXT.md marks this as Claude's discretion. Embedding provider already logs provider name.
   - What's unclear: Whether to log `llm_provider = None` (noisy) or only log when a provider is configured.
   - Recommendation: Only log when `llm_provider.is_some()`, mirroring the embedding section. Skip logging when `None` to avoid cluttering startup output for users who haven't configured LLM.

2. **compact_runs `threshold` column NOT NULL constraint**
   - What we know: CONTEXT.md specifies `threshold REAL NOT NULL`. Phase 8 will always pass a threshold.
   - What's unclear: Whether Phase 9 dry-run compaction might ever omit threshold.
   - Recommendation: Keep NOT NULL as decided. Phase 9 must always supply a threshold when inserting rows.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (cargo test) |
| Config file | none — cargo test discovers tests automatically |
| Quick run command | `cargo test --test integration test_schema` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LLM-01 | Config loads with llm_provider/llm_api_key/llm_base_url/llm_model from env | unit | `cargo test -p mnemonic --lib config::tests` | ✅ src/config.rs (new tests needed in existing file) |
| LLM-01 | validate_config rejects llm_provider=openai with no api_key | unit | `cargo test -p mnemonic --lib config::tests::test_validate_config_llm_openai_no_key` | ❌ Wave 0 |
| LLM-01 | validate_config accepts llm_provider=openai with api_key | unit | `cargo test -p mnemonic --lib config::tests::test_validate_config_llm_openai_with_key` | ❌ Wave 0 |
| LLM-01 | validate_config rejects unknown llm_provider | unit | `cargo test -p mnemonic --lib config::tests::test_validate_config_llm_unknown_provider` | ❌ Wave 0 |
| LLM-01 | validate_config passes when llm_provider is None | unit | `cargo test -p mnemonic --lib config::tests::test_validate_config_no_llm_ok` | ❌ Wave 0 |
| LLM-01 | source_ids column exists in memories table | integration | `cargo test --test integration test_schema_created` | ✅ tests/integration.rs (UPDATE required) |
| LLM-01 | compact_runs table exists and is queryable | integration | `cargo test --test integration test_compact_runs_exists` | ❌ Wave 0 |
| LLM-01 | db::open() is idempotent (call twice, no error) | integration | `cargo test --test integration test_db_open_idempotent` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p mnemonic --lib config::tests`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/integration.rs` — update `test_schema_created` expected_columns array (add `"source_ids"`) and column count assertion (8 → 9)
- [ ] `tests/integration.rs` — add `test_compact_runs_exists` — verifies compact_runs table exists with correct columns
- [ ] `tests/integration.rs` — add `test_db_open_idempotent` — calls `db::open()` twice on same db path, expects no error
- [ ] `src/config.rs` #[cfg(test)] — add 4 LLM validation tests (listed in test map above)
- [ ] `tests/integration.rs` — update `test_config()` helper to include new llm_* fields

## Sources

### Primary (HIGH confidence)

- `src/config.rs` — Complete Config struct, validate_config(), load_config() implementation verified by direct read
- `src/error.rs` — Complete error hierarchy including MnemonicError, DbError, ConfigError, EmbeddingError verified by direct read
- `src/db.rs` — Complete db::open() with execute_batch DDL verified by direct read
- `src/main.rs` — validate_config call site (line 21), embedding init pattern (lines 38-69) verified by direct read
- `tests/integration.rs` — test_schema_created column count assertion (line 93) verified by direct read
- `Cargo.toml` — rusqlite 0.37 bundled, figment 0.10, thiserror 2, reqwest 0.13 all confirmed present

### Secondary (MEDIUM confidence)
- SQLite 3.37.0 release notes: `ALTER TABLE ADD COLUMN IF NOT EXISTS` supported since 3.37.0 (2021-11-27). Bundled rusqlite 0.37 ships a current SQLite, well above this floor.

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in Cargo.toml, versions confirmed
- Architecture: HIGH — all patterns are direct copies of existing code, verified by reading source
- Pitfalls: HIGH — column count assertion and struct literal issues are direct code analysis findings
- Schema DDL: HIGH — `IF NOT EXISTS` on ALTER TABLE is a SQLite 3.37+ feature; bundled rusqlite guarantees this

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable Rust ecosystem; rusqlite/figment/thiserror APIs do not change at patch level)
