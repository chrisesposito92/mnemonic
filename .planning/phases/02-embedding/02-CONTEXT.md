# Phase 2: Embedding - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

An `EmbeddingEngine` trait with a working `LocalEngine` (candle BERT, all-MiniLM-L6-v2, attention-mask-weighted mean pooling, L2 normalization) that produces semantically valid 384-dimension vectors, loaded once at startup and shared across requests via Arc, with an optional `OpenAiEngine` fallback selectable via `OPENAI_API_KEY` environment variable. No memory CRUD, no REST endpoints for memories — those are Phase 3.

</domain>

<decisions>
## Implementation Decisions

### Model download & caching
- Model downloads from HuggingFace Hub on first startup (lazy download)
- Cached at `~/.cache/huggingface/` (standard HF cache location)
- Works offline if model was previously downloaded and cached
- If download fails at startup, server exits with a fatal error and clear message explaining what happened and how to fix (e.g., check network, or pre-download the model)

### OpenAI dimension alignment
- Vec table schema is `embedding float[384]` — all embeddings must be 384 dimensions regardless of provider
- OpenAI text-embedding-3-small supports the `dimensions` parameter (Matryoshka representation learning) — request `dimensions=384` to match the local model
- This keeps the vec table schema consistent and avoids needing separate tables or dimension detection per provider
- OpenAI model is `text-embedding-3-small` (already specified in requirements)

### Trait API shape
- Async trait: `async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>`
- Single text input per call — no batch method in v1 (batch can be added as a separate trait method later)
- Trait lives in `src/embedding.rs` (follows flat module pattern from Phase 1)
- LocalEngine wraps candle sync inference in `tokio::task::spawn_blocking`
- OpenAiEngine uses an async HTTP client (reqwest) to call the OpenAI API
- Both implementations return `Vec<f32>` with exactly 384 elements

### Input edge cases
- OpenAI inputs exceeding token limit: return an error (reject, don't silently truncate — prevents data loss, caller should chunk if needed)
- Empty text input: return an error (embedding empty text is undefined)
- Local model handles tokenization internally via candle tokenizer

### Startup & integration
- Model loads once at startup and is stored behind Arc for sharing across requests
- `AppState` gains an `embedding: Arc<dyn EmbeddingEngine>` field alongside existing `db` and `config`
- `main.rs` selects the engine based on `config.embedding_provider` ("local" or "openai") and whether `OPENAI_API_KEY` is set
- Config gains `openai_api_key: Option<String>` field (from `MNEMONIC_OPENAI_API_KEY` env var)
- Startup logs which embedding provider is active and model load time
- Progress during model download/load: tracing::info log lines (consistent with Phase 1 structured logging)

### Error types
- New `EmbeddingError` variant in error.rs with sub-variants for model load failures, inference failures, and API failures
- Follows existing thiserror pattern from Phase 1

### Claude's Discretion
- Exact candle tensor operations for mean pooling (attention mask multiplication, sum, divide)
- reqwest client configuration (timeouts, retry policy)
- Tokenizer configuration details
- Test fixtures and similarity threshold values for semantic validation tests
- Whether to add `Send + Sync` bounds explicitly or let the compiler infer them

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project specifications
- `.planning/REQUIREMENTS.md` — Phase 2 covers EMBD-01 through EMBD-05; see §Embedding for exact requirements
- `.planning/ROADMAP.md` §Phase 2 — Success criteria (semantic similarity validation, startup behavior, provider switching)
- `.planning/PROJECT.md` — Constraints: candle (not ort) for inference, all-MiniLM-L6-v2 model, single-binary distribution

### Prior phase context
- `.planning/phases/01-foundation/01-CONTEXT.md` — Schema decisions (vec_memories float[384]), module layout, config patterns, error handling conventions

### Technical references from STATE.md blockers
- candle BERT batch embedding API tensor shapes need verification before writing production embedding code
- OpenAI text-embedding-3-small input truncation strategy: decided to reject with error (no silent truncation)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `src/config.rs` — Config struct with `embedding_provider: String` field, figment-based loading with MNEMONIC_ prefix
- `src/error.rs` — MnemonicError enum with thiserror, ready to add EmbeddingError variant
- `src/server.rs` — AppState struct with Arc<Connection> and Arc<Config>, ready for Arc<dyn EmbeddingEngine>
- `src/db.rs` — vec_memories virtual table already created with `embedding float[384]`

### Established Patterns
- thiserror for domain errors, anyhow for main.rs propagation
- Flat module structure: one file per domain (db.rs, config.rs, server.rs, error.rs)
- Arc wrapping for shared state in AppState
- tracing::info for structured startup logging

### Integration Points
- `main.rs:35` — AppState construction: add embedding engine here
- `config.rs:9-13` — Config struct: add openai_api_key field
- `error.rs` — Add EmbeddingError enum
- `lib.rs` — Add `pub mod embedding` re-export

</code_context>

<specifics>
## Specific Ideas

No specific requirements — auto mode selected recommended defaults across all areas. User preferences from Phase 1 indicate a preference for idiomatic Rust conventions and standard ecosystem patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 02-embedding*
*Context gathered: 2026-03-19*
