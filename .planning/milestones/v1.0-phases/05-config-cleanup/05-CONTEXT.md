# Phase 5: Config & Embedding Provider Cleanup - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

Close integration gaps identified in v1.0-MILESTONE-AUDIT.md (INT-01 and INT-02): wire the `embedding_provider` config field into engine selection so it's no longer a dead knob, update `mnemonic.toml.example` with the `openai_api_key` field added in Phase 2, and remove all dead code producing compiler warnings. No new features, no API changes, no behavior changes for existing working configurations.

</domain>

<decisions>
## Implementation Decisions

### embedding_provider wiring (INT-02)
- Wire `embedding_provider` into engine selection logic in `main.rs` — it currently selects engine based solely on `config.openai_api_key.is_some()`, ignoring `embedding_provider`
- Selection logic should be:
  - `embedding_provider == "local"` → use LocalEngine (ignore API key even if present)
  - `embedding_provider == "openai"` + API key present → use OpenAiEngine
  - `embedding_provider == "openai"` + API key missing → **startup error** (fail fast with clear message)
  - Unknown `embedding_provider` value → **startup error** (invalid config)
- Default remains `"local"` — existing zero-config behavior unchanged
- This makes the config field meaningful and closes INT-02

### Example config update (INT-01)
- Add commented `openai_api_key` field to `mnemonic.toml.example`
- Place it after `embedding_provider` with a comment explaining it's required when `embedding_provider = "openai"`

### Dead code removal (compiler warnings)
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

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Gap definitions
- `.planning/v1.0-MILESTONE-AUDIT.md` — INT-01 (mnemonic.toml.example missing openai_api_key) and INT-02 (embedding_provider dead knob); full tech debt inventory

### Source files to modify
- `src/main.rs` — Engine selection logic (lines 37-63), currently checks only `openai_api_key.is_some()`
- `src/config.rs` — Config struct with `embedding_provider: String` field
- `src/error.rs` — MnemonicError::Server and ConfigError::Invalid to remove
- `src/server.rs` — AppState struct with unused fields (db, config, embedding)
- `src/service.rs` — SearchResult struct to remove
- `mnemonic.toml.example` — Missing openai_api_key field

### Prior phase context
- `.planning/phases/01-foundation/01-CONTEXT.md` — Config behavior decisions, embedding_provider field origin
- `.planning/phases/02-embedding/02-CONTEXT.md` — OpenAI engine integration, openai_api_key addition to Config

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/config.rs:35-43` — `load_config()` with figment pipeline; validation can be added after `extract()`
- `src/error.rs:37-44` — ConfigError enum; could add a validation variant or reuse `Invalid` before removing it

### Established Patterns
- thiserror for typed errors, anyhow for main.rs propagation
- Config loaded once in main.rs, passed to downstream constructors
- Startup validation happens inline in main.rs before server starts

### Integration Points
- `src/main.rs:37-63` — Engine selection block: primary change site
- `src/main.rs:22-28` — Startup tracing: `embedding_provider` already logged here
- `tests/integration.rs:36,111` — Test configs set `embedding_provider: "local"` — must continue to work
- `README.md` — Documents `MNEMONIC_EMBEDDING_PROVIDER`; verify accuracy after wiring

</code_context>

<specifics>
## Specific Ideas

No specific requirements — auto mode selected recommended defaults. This is a pure cleanup phase: wire the dead knob, update the example file, remove warnings. Minimal blast radius, maximum correctness.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 05-config-cleanup*
*Context gathered: 2026-03-19*
