# Phase 4: Distribution - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

A shippable binary artifact with comprehensive documentation that enables any developer to go from download to first stored memory in under 3 commands. Includes a complete API reference covering every endpoint, working usage examples in curl and Python, and a simple tool-use example for AI agents. No new features, no authentication, no web UI — this phase packages and documents what exists.

</domain>

<decisions>
## Implementation Decisions

### README structure
- Single README.md file with linked table of contents at the top
- Section order: intro/tagline, quickstart, concepts, configuration, API reference, usage examples, how it works, contributing
- Quickstart is the first thing after the intro — download, run, store a memory in 3 commands or fewer
- API reference is inline with curl request + JSON response examples for every endpoint (no external docs site)
- Dedicated Configuration section with a table listing all MNEMONIC_* env vars, their defaults, and descriptions
- Brief "Concepts" section before API reference explaining agent_id, session_id, and tags

### Binary distribution
- Two distribution methods: `cargo install mnemonic` for Rust users, prebuilt binaries via GitHub Releases for everyone else
- Prebuilt binaries for: Linux x86_64, macOS x86_64 (Intel), macOS aarch64 (Apple Silicon)
- GitHub Actions CI workflow that builds release binaries on tag push (cross-compile matrix)
- No Docker image in v1 — mention as future work (conflicts with single-binary philosophy)
- Quickstart shows both methods: cargo install and direct binary download

### Agent framework examples
- Python examples using `requests` library only — no framework dependency, shows REST API directly
- Simple `MnemonicClient` helper class wrapping requests for store, search, list, delete
- Multi-agent example showing agent_id namespacing (two agents sharing one instance)
- One AI tool-use example showing how to define mnemonic as a tool for an LLM agent (conceptual, framework-agnostic)

### Example depth
- Curl examples use realistic data: agent names like "research-bot", real-looking memory content
- One error response example per endpoint in the API reference
- Brief "How it works" paragraph mentioning SQLite + sqlite-vec + candle without deep architecture detail
- Response examples show full JSON bodies so users know the exact shape

### Claude's Discretion
- Exact GitHub Actions workflow configuration and matrix details
- README formatting choices (badges, shields, etc.)
- Curl example content specifics
- Python client class internal structure
- Whether to include a CHANGELOG.md

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project specifications
- `.planning/REQUIREMENTS.md` — Phase 4 covers DOCS-01, DOCS-02, DOCS-03; see exact quickstart, API reference, and example requirements
- `.planning/ROADMAP.md` §Phase 4 — Success criteria (3-command quickstart, every endpoint documented, curl + Python + agent framework examples)
- `.planning/PROJECT.md` — Core value proposition ("Redis of agents"), constraints (single-binary, zero-config), out of scope items

### API surface (source of truth for documentation)
- `src/service.rs` — All request/response types (CreateMemoryRequest, SearchParams, ListParams, Memory, SearchResponse, ListResponse), validation rules, default values
- `src/server.rs` — Route definitions (POST /memories, GET /memories/search, GET /memories, DELETE /memories/:id, GET /health), status codes
- `src/config.rs` — Config struct with all configurable fields, defaults, env var prefix (MNEMONIC_), TOML support
- `src/main.rs` — Startup sequence, embedding provider selection logic, log output

### Prior phase context
- `.planning/phases/01-foundation/01-CONTEXT.md` — Configuration behavior (MNEMONIC_ prefix, precedence, defaults), startup output decisions
- `.planning/phases/02-embedding/02-CONTEXT.md` — Model download/caching behavior (~/.cache/huggingface/), OpenAI fallback via OPENAI_API_KEY, dimension alignment (384)
- `.planning/phases/03-service-and-api/03-CONTEXT.md` — Full API design: response formats, status codes, error format, search/list/delete behavior, multi-agent namespacing

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/service.rs` — Complete type definitions for all request/response shapes; documentation should mirror these exactly
- `src/config.rs:9-14` — Config struct is the source of truth for all configurable options
- `src/main.rs:22-28` — Startup log format shows what info the server prints on launch
- `idea.md` — Original project description with feature bullets; good source for README intro copy

### Established Patterns
- All endpoints return JSON; errors are `{"error": "message"}`
- POST /memories returns 201; all others return 200; 400 for validation; 404 for not found; 500 for internal
- Default port 8080, default db path ./mnemonic.db, default embedding provider "local"
- Model downloads from HuggingFace Hub on first run, cached at ~/.cache/huggingface/

### Integration Points
- `Cargo.toml` — Package metadata (name, version, description) should be filled in for `cargo install` to work properly
- `.github/workflows/` — New directory for CI release workflow
- `README.md` — Existing stub file to be replaced with full documentation

</code_context>

<specifics>
## Specific Ideas

No specific requirements — auto mode selected recommended defaults across all areas. Prior phases consistently chose idiomatic, standard approaches. README should match that philosophy: practical, no-nonsense, copy-paste-ready.

</specifics>

<deferred>
## Deferred Ideas

- Docker image distribution — future milestone
- Interactive API playground / Swagger UI — out of scope (conflicts with single-binary)
- CHANGELOG.md — can be added when there are multiple releases
- Homebrew formula — future distribution method
- Windows prebuilt binary — add when there's demand

</deferred>

---

*Phase: 04-distribution*
*Context gathered: 2026-03-19*
