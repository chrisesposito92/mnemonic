# Phase 7: Summarization Engine - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

A tested, prompt-injection-resistant SummarizationEngine is available for CompactionService to use — real LLM calls with OpenAiSummarizer, deterministic tests with MockSummarizer. This phase does NOT implement CompactionService itself (Phase 8) or the HTTP endpoint (Phase 9).

</domain>

<decisions>
## Implementation Decisions

### Trait API design
- `SummarizationEngine` trait mirrors `EmbeddingEngine` pattern exactly: `#[async_trait]`, object-safe, `Send + Sync`
- Single method: `async fn summarize(&self, texts: &[String]) -> Result<String, LlmError>`
- Input is a slice of memory content strings (the cluster to consolidate)
- Output is a single consolidated summary string
- Trait lives in a new `src/summarization.rs` file (mirrors `src/embedding.rs` separation)
- Used as `Arc<dyn SummarizationEngine>` — same pattern as embedding engine

### OpenAiSummarizer implementation
- Struct holds `reqwest::Client`, `api_key: String`, `base_url: String`, `model: String`
- Constructor: `OpenAiSummarizer::new(api_key, base_url, model)` — all three passed from config
- Default model: `gpt-4o-mini` (from Phase 6 config decisions)
- Default base_url: `https://api.openai.com/v1` — overridable for Azure/local endpoints
- Uses OpenAI chat completions API (`/chat/completions`), not the legacy completions endpoint
- Request/response serde structs follow the same pattern as `OpenAiEmbedRequest`/`OpenAiEmbedResponse` in embedding.rs

### LLM request configuration
- Timeout: 30 seconds (matches `OpenAiEngine` for embeddings)
- Temperature: 0.3 — low for deterministic, factual summarization
- No max_tokens cap — let the model determine appropriate summary length
- Single system message + single user message structure

### Prompt structure and injection prevention (LLM-03)
- System prompt instructs the LLM to consolidate memories into a single summary
- Memory content wrapped in XML-style data delimiters: `<memory index="N">content</memory>`
- All memories placed inside a `<memories>` container tag in the user message
- The instruction template is separate from the data block — raw content never reaches the prompt template directly
- System message contains ONLY instructions; user message contains ONLY the delimited data block
- This satisfies LLM-03: structured delimiters prevent prompt injection from memory content

### Error handling and fallback (LLM-04)
- `OpenAiSummarizer` returns `Err(LlmError::*)` on failure — it does NOT fall back itself
- Fallback to Tier 1 is CompactionService's responsibility (Phase 8), not the engine's
- Timeout maps to `LlmError::Timeout`
- HTTP/API errors map to `LlmError::ApiCall`
- Unparseable responses map to `LlmError::ParseError`
- No panics — all error paths return Result

### MockSummarizer implementation
- Returns deterministic output: `"MOCK_SUMMARY: "` + texts joined by `" | "`
- Example: `summarize(&["fact A", "fact B"])` → `"MOCK_SUMMARY: fact A | fact B"`
- Zero network calls, zero external dependencies
- Lives in `src/summarization.rs` behind `#[cfg(test)]` — OR as a public struct if integration tests need it
- Mirrors `MockEmbeddingEngine` in `tests/integration.rs` approach

### Engine initialization in main.rs
- After embedding engine init (line ~69), add LLM engine init block
- If `llm_provider` is `Some("openai")`, construct `OpenAiSummarizer` with config values
- If `llm_provider` is `None`, the summarization engine is `None` — `Option<Arc<dyn SummarizationEngine>>`
- AppState will eventually hold `Option<Arc<dyn SummarizationEngine>>` but actual AppState wiring is Phase 8/9 scope
- Phase 7 focuses on the engine itself + unit tests; main.rs wiring is minimal

### Claude's Discretion
- Exact system prompt wording for the summarization instruction
- Whether MockSummarizer is `pub` or `#[cfg(test)]`-only (depends on integration test needs)
- Internal helper function organization within summarization.rs
- Whether to add tracing::debug for LLM request/response logging

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — LLM-02, LLM-03, LLM-04 requirements define summarization behavior, injection prevention, and fallback

### Architecture patterns to mirror
- `src/embedding.rs` — EmbeddingEngine trait pattern: `#[async_trait]`, object-safe, `Send + Sync`, `Arc<dyn ...>` usage, OpenAiEngine struct with reqwest
- `src/error.rs` — LlmError enum already defined with ApiCall, Timeout, ParseError variants
- `src/config.rs` — LLM config fields (llm_provider, llm_api_key, llm_base_url, llm_model) already exist
- `src/main.rs` lines 38-69 — Embedding engine init pattern to mirror for summarization engine

### Prior phase context
- `.planning/phases/06-foundation/06-CONTEXT.md` — Config field naming, validation rules, error type decisions

### Project decisions
- `.planning/PROJECT.md` §Key Decisions — reqwest 0.13 for HTTP, no async-openai
- `.planning/STATE.md` §Accumulated Context — SummarizationEngine mirrors EmbeddingEngine, LlmError conversion chain

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `EmbeddingEngine` trait + `OpenAiEngine`: exact structural pattern to copy for `SummarizationEngine` + `OpenAiSummarizer`
- `reqwest::Client` already in Cargo.toml with `json` feature — no new dependencies needed
- `LlmError` enum already defined in `error.rs` — ready to use as return type
- `MockEmbeddingEngine` in `tests/integration.rs`: pattern for deterministic mock
- `async_trait` crate already in Cargo.toml

### Established Patterns
- Trait + impl in same file (`embedding.rs` has trait + LocalEngine + OpenAiEngine)
- Object safety tests: `fn _assert_object_safe(_: &dyn Engine) {}` compile-time check
- Send+Sync assertion tests: `fn _assert_send::<T: Send>() {}`
- Serde structs for OpenAI API: `#[derive(serde::Serialize)]` request, `#[derive(serde::Deserialize)]` response
- `reqwest::Client::builder().timeout().build()` pattern
- `bearer_auth()` + `.json()` + `.error_for_status()` chain

### Integration Points
- `src/main.rs` line ~69: after embedding init — LLM engine init goes here
- `src/server.rs` AppState: will hold `Option<Arc<dyn SummarizationEngine>>` (Phase 8/9 wiring)
- `src/lib.rs`: needs `pub mod summarization;` added
- `src/error.rs`: LlmError already wired into MnemonicError — no changes needed
- `Cargo.toml`: no new dependencies — reqwest, async-trait, serde already present

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. Follow existing codebase patterns exactly. The EmbeddingEngine → OpenAiEngine pattern is the blueprint.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 07-summarization-engine*
*Context gathered: 2026-03-20*
