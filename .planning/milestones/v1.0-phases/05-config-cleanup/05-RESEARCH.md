# Phase 5: Config & Embedding Provider Cleanup - Research

**Researched:** 2026-03-19
**Domain:** Rust dead code removal, config validation, startup error handling
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### embedding_provider wiring (INT-02)
- Wire `embedding_provider` into engine selection logic in `main.rs` — it currently selects engine based solely on `config.openai_api_key.is_some()`, ignoring `embedding_provider`
- Selection logic should be:
  - `embedding_provider == "local"` → use LocalEngine (ignore API key even if present)
  - `embedding_provider == "openai"` + API key present → use OpenAiEngine
  - `embedding_provider == "openai"` + API key missing → **startup error** (fail fast with clear message)
  - Unknown `embedding_provider` value → **startup error** (invalid config)
- Default remains `"local"` — existing zero-config behavior unchanged
- This makes the config field meaningful and closes INT-02

#### Example config update (INT-01)
- Add commented `openai_api_key` field to `mnemonic.toml.example`
- Place it after `embedding_provider` with a comment explaining it's required when `embedding_provider = "openai"`

#### Dead code removal (compiler warnings)
- Remove `MnemonicError::Server` variant — never constructed, no producers
- Remove `ConfigError::Invalid` variant — never constructed
- Remove `SearchResult` struct in `service.rs` — never constructed (only `SearchResultItem` and `SearchResponse` are used)
- Remove unused AppState fields (`db`, `config`, `embedding`) — all accessed exclusively via `service` field; alternatively prefix with `_` if removal requires broader refactoring
- Target: `cargo build 2>&1 | grep warning` produces zero warnings

### Claude's Discretion
- Exact error message wording for config mismatch startup errors
- Whether to use an enum for `embedding_provider` instead of String (if it simplifies validation)
- Whether removing AppState fields requires adjusting test helpers or integration tests
- Order of fields in updated `mnemonic.toml.example`

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CONF-02 | User can override settings via environment variables (port, storage path, embedding provider, OpenAI API key) | INT-02 fix makes `MNEMONIC_EMBEDDING_PROVIDER` a real, enforced override instead of a dead knob |
| CONF-03 | User can optionally provide a TOML configuration file for all settings | INT-01 fix adds `openai_api_key` to `mnemonic.toml.example` so TOML config is complete |
| EMBD-04 | User can optionally set `OPENAI_API_KEY` env var to use OpenAI embeddings instead of local model | INT-02 fix enforces the contract; startup error when `embedding_provider=openai` but no key is present |
</phase_requirements>

---

## Summary

Phase 5 is a pure cleanup phase with zero new features. All work falls into three buckets: (1) wiring the `embedding_provider` config field into engine selection in `main.rs`, (2) adding a commented `openai_api_key` field to `mnemonic.toml.example`, and (3) removing four dead code items that produce compiler warnings. The codebase is a single Rust binary; no external library research is needed. All information required to plan this phase comes from direct code inspection.

The four compiler warnings are confirmed by `cargo build 2>&1 | grep warning`: `MnemonicError::Server` (unused variant), `ConfigError::Invalid` (unused variant), `SearchResult` struct (never constructed), and `AppState` fields `db`/`config`/`embedding` (never read). Each has a clear, localized fix. The integration test file (`tests/integration.rs`) directly instantiates `AppState` with all four fields, so removing `AppState` fields requires updating the test helper `build_test_state()` in addition to the struct definition and `main.rs` construction site.

The `embedding_provider` wiring changes one block in `main.rs` (lines 37–70) and optionally moves validation into `load_config()` or a new `validate_config()` function. The locked decision mandates fail-fast on `embedding_provider=openai` + missing key and on unknown provider values. Because `ConfigError::Invalid` is being removed, the startup error for invalid config should propagate as an `anyhow::anyhow!()` in `main.rs` or as `ConfigError::Load` — the planner chooses whichever is cleaner.

**Primary recommendation:** Execute all three work streams in a single wave. Each change is localized to one or two files with no blast radius outside the files listed in CONTEXT.md canonical refs.

---

## Standard Stack

No new dependencies required. This phase uses only what is already in `Cargo.toml`.

### Core (already present)
| Library | Purpose | Relevant to This Phase |
|---------|---------|----------------------|
| thiserror | Typed error enums | Used when removing unused variants from `MnemonicError` and `ConfigError` |
| anyhow | Error propagation in `main.rs` | Used to surface startup config validation errors |
| figment | Config loading pipeline | `load_config()` returns `Result<Config, ConfigError>` — validation can be added post-extract |

### Installation
No new packages needed. No `cargo add` commands required.

---

## Architecture Patterns

### Recommended Project Structure (unchanged)
```
src/
├── main.rs        # Engine selection block — primary change site
├── config.rs      # Config struct and load_config() — optional validation addition
├── error.rs       # MnemonicError and ConfigError enum cleanup
├── server.rs      # AppState struct — field removal
├── service.rs     # SearchResult struct removal
└── embedding/     # Untouched
mnemonic.toml.example  # Add openai_api_key field
```

### Pattern 1: Fail-Fast Config Validation in main.rs

**What:** After `load_config()` succeeds, immediately validate business-rule constraints before any I/O. Return a clear anyhow error so the process exits with a non-zero code.

**When to use:** Config fields that are valid TOML/env types individually but invalid in combination. Embedding provider + API key is a perfect case — both fields parse fine, the constraint is relational.

**Example (current broken code):**
```rust
// src/main.rs lines 37-63 — CURRENT: ignores embedding_provider entirely
let embedding: Arc<dyn EmbeddingEngine> =
    if let Some(ref api_key) = config.openai_api_key {
        // uses OpenAI if key present, regardless of embedding_provider
        Arc::new(OpenAiEngine::new(api_key.clone()))
    } else {
        // uses Local if key absent, regardless of embedding_provider
        Arc::new(LocalEngine::new()...)
    };
```

**Example (target logic):**
```rust
// src/main.rs — AFTER: embedding_provider drives selection
let embedding: Arc<dyn EmbeddingEngine> = match config.embedding_provider.as_str() {
    "local" => {
        // load LocalEngine; ignore openai_api_key even if present
        let engine = tokio::task::spawn_blocking(|| embedding::LocalEngine::new())
            .await??;
        Arc::new(engine) as Arc<dyn EmbeddingEngine>
    }
    "openai" => {
        let api_key = config.openai_api_key.as_ref()
            .ok_or_else(|| anyhow::anyhow!(
                "embedding_provider is \"openai\" but MNEMONIC_OPENAI_API_KEY is not set"
            ))?;
        Arc::new(embedding::OpenAiEngine::new(api_key.clone()))
    }
    other => {
        return Err(anyhow::anyhow!(
            "unknown embedding_provider {:?}: expected \"local\" or \"openai\"",
            other
        ));
    }
};
```

**Companion fix — embedding_model string (lines 66–70):**
```rust
// CURRENT: also ignores embedding_provider
let embedding_model = if config.openai_api_key.is_some() {
    "text-embedding-3-small".to_string()
} else {
    "all-MiniLM-L6-v2".to_string()
};

// AFTER: driven by provider
let embedding_model = match config.embedding_provider.as_str() {
    "openai" => "text-embedding-3-small".to_string(),
    _        => "all-MiniLM-L6-v2".to_string(),
};
```

### Pattern 2: Removing Unused Error Variants

**What:** Delete enum variants that are never constructed. In thiserror enums, unused variants produce `variant X is never constructed` warnings. Removal is safe when no code matches on or creates them.

**Verification before removal:**
```bash
# Confirm no construction site exists
grep -rn "MnemonicError::Server\|ConfigError::Invalid" src/
# Expected: zero results
```

**Current error.rs state:**
- `MnemonicError::Server(String)` — line 14: no construction site exists anywhere in `src/`
- `ConfigError::Invalid(String)` — line 43: no construction site exists anywhere in `src/`

**After removal of `ConfigError::Invalid`:** The startup error for invalid `embedding_provider` must NOT use `ConfigError::Invalid` (it's being deleted). Use `anyhow::anyhow!()` directly in `main.rs` instead.

### Pattern 3: AppState Field Removal

**What:** Remove fields that are never accessed directly. `AppState.db`, `AppState.config`, and `AppState.embedding` are populated in `main.rs` but never accessed in any handler — all handler logic goes through `state.service`.

**Blast radius — two files:**

1. `src/server.rs` — `AppState` struct definition (lines 26–31): remove `db`, `config`, `embedding` fields
2. `tests/integration.rs` — `build_test_state()` (lines 386–404): this function constructs `AppState { db, config, embedding, service }` directly; removing those fields from the struct requires removing them from the test constructor too

**Current test constructor (lines 396–403):**
```rust
let state = AppState {
    db,
    config: Arc::new(config),
    embedding,
    service: service.clone(),
};
```

**After removal:**
```rust
let state = AppState {
    service: service.clone(),
};
```

**Note on `_` prefix alternative:** The CONTEXT.md locked decision says "alternatively prefix with `_` if removal requires broader refactoring." Given that the test helper is a two-line change, full removal is preferred over `_` prefixing. The planner should evaluate whether to use `_` or remove outright — both eliminate the warning.

### Pattern 4: SearchResult Struct Removal

**What:** `SearchResult` struct in `service.rs` (lines 70–74) is never constructed. The API uses `SearchResultItem` and `SearchResponse` exclusively. Removal is a pure delete of lines 70–74.

**Verification:**
```bash
grep -rn "SearchResult[^I]" src/ tests/
# Only definition at service.rs:71 should appear; no construction sites
```

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Config validation with typed errors | A new error type or custom validator | `anyhow::anyhow!()` in `main.rs` or reuse `ConfigError::Load` | The validation is startup-only, one-shot, not returned from library functions — anyhow is the right tool |
| Provider enum with custom serde | Hand-written `FromStr` + `Deserialize` impl | Keep `embedding_provider: String` with a `match .as_str()` | Enum approach is valid but adds code for minimal gain; match on string is idiomatic Rust for a 2-variant config value. Claude's discretion per CONTEXT.md. |

**Key insight:** This phase has no complex problems to solve — only straightforward deletions and a match arm rewrite. Resist the urge to refactor the config system into something more "correct."

---

## Common Pitfalls

### Pitfall 1: Forgetting the embedding_model string block
**What goes wrong:** Fix the engine selection `if let` block (lines 37–63) but forget the parallel `if config.openai_api_key.is_some()` block for `embedding_model` (lines 66–70). Both blocks use the same broken logic and must be updated together.
**Why it happens:** The two blocks are visually separated in `main.rs`; easy to see one and miss the other.
**How to avoid:** Read lines 37–70 as a single logical unit. Fix both blocks in the same edit.
**Warning signs:** `cargo build` passes but integration tests show `"all-MiniLM-L6-v2"` as `embedding_model` even when OpenAI is selected.

### Pitfall 2: Using ConfigError::Invalid for the new startup error
**What goes wrong:** Writing new startup error logic that constructs `ConfigError::Invalid(...)` and then removing that variant in the same PR — causing a compile error.
**Why it happens:** The variant exists at the start of the phase, making it tempting to use it.
**How to avoid:** The locked decision removes `ConfigError::Invalid`. Use `anyhow::anyhow!()` directly in `main.rs` for the new validation errors. Do not add a new `ConfigError` variant.

### Pitfall 3: AppState field removal breaks integration tests
**What goes wrong:** Remove `db`, `config`, `embedding` from `AppState` struct in `server.rs` but forget to update `build_test_state()` in `tests/integration.rs`. Results in compile error: "struct update syntax requires ..., found: unknown field `db`".
**Why it happens:** The struct definition and its only construction site outside `main.rs` are in different files (server.rs vs tests/integration.rs).
**How to avoid:** After editing `AppState` struct, grep for `AppState {` to find all construction sites before building.
**Warning signs:** `cargo test` fails with "unknown field" compile errors.

### Pitfall 4: README documents old behavior after wiring
**What goes wrong:** After wiring `embedding_provider`, the README line "setting this switches provider to OpenAI" (line 74) still implies that setting `MNEMONIC_OPENAI_API_KEY` alone switches the provider — which is no longer true. Users now need `MNEMONIC_EMBEDDING_PROVIDER=openai` AND the API key.
**Why it happens:** README was written under the old logic where API key presence was sufficient.
**How to avoid:** After wiring, verify README line 74 accurately reflects the new behavior. Update the configuration table entry for `MNEMONIC_OPENAI_API_KEY` to clarify it is only consulted when `MNEMONIC_EMBEDDING_PROVIDER=openai`.
**Warning signs:** README config table implies API key alone is sufficient to switch providers.

### Pitfall 5: Tracing log at startup references old logic
**What goes wrong:** `main.rs` line 22–28 logs `embedding_provider` from config. After the fix, this log is still correct — but the tracing info logs inside each match arm (currently inside the `if let`) must also be updated to reflect the new match structure.
**Why it happens:** The log messages inside the old `if let Some(api_key)` block use hardcoded `provider = "openai"` and `provider = "local"`. These need to move into the new match arms.
**How to avoid:** The existing logging pattern is fine — just ensure each match arm has the correct `tracing::info!` call with accurate field values.

---

## Code Examples

### Exact warning output (verified 2026-03-19)
```
warning: variant `Server` is never constructed
  --> src/error.rs:14:5
warning: variant `Invalid` is never constructed
  --> src/error.rs:43:5
warning: fields `db`, `config`, and `embedding` are never read
  --> src/server.rs:27:9
warning: struct `SearchResult` is never constructed
  --> src/service.rs:71:12
warning: `mnemonic` (bin "mnemonic") generated 4 warnings
```

### mnemonic.toml.example — target state
```toml
# Mnemonic Configuration
# Copy to mnemonic.toml and modify as needed.
# Environment variables override these values (prefix: MNEMONIC_).

# HTTP server port (env: MNEMONIC_PORT, default: 8080)
port = 8080

# Path to the SQLite database file (env: MNEMONIC_DB_PATH, default: "./mnemonic.db")
db_path = "./mnemonic.db"

# Embedding provider: "local" or "openai" (env: MNEMONIC_EMBEDDING_PROVIDER, default: "local")
embedding_provider = "local"

# OpenAI API key (env: MNEMONIC_OPENAI_API_KEY)
# Required when embedding_provider = "openai". Not used for local provider.
# openai_api_key = "sk-..."
```

### Grep commands to verify no construction sites before deleting
```bash
# Verify MnemonicError::Server has no producers
grep -rn "MnemonicError::Server" /Users/chrisesposito/Documents/github/mnemonic/src/

# Verify ConfigError::Invalid has no producers
grep -rn "ConfigError::Invalid" /Users/chrisesposito/Documents/github/mnemonic/src/

# Verify SearchResult (not SearchResultItem) has no construction sites
grep -rn "SearchResult {" /Users/chrisesposito/Documents/github/mnemonic/src/ /Users/chrisesposito/Documents/github/mnemonic/tests/

# Find all AppState construction sites
grep -rn "AppState {" /Users/chrisesposito/Documents/github/mnemonic/src/ /Users/chrisesposito/Documents/github/mnemonic/tests/
```

---

## State of the Art

No library upgrades are part of this phase. All patterns used here (match on string config values, anyhow for startup errors, thiserror for typed errors) are standard stable Rust idioms with no deprecation concerns.

| Old Approach | Current Approach (after phase) | Impact |
|---|---|---|
| `if config.openai_api_key.is_some()` drives engine selection | `match config.embedding_provider.as_str()` drives selection | `MNEMONIC_EMBEDDING_PROVIDER` becomes a real config knob |
| Silent fallback to local when provider=openai + no key | Startup error with clear message | No surprise behavior |
| `ConfigError::Invalid` declared but never used | Variant removed | Zero dead code warnings |
| `MnemonicError::Server` declared but never used | Variant removed | Zero dead code warnings |
| `SearchResult` alongside `SearchResultItem` | `SearchResult` removed | Zero dead code warnings |
| `AppState.db/.config/.embedding` populated but unused | Fields removed | Zero dead code warnings |

---

## Open Questions

1. **AppState: full removal vs `_` prefix**
   - What we know: Both options silence the warning. Full removal is cleaner; `_` prefix is safer if other code ever needs those fields.
   - What's unclear: Whether the planner wants to keep `_db`, `_config`, `_embedding` for future use or delete them entirely.
   - Recommendation: Full removal — the CONTEXT.md decision says "alternatively prefix with `_` if removal requires broader refactoring." The refactoring (one test helper update) is trivial, so prefer full removal.

2. **README line 74 update scope**
   - What we know: README line 74 currently says `MNEMONIC_OPENAI_API_KEY` — "setting this switches provider to OpenAI." After wiring, the API key alone is not sufficient; `MNEMONIC_EMBEDDING_PROVIDER=openai` must also be set.
   - What's unclear: Exact wording the user prefers.
   - Recommendation: Update the README config table to say something like "Required when `MNEMONIC_EMBEDDING_PROVIDER=openai`" and remove the implication that setting the key alone switches providers. This is a one-line change in the table.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + tokio-test (async) |
| Config file | none (Cargo.toml `[[test]]` is implicit) |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CONF-02 | `MNEMONIC_EMBEDDING_PROVIDER=openai` without API key produces startup error | unit | `cargo test --lib config` | ❌ Wave 0 — new test needed in `src/config.rs` or a new validation test |
| CONF-02 | `MNEMONIC_EMBEDDING_PROVIDER=unknown_value` produces startup error | unit | `cargo test --lib config` | ❌ Wave 0 |
| CONF-02 | `MNEMONIC_EMBEDDING_PROVIDER=local` works without API key (default behavior) | unit (existing) | `cargo test --lib` | ✅ `test_config_defaults` covers this |
| CONF-03 | `mnemonic.toml.example` contains `openai_api_key` field | manual verification | `grep openai_api_key mnemonic.toml.example` | ✅ after INT-01 fix |
| EMBD-04 | `embedding_provider=openai` + key present uses OpenAI engine (not local) | integration (ignored) | `cargo test -- --ignored test_openai_embedding` | ✅ existing ignored test |
| (all) | No compiler warnings after dead code removal | build check | `cargo build 2>&1 \| grep warning` | N/A — build gate |
| (all) | All existing tests continue to pass | regression | `cargo test` | ✅ full suite |

**Note on validation approach for startup error:** The startup error logic lives in `main.rs` which is not unit-testable in the traditional sense (it's the binary entry point). The validation can be moved to a `validate_config(config: &Config) -> anyhow::Result<()>` function in `config.rs` that IS unit-testable with `figment::Jail`. This is the recommended approach.

### Sampling Rate
- **Per task commit:** `cargo build 2>&1 | grep warning` (must be zero) + `cargo test --lib`
- **Per wave merge:** `cargo test` (full suite including integration)
- **Phase gate:** `cargo test` green + `cargo build 2>&1 | grep warning` is empty before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/config.rs` — add `validate_config()` function + unit tests for: (a) `embedding_provider=openai` + no key → error, (b) `embedding_provider=unknown` → error, (c) `embedding_provider=local` + no key → ok. Uses `figment::Jail` pattern already established in the file.

*(All other test infrastructure exists — integration tests in `tests/integration.rs`, `figment` test feature in `Cargo.toml`, `cargo test` suite passing)*

---

## Sources

### Primary (HIGH confidence)
- Direct code inspection of `src/main.rs`, `src/config.rs`, `src/error.rs`, `src/server.rs`, `src/service.rs` — confirmed exact line numbers and dead code locations
- `cargo build 2>&1 | grep warning` — confirmed exact 4 warnings as of 2026-03-19
- `.planning/v1.0-MILESTONE-AUDIT.md` — authoritative INT-01 and INT-02 gap definitions
- `.planning/phases/05-config-cleanup/05-CONTEXT.md` — locked implementation decisions

### Secondary (MEDIUM confidence)
- Rust compiler warning messages — `variant X is never constructed`, `fields X are never read`, `struct X is never constructed` — standard stable Rust behavior, no version sensitivity

### Tertiary (LOW confidence)
- None

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new libraries, all existing
- Architecture: HIGH — code inspected directly, line numbers confirmed
- Pitfalls: HIGH — confirmed by reading actual code, not speculation
- Validation: HIGH — existing test infrastructure confirmed by inspection

**Research date:** 2026-03-19
**Valid until:** Indefinite — this is a closed codebase, no external API drift possible
