# Phase 1: Foundation - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

A compiling Rust binary that initializes a SQLite database with the correct schema on startup, applies WAL mode, loads the sqlite-vec extension, reads configuration from environment variables or a TOML file, and starts a placeholder axum server with a health endpoint. No embedding logic, no memory API endpoints — those are Phase 2 and Phase 3.

</domain>

<decisions>
## Implementation Decisions

### Schema design
- Full schema from day one — include all columns needed across all phases to avoid migrations
- Columns: id (UUID v7 TEXT PK), content (TEXT), agent_id (TEXT), session_id (TEXT), tags (JSON array in TEXT column), embedding_model (TEXT), created_at (DATETIME), updated_at (DATETIME nullable)
- Tags stored as JSON array in a single TEXT column, queryable via SQLite json_each()
- IDs are UUID v7 (time-ordered, globally unique, TEXT primary key)
- Include updated_at column now (nullable, defaults to null) for future PUT support
- Embedding virtual table via sqlite-vec created alongside the memories table

### Configuration behavior
- Environment variable prefix: MNEMONIC_ (e.g., MNEMONIC_PORT, MNEMONIC_DB_PATH, MNEMONIC_EMBEDDING_PROVIDER)
- TOML config discovery: look for `mnemonic.toml` in CWD only; override path with MNEMONIC_CONFIG_PATH env var
- Precedence: Environment variables > TOML file > Defaults
- Default database path: ./mnemonic.db (current working directory)
- Default port: 8080
- Default embedding provider: local

### Startup output
- Logging via tracing + tracing-subscriber (Rust ecosystem standard, future OpenTelemetry compatible)
- Default log level: info (configurable via RUST_LOG)
- Human-readable log format by default (pretty-printed with colors in terminal)
- Compact startup info logs: version, listen address, storage path (WAL mode), embedding provider
- No ASCII art banner — clean structured log lines

### Project layout
- Single crate (one Cargo.toml, one src/ directory)
- Flat domain modules: src/main.rs, src/config.rs, src/db.rs, src/server.rs, src/error.rs
- Add submodules only when a file gets too large
- Error handling: thiserror for typed errors (db, config), anyhow for main.rs top-level propagation
- Phase 1 includes a placeholder axum server with GET /health returning {"status":"ok"}

### Claude's Discretion
- Exact tracing-subscriber format configuration
- sqlite-vec virtual table naming and column conventions
- TOML config file structure and field naming
- Cargo.toml dependency version pinning strategy
- Test structure (integration tests, unit tests placement)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project specifications
- `.planning/REQUIREMENTS.md` — Full v1 requirements; Phase 1 covers STOR-01 through STOR-04 and CONF-01 through CONF-03
- `.planning/ROADMAP.md` §Phase 1 — Success criteria and dependency chain
- `.planning/PROJECT.md` — Constraints (candle, sqlite-vec, tokio-rusqlite, axum), key decisions table

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- No existing code — greenfield project. Phase 1 creates the entire project skeleton.

### Established Patterns
- No patterns yet — this phase establishes the foundational patterns for all subsequent phases.

### Integration Points
- main.rs will be the entry point that wires config → db → server
- db.rs will expose a connection/pool type that Phase 2 (embedding storage) and Phase 3 (memory CRUD) build on
- config.rs will provide a Config struct used by all subsequent phases
- server.rs will provide the axum Router that Phase 3 adds routes to

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. The user selected recommended options across all areas, indicating a preference for idiomatic Rust conventions and widely-adopted ecosystem tools.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 01-foundation*
*Context gathered: 2026-03-19*
