# Phase 07: Summarization Engine - Research

**Researched:** 2026-03-20
**Domain:** Rust async trait pattern, OpenAI chat completions API, prompt injection prevention
**Confidence:** HIGH

## Summary

Phase 7 implements a `SummarizationEngine` trait and its two concrete implementations: `OpenAiSummarizer` (live HTTP calls) and `MockSummarizer` (deterministic, zero-network). The entire implementation pattern is prescribed by the existing codebase — `EmbeddingEngine` / `OpenAiEngine` in `src/embedding.rs` is the direct blueprint. All required crates are already in `Cargo.toml`; no new dependencies are needed.

The critical new concern relative to `EmbeddingEngine` is **prompt injection prevention**. Memory content is user-controlled text that reaches the LLM; without structural separation, an adversarial memory could escape the data block and hijack the instruction. The solution locked in CONTEXT.md — XML-style `<memory index="N">` delimiters inside a `<memories>` container, with the system message carrying only instructions — is the standard technique and requires no additional libraries.

Error handling is simpler than embedding because the caller (Phase 8 CompactionService) owns the fallback decision. `OpenAiSummarizer` propagates typed `LlmError` variants and never panics; `MockSummarizer` is infallible, enabling zero-dependency unit tests.

**Primary recommendation:** Copy `src/embedding.rs` structure verbatim for `src/summarization.rs`, adapting field names and the HTTP call shape. Use the three `LlmError` variants already defined in `src/error.rs`. Do not add any new crates.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Trait API**
- `SummarizationEngine` trait: `#[async_trait]`, object-safe, `Send + Sync`
- Single method: `async fn summarize(&self, texts: &[String]) -> Result<String, LlmError>`
- Lives in `src/summarization.rs`; used as `Arc<dyn SummarizationEngine>`

**OpenAiSummarizer**
- Fields: `reqwest::Client`, `api_key: String`, `base_url: String`, `model: String`
- Constructor: `OpenAiSummarizer::new(api_key, base_url, model)`
- Default model: `gpt-4o-mini`; default base_url: `https://api.openai.com/v1`
- Endpoint: `/chat/completions` (not legacy `/completions`)
- Timeout: 30 seconds; temperature: 0.3; no `max_tokens` cap
- Single system message + single user message

**Prompt structure (LLM-03)**
- System message: instructions only
- User message: `<memories>` container with `<memory index="N">content</memory>` per item
- Raw memory content never appears in the prompt template string

**Error handling (LLM-04)**
- Returns `Err(LlmError::Timeout)` on timeout
- Returns `Err(LlmError::ApiCall)` on HTTP/API errors
- Returns `Err(LlmError::ParseError)` on unparseable response
- No panics, no self-fallback — fallback is CompactionService's job

**MockSummarizer**
- Output: `"MOCK_SUMMARY: "` + texts joined by `" | "`
- Zero network calls, zero external dependencies

**Module wiring**
- `src/lib.rs`: add `pub mod summarization;`
- `src/main.rs`: add LLM engine init block after embedding init (line ~69)
- `AppState` wiring deferred to Phase 8/9
- No new dependencies in `Cargo.toml`

### Claude's Discretion
- Exact system prompt wording for the summarization instruction
- Whether `MockSummarizer` is `pub` or `#[cfg(test)]`-only
- Internal helper function organization within `summarization.rs`
- Whether to add `tracing::debug` for LLM request/response logging

### Deferred Ideas (OUT OF SCOPE)
- None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| LLM-02 | When LLM is configured, compaction consolidates memory clusters into rich summaries via LLM | OpenAiSummarizer makes a `/chat/completions` POST; returns the `content` string from the first choice |
| LLM-03 | LLM prompts use structured delimiters to prevent prompt injection from memory content | XML-delimited `<memory index="N">` blocks in user message; system message is instruction-only |
| LLM-04 | If LLM call fails, system falls back to Tier 1 algorithmic merge instead of erroring | OpenAiSummarizer returns `Err(LlmError::*)` — CompactionService (Phase 8) owns the fallback |
</phase_requirements>

---

## Standard Stack

### Core (all already in Cargo.toml — verified)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `async-trait` | 0.1 | `#[async_trait]` macro for object-safe async traits | Required pattern, already used by EmbeddingEngine |
| `reqwest` | 0.13 | HTTP client for OpenAI chat completions | Already present with `json` feature; no async-openai (conflicts) |
| `serde` / `serde_json` | 1 | Serialize request / deserialize response structs | Already present; same usage as `OpenAiEmbedRequest` |
| `thiserror` | 2 | `LlmError` derive — already defined in `error.rs` | Already present; no change needed |
| `tokio` | 1 | Async runtime | Already present |
| `tracing` | 0.1 | Optional debug logging for LLM calls | Already present |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `std::time::Duration` | stdlib | 30-second timeout on reqwest client | Always — mirrors `OpenAiEngine::new` |
| `std::sync::Arc` | stdlib | `Arc<dyn SummarizationEngine>` usage site | main.rs init, future AppState |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `reqwest` direct | `async-openai` crate | async-openai conflicts with reqwest 0.13 — ruled out (STATE.md) |
| XML delimiters | Markdown fencing (` ```memories ``` `) | XML is less ambiguous for nested content; `<` / `>` chars are uncommon in memory text |
| `max_tokens` cap | None | Letting model size response naturally is correct for summaries of variable-length clusters |

**Installation:** No new packages needed. All dependencies present in `Cargo.toml`.

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── summarization.rs     # NEW — SummarizationEngine trait + OpenAiSummarizer + MockSummarizer
├── embedding.rs         # EXISTING — blueprint to follow
├── error.rs             # EXISTING — LlmError already defined here
├── config.rs            # EXISTING — llm_* fields already present
├── lib.rs               # ADD pub mod summarization;
└── main.rs              # ADD LLM engine init block after embedding init (~line 69)
```

### Pattern 1: Trait Declaration (mirrors EmbeddingEngine exactly)

**What:** Object-safe async trait with `Send + Sync` supertrait bounds, behind `#[async_trait]`
**When to use:** Any engine abstraction that must be stored as `Arc<dyn Trait>`

```rust
// Source: src/embedding.rs — mirror this pattern
use async_trait::async_trait;
use crate::error::LlmError;

#[async_trait]
pub trait SummarizationEngine: Send + Sync {
    async fn summarize(&self, texts: &[String]) -> Result<String, LlmError>;
}
```

### Pattern 2: OpenAiSummarizer Struct and Constructor

**What:** Holds a pre-built `reqwest::Client` (with timeout), API key, base URL, and model name
**When to use:** When `llm_provider == "openai"` in config

```rust
// Source: src/embedding.rs OpenAiEngine::new — mirror this constructor pattern
pub struct OpenAiSummarizer {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAiSummarizer {
    pub fn new(api_key: String, base_url: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to create HTTP client");
        Self { client, api_key, base_url, model }
    }
}
```

### Pattern 3: OpenAI Chat Completions Serde Structs

**What:** Minimal request/response structs for `/chat/completions`
**When to use:** In `summarize()` implementation — same pattern as `OpenAiEmbedRequest`

```rust
// Source: OpenAI API docs + embedding.rs pattern
#[derive(serde::Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    temperature: f32,
}

#[derive(serde::Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(serde::Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(serde::Deserialize)]
struct ChatChoice {
    message: ChatMessageOwned,
}

#[derive(serde::Deserialize)]
struct ChatMessageOwned {
    content: String,
}
```

### Pattern 4: Prompt Injection Prevention (LLM-03)

**What:** Structural separation of instructions (system message) from data (user message with XML delimiters)
**When to use:** Always — raw memory content MUST NOT appear in prompt template strings

```rust
// System message — instructions only, no user data
let system_msg = "You are a memory consolidation assistant. \
    Consolidate the provided memories into a single concise summary \
    that preserves all important facts. Output only the summary text.";

// User message — data block only, all content in XML delimiters
let mut data_block = String::from("<memories>\n");
for (i, text) in texts.iter().enumerate() {
    data_block.push_str(&format!("<memory index=\"{}\">{}</memory>\n", i, text));
}
data_block.push_str("</memories>");
```

### Pattern 5: Error Mapping in `summarize()`

**What:** Map reqwest errors to typed `LlmError` variants; never panic
**When to use:** Every fallible operation in the HTTP call chain

```rust
// Source: src/embedding.rs error mapping pattern — adapted for LlmError
let resp = self
    .client
    .post(format!("{}/chat/completions", self.base_url))
    .bearer_auth(&self.api_key)
    .json(&req)
    .send()
    .await
    .map_err(|e| {
        if e.is_timeout() {
            LlmError::Timeout
        } else {
            LlmError::ApiCall(e.to_string())
        }
    })?
    .error_for_status()
    .map_err(|e| LlmError::ApiCall(e.to_string()))?
    .json::<ChatResponse>()
    .await
    .map_err(|e| LlmError::ParseError(e.to_string()))?;

let summary = resp
    .choices
    .into_iter()
    .next()
    .map(|c| c.message.content)
    .ok_or_else(|| LlmError::ParseError("empty choices array".into()))?;
```

### Pattern 6: MockSummarizer

**What:** Deterministic impl that returns a predictable string without any I/O
**When to use:** Unit tests — eliminates network dependency entirely

```rust
// Mirrors MockEmbeddingEngine in tests/integration.rs
pub struct MockSummarizer;

#[async_trait]
impl SummarizationEngine for MockSummarizer {
    async fn summarize(&self, texts: &[String]) -> Result<String, LlmError> {
        Ok(format!("MOCK_SUMMARY: {}", texts.join(" | ")))
    }
}
```

### Pattern 7: Compile-Time Safety Assertions

**What:** Zero-cost tests that verify object safety and `Send + Sync` at compile time
**When to use:** At the bottom of `summarization.rs` in `#[cfg(test)]` — mirrors embedding.rs

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_object_compiles() {
        fn _assert_object_safe(_: &dyn SummarizationEngine) {}
    }

    #[test]
    fn test_openai_summarizer_send_sync() {
        fn _assert_send<T: Send>() {}
        fn _assert_sync<T: Sync>() {}
        _assert_send::<OpenAiSummarizer>();
        _assert_sync::<OpenAiSummarizer>();
    }

    #[test]
    fn test_mock_summarizer_send_sync() {
        fn _assert_send<T: Send>() {}
        fn _assert_sync<T: Sync>() {}
        _assert_send::<MockSummarizer>();
        _assert_sync::<MockSummarizer>();
    }
}
```

### Pattern 8: main.rs LLM Engine Init Block

**What:** Init block added after embedding engine init (line ~69)
**When to use:** When `llm_provider` is `Some("openai")`; wraps in `Option<Arc<...>>`

```rust
// After existing embedding init block (~line 69 in main.rs)
let llm_engine: Option<std::sync::Arc<dyn summarization::SummarizationEngine>> =
    match config.llm_provider.as_deref() {
        Some("openai") => {
            let api_key = config.llm_api_key.as_ref().unwrap(); // safe: validate_config passed
            let base_url = config.llm_base_url.clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            let model = config.llm_model.clone()
                .unwrap_or_else(|| "gpt-4o-mini".to_string());
            tracing::info!(
                provider = "openai",
                model = %model,
                "LLM summarization engine ready"
            );
            Some(std::sync::Arc::new(
                summarization::OpenAiSummarizer::new(api_key.clone(), base_url, model)
            ))
        }
        None => None,
        _ => unreachable!(), // validate_config rejects unknown providers
    };
```

### Anti-Patterns to Avoid

- **Timeout detection via string matching:** Use `reqwest::Error::is_timeout()` method, not `e.to_string().contains("timeout")`. The method is reliable; string matching is fragile across reqwest versions.
- **Raw content in prompt template:** Never do `format!("Summarize: {}", texts[0])`. Always route all memory content through the XML delimiter block.
- **`unwrap()` in async trait impl:** Any unwrap in `summarize()` that fires at runtime causes a panic visible to the caller. Use `?` with `map_err` throughout.
- **Single-string input flattening before passing to trait:** The trait accepts `&[String]` — let the caller pass the full cluster slice; don't pre-join outside the method.
- **`max_tokens` hardcoded to a small value:** Summaries of large clusters can legitimately be long. The decision is to let the model determine length.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP client with timeout | Custom `tokio::time::timeout` wrapper | `reqwest::ClientBuilder::timeout()` | reqwest's built-in timeout correctly maps to `is_timeout()` on the error |
| Retry logic | Manual exponential backoff loop | Nothing (Phase 7 scope) | Retry is CompactionService's concern — engine returns Err and lets caller decide |
| API key masking in logs | Custom redaction code | `tracing` field omission — don't log the key at all | Simplest and most secure; no risk of accidental exposure |
| JSON serialization of chat messages | `serde_json::Value` manually | Typed serde structs (`#[derive(Serialize, Deserialize)]`) | Compile-time shape guarantee; same pattern already used for embeddings |

**Key insight:** The entire HTTP call pattern is already proven in `OpenAiEngine::embed()`. Adapting that 20-line method for chat completions is lower risk than designing a new approach.

---

## Common Pitfalls

### Pitfall 1: Timeout error not distinguishable from other ApiCall errors

**What goes wrong:** Both network errors and timeouts map to `LlmError::ApiCall`, so Phase 8 cannot distinguish them for metrics/logging.
**Why it happens:** `reqwest::Error` has multiple categories; callers using `.map_err(|e| LlmError::ApiCall(e.to_string()))` uniformly lose the category.
**How to avoid:** Check `e.is_timeout()` before falling back to `ApiCall`:
```rust
.map_err(|e| if e.is_timeout() { LlmError::Timeout } else { LlmError::ApiCall(e.to_string()) })
```
**Warning signs:** `LlmError::Timeout` variant never appears in tests even when timeout is triggered.

### Pitfall 2: Empty texts slice produces a malformed prompt

**What goes wrong:** `texts.is_empty()` → empty `<memories></memories>` block → LLM returns unhelpful output or empty string.
**Why it happens:** No guard on the input slice.
**How to avoid:** Return `Err(LlmError::ApiCall("cannot summarize empty cluster".into()))` immediately if `texts.is_empty()`. MockSummarizer should do the same for consistency.
**Warning signs:** Test with empty slice returns `Ok("")` instead of an error.

### Pitfall 3: XML delimiter escaping for content containing `<` or `>`

**What goes wrong:** Memory text containing `<` (e.g., `"score < 5"`) breaks the XML structure in the user message.
**Why it happens:** Content is inserted verbatim without HTML entity encoding.
**How to avoid:** Either (a) note this is low-risk for real memory text and accept it for v1.1, OR (b) replace `<` with `&lt;` and `>` with `&gt;` when building the delimiter block. Decision is Claude's discretion.
**Warning signs:** LLM returns a confused response when memory content includes comparison operators.

### Pitfall 4: `choices` array empty in valid API response

**What goes wrong:** OpenAI occasionally returns a response with an empty `choices` array (filtered content, model error). Calling `choices[0]` panics.
**Why it happens:** Assuming a non-empty response.
**How to avoid:** Use `.into_iter().next().ok_or_else(|| LlmError::ParseError(...))` — already shown in Pattern 5.
**Warning signs:** Integration test with a mock server that returns `{"choices":[]}` panics instead of returning `Err`.

### Pitfall 5: MockSummarizer visibility causing compilation issues

**What goes wrong:** If `MockSummarizer` is `#[cfg(test)]`-only in `summarization.rs`, integration tests in `tests/integration.rs` cannot reference it (different compilation unit).
**Why it happens:** `#[cfg(test)]` in `src/` is invisible to `tests/`.
**How to avoid:** Either make `MockSummarizer` `pub` (always compiled) or add it as a non-`#[cfg(test)]` public struct with a clear doc comment. The `MockEmbeddingEngine` in `tests/integration.rs` is defined there directly as a local struct — that approach also works for mock summarizer tests.
**Warning signs:** `error[E0425]: cannot find struct 'MockSummarizer'` in integration test file.

---

## Code Examples

### Full `summarize()` implementation skeleton

```rust
// Source: embedding.rs OpenAiEngine::embed pattern, adapted for chat completions
#[async_trait]
impl SummarizationEngine for OpenAiSummarizer {
    async fn summarize(&self, texts: &[String]) -> Result<String, LlmError> {
        if texts.is_empty() {
            return Err(LlmError::ApiCall("cannot summarize empty cluster".into()));
        }

        let system_content = "You are a memory consolidation assistant. \
            Consolidate the provided memories into a single concise summary \
            that preserves all important facts. Output only the summary text.";

        let mut data_block = String::from("<memories>\n");
        for (i, text) in texts.iter().enumerate() {
            data_block.push_str(&format!("<memory index=\"{}\">{}</memory>\n", i, text));
        }
        data_block.push_str("</memories>");

        let req = ChatRequest {
            model: &self.model,
            messages: vec![
                ChatMessage { role: "system", content: system_content },
                ChatMessage { role: "user",   content: &data_block },
            ],
            temperature: 0.3,
        };

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() { LlmError::Timeout }
                else { LlmError::ApiCall(e.to_string()) }
            })?
            .error_for_status()
            .map_err(|e| LlmError::ApiCall(e.to_string()))?
            .json::<ChatResponse>()
            .await
            .map_err(|e| LlmError::ParseError(e.to_string()))?;

        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| LlmError::ParseError("empty choices array".into()))
    }
}
```

### Unit test: MockSummarizer output format

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_summarizer_output() {
        let engine = MockSummarizer;
        let texts = vec!["fact A".to_string(), "fact B".to_string()];
        let result = engine.summarize(&texts).await.unwrap();
        assert_eq!(result, "MOCK_SUMMARY: fact A | fact B");
    }

    #[tokio::test]
    async fn test_mock_summarizer_single_input() {
        let engine = MockSummarizer;
        let texts = vec!["only memory".to_string()];
        let result = engine.summarize(&texts).await.unwrap();
        assert_eq!(result, "MOCK_SUMMARY: only memory");
    }

    #[tokio::test]
    async fn test_mock_summarizer_empty_returns_err() {
        let engine = MockSummarizer;
        let result = engine.summarize(&[]).await;
        assert!(result.is_err());
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Legacy OpenAI `/v1/completions` | Chat completions `/v1/chat/completions` | Nov 2023 (GPT-4 Turbo) | Legacy endpoint does not support `gpt-4o-mini`; chat completions is required |
| `async-openai` crate | `reqwest` direct | Phase 6 decision | `async-openai` conflicts with reqwest 0.13 in this project |
| Freeform prompt interpolation | XML-delimited data blocks | Industry pattern 2023+ | Prevents prompt injection from user-controlled content |

**Deprecated/outdated:**
- Legacy `/v1/completions` endpoint: Does not support modern GPT-4 models. Use `/v1/chat/completions` exclusively.

---

## Open Questions

1. **XML escaping for `<` and `>` in memory content**
   - What we know: Memory content is user-supplied; can contain arbitrary characters
   - What's unclear: Whether the extra character replacement is worth the added complexity for v1.1
   - Recommendation: Skip escaping for v1.1; add a comment in code noting the known limitation. Real memory text rarely contains raw `<>`.

2. **Whether `MockSummarizer` should be `pub` or test-only**
   - What we know: `MockEmbeddingEngine` lives in `tests/integration.rs` as a local struct (not imported from `src/`)
   - What's unclear: Whether Phase 8 integration tests will need `MockSummarizer` from `src/`
   - Recommendation: Make `MockSummarizer` `pub` in `src/summarization.rs` with no feature flag. Cost is ~10 lines always compiled; benefit is it's available everywhere without re-declaration. This avoids Pitfall 5.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness + tokio-test (via `#[tokio::test]`) |
| Config file | none — standard `cargo test` |
| Quick run command | `cargo test -p mnemonic summarization` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LLM-02 | `OpenAiSummarizer::summarize` sends chat completion request and returns summary string | unit (ignored, requires API key) | `cargo test test_openai_summarizer -- --ignored` | Wave 0 |
| LLM-02 | `MockSummarizer::summarize` returns `"MOCK_SUMMARY: ..."` format | unit | `cargo test test_mock_summarizer` | Wave 0 |
| LLM-03 | Prompt data block uses `<memory index="N">` XML delimiters; system msg has no user data | unit | `cargo test test_prompt_structure` | Wave 0 |
| LLM-04 | Timeout returns `Err(LlmError::Timeout)` | unit | `cargo test test_summarize_timeout_error` | Wave 0 |
| LLM-04 | API error returns `Err(LlmError::ApiCall(...))` | unit | `cargo test test_summarize_api_error` | Wave 0 |
| LLM-04 | Parse error returns `Err(LlmError::ParseError(...))` | unit | `cargo test test_summarize_parse_error` | Wave 0 |
| Compile | `SummarizationEngine` is object-safe | compile-time | `cargo test test_trait_object_compiles` | Wave 0 |
| Compile | `OpenAiSummarizer` is `Send + Sync` | compile-time | `cargo test test_openai_summarizer_send_sync` | Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p mnemonic summarization`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `src/summarization.rs` — module does not exist yet; all tests live inside it
- [ ] `src/lib.rs` — needs `pub mod summarization;` line added

*(No separate test file needed — unit tests live in `src/summarization.rs` `#[cfg(test)]` block, following the pattern in `src/embedding.rs`)*

---

## Sources

### Primary (HIGH confidence)

- `src/embedding.rs` — Direct blueprint for trait, struct, serde structs, reqwest call chain, error mapping, and `#[cfg(test)]` assertions
- `src/error.rs` — `LlmError` variants (`ApiCall`, `Timeout`, `ParseError`) already defined and wired into `MnemonicError`
- `src/config.rs` — `llm_provider`, `llm_api_key`, `llm_base_url`, `llm_model` fields already present with validation logic
- `src/main.rs` — Embedding init block (lines 38–69) is the exact structural template for LLM init block
- `tests/integration.rs` — `MockEmbeddingEngine` pattern shows how to write a deterministic test double
- `Cargo.toml` — Verified: `async-trait 0.1`, `reqwest 0.13` with `json` feature, `serde 1`, `thiserror 2`, `tokio 1`, `tracing 0.1` — all present

### Secondary (MEDIUM confidence)

- OpenAI API docs (chat completions): `/v1/chat/completions` accepts `model`, `messages[]` with `role`/`content`, `temperature`; response has `choices[].message.content`
- reqwest 0.13 docs: `Error::is_timeout()` method available for timeout detection; `ClientBuilder::timeout()` sets a unified connect+read timeout

### Tertiary (LOW confidence)

- None — all claims verified against project source files or well-established API contracts.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified against Cargo.toml; no new dependencies
- Architecture: HIGH — patterns copied directly from existing src/embedding.rs in the same repo
- Pitfalls: HIGH — derived from direct code analysis (reqwest error types, Rust cfg(test) scoping rules, OpenAI API behavior)
- Prompt injection pattern: HIGH — XML delimiter approach is described in CONTEXT.md as locked decision

**Research date:** 2026-03-20
**Valid until:** 2026-06-20 (stable domain; reqwest/serde APIs rarely break within a minor version)
