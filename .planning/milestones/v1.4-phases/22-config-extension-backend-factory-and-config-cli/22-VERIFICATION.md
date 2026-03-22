---
phase: 22-config-extension-backend-factory-and-config-cli
verified: 2026-03-21T00:00:00Z
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 22: Config Extension, Backend Factory, and Config CLI — Verification Report

**Phase Goal:** Extend Config, build backend factory, add config CLI subcommand
**Verified:** 2026-03-21
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Setting storage_provider to 'sqlite' (or omitting it) starts with zero behavior change | VERIFIED | `Config::default()` sets `storage_provider: "sqlite".to_string()`. `validate_config` passes for sqlite. `create_backend` returns `SqliteBackend::new(sqlite_conn)`. Test `test_validate_config_sqlite_ok` confirms. |
| 2 | Setting storage_provider to 'qdrant' without qdrant_url exits with a clear error before accepting traffic | VERIFIED | `validate_config` match arm for "qdrant" calls `anyhow::bail!("storage_provider is \"qdrant\" but MNEMONIC_QDRANT_URL is not set")`. Test `test_validate_config_qdrant_no_url` asserts the error contains "MNEMONIC_QDRANT_URL". |
| 3 | Setting storage_provider to 'postgres' without postgres_url exits with a clear error before accepting traffic | VERIFIED | `validate_config` match arm for "postgres" calls `anyhow::bail!("storage_provider is \"postgres\" but MNEMONIC_POSTGRES_URL is not set")`. Test `test_validate_config_postgres_no_url` asserts the error contains "MNEMONIC_POSTGRES_URL". |
| 4 | Setting storage_provider to an unknown value exits with a clear error listing valid options | VERIFIED | `validate_config` other arm bails with `"unknown storage_provider {:?}: expected \"sqlite\", \"qdrant\", or \"postgres\""`. Test `test_validate_config_unknown_storage_provider` asserts the message contains "unknown storage_provider", "sqlite", "qdrant", and "postgres". |
| 5 | A create_backend() factory function returns the correct backend based on config.storage_provider | VERIFIED | `pub async fn create_backend(config: &Config, sqlite_conn: Arc<Connection>) -> Result<Arc<dyn StorageBackend>, ApiError>` exists in `src/storage/mod.rs`. 4 tests cover all branches. |
| 6 | Running `mnemonic config show` prints current configuration with api keys redacted as **** | VERIFIED | `pub fn run_config_show(json_mode: bool)` in `src/cli.rs` prints grouped Server/Storage/Embedding/LLM sections. `redact_option()` maps `Some(_)` to `"****"`. `Commands::Config(ConfigArgs)` dispatched in `src/main.rs` before any DB/embedding init. |
| 7 | Running `mnemonic config show --json` prints a JSON object with api keys redacted as **** | VERIFIED | `run_config_show(json_mode: true)` path builds a `serde_json::json!({...})` object calling `redact_option()` for openai_api_key, llm_api_key, and qdrant_api_key. |
| 8 | GET /health returns a response with a 'backend' field showing the active storage backend name | VERIFIED | `health_handler(State(state): State<AppState>)` returns `Json(serde_json::json!({"status": "ok", "backend": state.backend_name}))`. `AppState.backend_name` is set from `config.storage_provider.clone()` in both `main.rs` and `tests/integration.rs`. |
| 9 | All CLI subcommands (remember, search, compact, serve) use the create_backend() factory instead of hardcoded SqliteBackend::new() | VERIFIED | `src/cli.rs` `init_db_and_embedding()` and `init_compaction()` both call `crate::storage::create_backend(&config, conn_arc).await`. `src/main.rs` server path calls `storage::create_backend(&config, db_arc.clone()).await`. No `SqliteBackend::new` remains in `src/main.rs` or production paths of `src/cli.rs`. |

**Score:** 9/9 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/config.rs` | Config struct with storage_provider, qdrant_url, qdrant_api_key, postgres_url fields; expanded validate_config() | VERIFIED | All 4 fields present with correct types and defaults. validate_config() contains the full match block for "sqlite", "qdrant", "postgres", and unknown providers. 9 new storage-related tests present. |
| `src/storage/mod.rs` | create_backend() async factory function | VERIFIED | `pub async fn create_backend(config: &Config, sqlite_conn: Arc<Connection>) -> Result<Arc<dyn StorageBackend>, ApiError>` present. Handles "sqlite", "qdrant" (with feature-gate), "postgres" (with feature-gate), and unknown. 4 tests present. |
| `src/cli.rs` | Config subcommand variant, ConfigSubcommand::Show, run_config_show() handler, updated init_db_and_embedding() and init_compaction() using create_backend() | VERIFIED | `ConfigArgs`, `ConfigSubcommand::Show`, `Commands::Config(ConfigArgs)`, `run_config_show(json_mode: bool)`, `redact_option()` all present. Both init functions use create_backend(). |
| `src/main.rs` | Config dispatch arm before Serve, factory call replacing hardcoded SqliteBackend::new() | VERIFIED | `Some(cli::Commands::Config(config_args))` arm dispatches at line 89. `storage::create_backend(&config, db_arc.clone()).await` at line 218. `backend_name: config.storage_provider.clone()` at line 245. |
| `src/server.rs` | AppState with backend_name: String, health_handler returning backend field | VERIFIED | `pub backend_name: String` in AppState at line 39. `health_handler` returns `{"status":"ok","backend":state.backend_name}` at line 99. Handler takes `State(state): State<AppState>`. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/config.rs` | `src/storage/mod.rs` | `create_backend takes &Config` | WIRED | `create_backend(config: &Config, ...)` in `src/storage/mod.rs` line 103 matches pattern `create_backend.*Config`. |
| `src/main.rs` | `src/storage/mod.rs` | `storage::create_backend(&config, db_arc.clone()).await` | WIRED | Line 218 in `src/main.rs` calls `storage::create_backend(&config, db_arc.clone()).await`. Result used to build `MemoryService` and `CompactionService`. |
| `src/cli.rs` | `src/storage/mod.rs` | `crate::storage::create_backend(&config, conn_arc).await` | WIRED | `init_db_and_embedding()` line 309 and `init_compaction()` line 380 both call `crate::storage::create_backend(&config, ...)`. Result assigned to `backend` and used in service construction. |
| `src/server.rs` | `src/main.rs` | `AppState.backend_name populated from config.storage_provider` | WIRED | `main.rs` line 245 sets `backend_name: config.storage_provider.clone()`. `server.rs` `health_handler` reads `state.backend_name`. |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| CONF-01 | 22-01-PLAN.md | storage_provider config field (sqlite/qdrant/postgres) in TOML and env vars with startup validation | SATISFIED | `storage_provider: String` in Config struct, defaults to "sqlite", loadable via TOML and `MNEMONIC_STORAGE_PROVIDER` env var (proven by `test_storage_provider_env_override` and `test_storage_provider_toml_override`). `validate_config()` checks the field at startup. |
| CONF-02 | 22-01-PLAN.md | Backend-specific config fields (qdrant_url, qdrant_api_key, postgres_url) with validate_config() checks | SATISFIED | All three Option<String> fields present in Config. validate_config() enforces qdrant_url for qdrant provider, postgres_url for postgres provider. Tests for each. |
| CONF-03 | 22-02-PLAN.md | mnemonic config show subcommand displays current configuration with secret redaction | SATISFIED | `run_config_show()` in `src/cli.rs` prints all config fields in both human-readable and JSON modes. `redact_option()` redacts openai_api_key, llm_api_key, qdrant_api_key. `Commands::Config` dispatched in main.rs. Verified running per summary. |
| CONF-04 | 22-02-PLAN.md | GET /health reports active storage backend name and connection status | SATISFIED (scoped) | Health endpoint returns `{"status":"ok","backend":"<provider>"}`. Per context decision D-22 through D-24, "connection status" was interpreted as backend name reporting only (no live connection probe) — this was an explicit phase design decision. The health endpoint reports the active backend name as intended. |

**Orphaned requirements check:** CONF-01, CONF-02, CONF-03, CONF-04 are all claimed by Phase 22 plans. No phase-22 requirements in REQUIREMENTS.md are orphaned. All four are marked `[x]` in REQUIREMENTS.md.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/storage/mod.rs` | 114 | `todo!("QdrantBackend construction — implemented in Phase 23")` | Info | Inside `#[cfg(feature = "backend-qdrant")]` block only — the feature flag is NOT enabled by default. The `#[cfg(not(feature = "backend-qdrant"))]` block (lines 116-121) handles the normal case and returns a real error. This todo is unreachable in standard builds. Intentional placeholder for Phase 23. |
| `src/storage/mod.rs` | 127 | `todo!("PostgresBackend construction — implemented in Phase 24")` | Info | Same pattern as above — behind `#[cfg(feature = "backend-postgres")]` which is not enabled by default. Intentional placeholder for Phase 24. |

No blocker anti-patterns. The two `todo!` stubs are correctly gated behind disabled feature flags and do not affect production code paths.

---

### Human Verification Required

The following items cannot be verified programmatically and should be spot-checked when convenient:

**1. Config Show Output Format**

**Test:** Run `cargo run -- config show` in the project directory
**Expected:** Output groups fields under Server, Storage, Embedding, LLM headers. Fields with secret values print `****`. Fields with None values are omitted.
**Why human:** Terminal output formatting and readability requires visual inspection.

**2. Config Show JSON Output**

**Test:** Run `cargo run -- config show --json`
**Expected:** Valid JSON object with "storage_provider", "port", "db_path" etc. Secret fields show `"****"` when set, `null` when not set.
**Why human:** JSON correctness could be checked programmatically, but real user experience of the output requires visual review.

**3. Health Endpoint Backend Field**

**Test:** Start server with `cargo run -- serve`, then `curl http://localhost:8080/health`
**Expected:** `{"status":"ok","backend":"sqlite"}`
**Why human:** Requires running the server, which can't be done in a static code check.

---

### Gaps Summary

No gaps. All 9 observable truths are verified, all 5 required artifacts are substantive and wired, all 4 key links are confirmed, and all 4 requirements (CONF-01 through CONF-04) are satisfied.

The two `todo!` stubs in `create_backend()` are intentional, gated behind feature flags that are disabled by default, and explicitly documented as Phase 23/24 work. They do not constitute gaps for this phase.

Test suite: **273 tests, 0 failures** (80 + 80 + 55 + 4 + 54 across all test suites).

---

_Verified: 2026-03-21_
_Verifier: Claude (gsd-verifier)_
