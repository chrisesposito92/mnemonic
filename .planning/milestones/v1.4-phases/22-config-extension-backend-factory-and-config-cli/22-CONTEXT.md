# Phase 22: Config Extension, Backend Factory, and Config CLI - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Extend the Config struct with `storage_provider` and backend-specific credential fields, expand `validate_config()` to catch misconfiguration before startup, wire a backend factory in `main.rs` and CLI init functions so the correct `StorageBackend` is instantiated from config, add a `mnemonic config show` subcommand that displays current configuration with secret redaction, and update `GET /health` to report the active backend name. Only the SQLite backend is wired in this phase — Qdrant and Postgres backends are implemented in Phases 23-24.

</domain>

<decisions>
## Implementation Decisions

### Config struct extension
- **D-01:** Add `storage_provider: String` field to Config, defaulting to `"sqlite"` — zero behavior change for existing users
- **D-02:** Add backend credential fields as `Option<String>`: `qdrant_url`, `qdrant_api_key`, `postgres_url` — all None by default
- **D-03:** All new fields follow the existing `MNEMONIC_` env prefix pattern (e.g. `MNEMONIC_STORAGE_PROVIDER`, `MNEMONIC_QDRANT_URL`) and are settable via TOML or env vars through Figment

### validate_config() expansion
- **D-04:** Add `storage_provider` validation as a new match arm in `validate_config()`, following the same pattern as `embedding_provider` validation
- **D-05:** `"sqlite"` → no extra validation (db_path already validated by existing logic)
- **D-06:** `"qdrant"` → require `qdrant_url` is Some, bail with `"storage_provider is \"qdrant\" but MNEMONIC_QDRANT_URL is not set"` — same wording pattern as existing embedding errors
- **D-07:** `"postgres"` → require `postgres_url` is Some, bail with `"storage_provider is \"postgres\" but MNEMONIC_POSTGRES_URL is not set"`
- **D-08:** Unknown storage_provider → bail with `"unknown storage_provider \"X\": expected \"sqlite\", \"qdrant\", or \"postgres\""`
- **D-09:** Feature-gate awareness: validation passes for "qdrant"/"postgres" even when built without the feature flag — the error comes at backend construction time, not config validation. This keeps config portable across builds.

### Backend factory
- **D-10:** Add a `storage::create_backend()` async function in `src/storage/mod.rs` that takes `&Config` and `Arc<Connection>` and returns `Result<Arc<dyn StorageBackend>, ApiError>`
- **D-11:** For `"sqlite"`, return `SqliteBackend::new(conn_arc)` — same as current hardcoded path
- **D-12:** For `"qdrant"` without `backend-qdrant` feature: return a clear compile-time or runtime error: `"qdrant backend requires building with --features backend-qdrant"`
- **D-13:** For `"postgres"` without `backend-postgres` feature: same pattern as D-12
- **D-14:** Replace all hardcoded `SqliteBackend::new()` calls in `main.rs`, `cli::init_db_and_embedding()`, and `cli::init_compaction()` with calls to the factory function
- **D-15:** The factory function signature accepts `Arc<Connection>` for SQLite but future backends won't use it — use `Option<Arc<Connection>>` or have the factory only pass it when storage_provider is "sqlite". Simplest: accept the conn_arc and ignore it for non-sqlite backends.

### `mnemonic config show` subcommand
- **D-16:** Add `Config` variant to the `Commands` enum with a `ConfigSubcommand::Show` sub-subcommand — mirrors the existing `Keys(KeysArgs)` pattern
- **D-17:** Init tier: no DB, no embedding, no validation — just `load_config()` and display. This is the lightest possible init (even lighter than `init_db`)
- **D-18:** Human output format: labeled key-value pairs, one per line, grouped logically (server, storage, embedding, LLM)
- **D-19:** Secret redaction: any field ending in `_key` that is `Some(value)` displays as `****` — applies to `openai_api_key`, `llm_api_key`, `qdrant_api_key`
- **D-20:** `--json` mode: same redaction applied, output as a single JSON object with all fields
- **D-21:** Respects the global `--json` flag already on the `Cli` struct — no per-subcommand flag needed

### Health endpoint backend reporting
- **D-22:** Extend the health response from `{"status":"ok"}` to `{"status":"ok","backend":"sqlite"}` (or "qdrant"/"postgres")
- **D-23:** Store the backend name as a `String` field in `AppState` — simplest approach, avoids adding a method to the StorageBackend trait for this phase
- **D-24:** The backend name comes directly from `config.storage_provider` — no need to query the backend itself

### CLI init function updates
- **D-25:** `init_db()` (used by keys/recall) remains unchanged — these commands don't use StorageBackend
- **D-26:** `init_db_and_embedding()` and `init_compaction()` gain a `storage::create_backend(&config, conn_arc)` call replacing the hardcoded `SqliteBackend::new()`
- **D-27:** The `Config` subcommand is dispatched in `main.rs` before any init — it only calls `load_config()` and prints

### Claude's Discretion
- Exact grouping and label text for human-readable config show output
- Whether `config show` shows all fields or only non-default ones
- Error message wording for feature-gate compile errors
- Whether the factory function returns a `Result` or panics on unsupported backends
- Test structure for new config validation arms

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Configuration and validation
- `.planning/REQUIREMENTS.md` — CONF-01 through CONF-04 define the config, validation, CLI, and health requirements
- `.planning/ROADMAP.md` Phase 22 section — success criteria: config show with redaction, qdrant without URL fails at startup, health shows backend, --json output
- `src/config.rs` — Current Config struct, validate_config(), load_config() — the files being extended

### Backend factory wiring
- `src/storage/mod.rs` — StorageBackend trait definition and SqliteBackend re-export — factory function goes here
- `src/main.rs` lines 208-211 — Current hardcoded backend factory point: `SqliteBackend::new(db_arc.clone())`
- `src/cli.rs` lines 219-221 — init_db_and_embedding hardcoded backend factory
- `src/cli.rs` lines 290-291 — init_compaction hardcoded backend factory

### CLI subcommand pattern
- `src/cli.rs` lines 8-39 — Cli struct and Commands enum — pattern for adding Config subcommand
- `src/main.rs` lines 27-91 — CLI dispatch match — pattern for adding Config arm

### Health endpoint
- `src/server.rs` lines 35-39 — AppState struct — needs backend_name field
- `src/server.rs` lines 97-99 — health_handler — needs to include backend field

### Prior decisions
- `.planning/phases/21-storage-trait-and-sqlite-backend/21-CONTEXT.md` — D-06: future backends get own files behind feature gates
- `.planning/STATE.md` "Accumulated Context > Decisions" — feature gate strategy, KeyService exclusion

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `validate_config()` pattern in `src/config.rs`: match arm per provider with bail! on missing credentials — exact template for storage_provider validation
- `figment::Jail` test pattern in `src/config.rs`: used for all config tests — reuse for new storage_provider tests
- `Commands` enum in `src/cli.rs`: existing subcommand pattern — Config variant follows same shape
- `init_db()` in `src/cli.rs`: lightest init path — Config show is even lighter (no DB needed)

### Established Patterns
- **Provider validation**: `match config.X_provider.as_str() { ... }` with bail! for missing credentials — Phase 22 adds a third validation block
- **CLI dispatch**: match on `cli_args.command` in main.rs with early return per subcommand — Config gets its own arm
- **Global --json flag**: already on Cli struct, extracted before match — Config show uses same `json` variable
- **AppState fields**: all Arc-wrapped services — backend_name is just a String, no Arc needed

### Integration Points
- `Config` struct in `src/config.rs`: add 3 new fields (storage_provider, qdrant_url/key, postgres_url)
- `validate_config()` in `src/config.rs`: add storage_provider match block
- `Commands` enum in `src/cli.rs`: add Config variant
- `main.rs` dispatch match: add Config arm before Serve
- `AppState` in `src/server.rs`: add backend_name: String
- `health_handler` in `src/server.rs`: include backend field in response
- `storage/mod.rs`: add `create_backend()` factory function
- 3 backend creation sites: main.rs:208, cli.rs:219, cli.rs:290 — all switch to factory

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. The existing validate_config() pattern and CLI dispatch pattern are clear templates. The factory function in storage/mod.rs follows the same module-owns-its-construction pattern as EmbeddingEngine selection in main.rs.

</specifics>

<deferred>
## Deferred Ideas

- `mnemonic config set` for interactive config editing — adds file-write complexity, TOML serialization; users can edit mnemonic.toml directly
- `mnemonic config validate` as a standalone command — validate_config() at startup already covers this; not worth a separate subcommand
- Backend health checks (ping Qdrant/Postgres at startup) — belongs in Phases 23/24 when those backends exist
- Config file auto-generation (`mnemonic config init`) — nice-to-have but not in scope

</deferred>

---

*Phase: 22-config-extension-backend-factory-and-config-cli*
*Context gathered: 2026-03-21*
