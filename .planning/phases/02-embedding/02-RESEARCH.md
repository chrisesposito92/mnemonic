# Phase 2: Embedding - Research

**Researched:** 2026-03-19
**Domain:** Rust ML inference (candle-transformers), HuggingFace Hub, async traits, OpenAI Embeddings API
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **Model download & caching:** Lazy download from HuggingFace Hub on first startup; cached at `~/.cache/huggingface/`; works offline if cached; fatal error with clear message if download fails.
- **OpenAI dimension alignment:** `embedding float[384]` schema; request `dimensions=384` from text-embedding-3-small (Matryoshka parameter).
- **Trait API shape:** `async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>`; single text input per call; trait in `src/embedding.rs`; `LocalEngine` wraps candle sync inference in `tokio::task::spawn_blocking`; `OpenAiEngine` uses reqwest; both return exactly 384 elements.
- **Input edge cases:** Reject empty text and OpenAI token-limit-exceeded with error (no silent truncation).
- **Startup & integration:** Model loaded once at startup behind Arc; `AppState` gains `embedding: Arc<dyn EmbeddingEngine>` field; `main.rs` selects engine from `config.embedding_provider`; Config gains `openai_api_key: Option<String>` from `MNEMONIC_OPENAI_API_KEY`; startup logs provider and model load time.
- **Error types:** New `EmbeddingError` enum with `thiserror`; sub-variants for model load, inference, and API failures; added to `error.rs` and wrapped as a `MnemonicError` variant.
- **Candle specifically over ort** — pure Rust, no ONNX Runtime, true single-binary distribution.
- **all-MiniLM-L6-v2** — the specific model to use; 384-dimension hidden size.
- **Attention-mask-weighted mean pooling and L2 normalization** (not CLS token).

### Claude's Discretion

- Exact candle tensor operations for mean pooling (attention mask multiplication, sum, divide)
- reqwest client configuration (timeouts, retry policy)
- Tokenizer configuration details
- Test fixtures and similarity threshold values for semantic validation tests
- Whether to add `Send + Sync` bounds explicitly or let the compiler infer them

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| EMBD-01 | Server bundles all-MiniLM-L6-v2 via candle for zero-config local embedding inference | candle-transformers 0.9.2 BertModel + hf-hub 0.5.0 sync API downloads to HF cache automatically |
| EMBD-02 | Embedding pipeline uses attention-mask-weighted mean pooling and L2 normalization (not CLS token) | Official candle bert/main.rs shows exact tensor ops; BertModel.forward() takes `Option<&Tensor>` attention_mask |
| EMBD-03 | Embedding model loads once at startup and is shared across requests via Arc | tokio::task::spawn_blocking wraps sync candle inference; Arc<dyn EmbeddingEngine + Send + Sync> pattern |
| EMBD-04 | User can set OPENAI_API_KEY env var to use OpenAI embeddings instead | reqwest 0.13 client; `POST /v1/embeddings` with `dimensions: 384` parameter; serde_json for request/response |
| EMBD-05 | Embedding provider abstracted behind trait with local (candle) and OpenAI implementations | async-trait 0.1.89 enables `dyn EmbeddingEngine` trait objects; confirmed pattern for Arc<dyn AsyncTrait> |
</phase_requirements>

---

## Summary

Phase 2 adds an `EmbeddingEngine` trait with two implementations: `LocalEngine` using candle-transformers 0.9.2 to run the all-MiniLM-L6-v2 BERT model, and `OpenAiEngine` using reqwest 0.13.2 to call OpenAI's embeddings API. The official candle BERT example (`candle-examples/examples/bert/main.rs`) provides the complete, verified tensor operations for attention-mask-weighted mean pooling and L2 normalization — no guesswork needed. Both implementations must return exactly 384 `f32` values.

The critical integration constraint is threading: candle model inference is synchronous and CPU-bound. The `LocalEngine::embed()` implementation must wrap all candle inference in `tokio::task::spawn_blocking` to avoid blocking the tokio runtime. The `async-trait` crate (0.1.89) is required to make the trait usable as a `dyn EmbeddingEngine` — native async-in-traits (Rust 1.75) does not support dynamic dispatch without it. Model initialization uses `hf-hub`'s sync API inside `spawn_blocking` as well, since file downloads are blocking I/O.

The OpenAI path is straightforward: `POST https://api.openai.com/v1/embeddings` with `{"model": "text-embedding-3-small", "input": "...", "dimensions": 384}` and Bearer token auth. The response embedding is at `data[0].embedding`. Token limit for text-embedding-3-small is 8192 tokens — exceeding it should return an error (already decided in CONTEXT.md).

**Primary recommendation:** Follow the candle official bert example exactly for model loading and tensor operations; use `async-trait` for the trait definition; use `hf-hub` sync API inside `spawn_blocking` for model download; wire `Arc<dyn EmbeddingEngine + Send + Sync>` into `AppState`.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| candle-core | 0.9.2 | Tensor operations, device management | HuggingFace's official Rust ML framework; pure Rust, no ONNX Runtime |
| candle-nn | 0.9.2 | VarBuilder for loading safetensors weights | Required companion to candle-core for model construction |
| candle-transformers | 0.9.2 | BertModel implementation | Provides validated BertModel::load() and forward() |
| hf-hub | 0.5.0 | Download model files from HuggingFace Hub | Official HF Rust client; handles caching to ~/.cache/huggingface/ |
| tokenizers | 0.22.2 | Tokenize text input for BERT | Official HF tokenizers; handles all-MiniLM-L6-v2 vocabulary |
| async-trait | 0.1.89 | Enable async fn in trait with dyn dispatch | Required for `Arc<dyn EmbeddingEngine>`; native async-in-traits lacks dyn support |
| reqwest | 0.13.2 | HTTP client for OpenAI Embeddings API | Standard Rust async HTTP client; already in ecosystem |
| serde_json | 1 | Serialize OpenAI request body, deserialize response | Already in Cargo.toml |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio | 1 (full) | spawn_blocking for sync candle inference | Already in Cargo.toml; spawn_blocking is essential |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| async-trait | trait-variant (nightly) or manual Pin<Box<dyn Future>> | async-trait is stable, mature, and handles dyn correctly; trait-variant requires Rust nightly for full dyn support |
| hf-hub sync API | hf-hub tokio async API | Sync API is simpler; tokio async API still returns file paths (not data), so both require blocking I/O; sync inside spawn_blocking is cleaner |
| reqwest | ureq (sync) | reqwest is async-native and matches tokio runtime; ureq would require spawn_blocking |

**Installation:**
```bash
cargo add candle-core candle-nn candle-transformers --version "0.9.2"
cargo add hf-hub --version "0.5.0"
cargo add tokenizers --version "0.22.2"
cargo add async-trait --version "0.1.89"
cargo add reqwest --version "0.13.2" --features "json"
```

**Version verification:** Confirmed against crates.io registry on 2026-03-19:
- candle-core, candle-nn, candle-transformers: 0.9.2
- hf-hub: 0.5.0
- tokenizers: 0.22.2
- async-trait: 0.1.89
- reqwest: 0.13.2

---

## Architecture Patterns

### Recommended Project Structure
```
src/
├── embedding.rs         # EmbeddingEngine trait + LocalEngine + OpenAiEngine
├── config.rs            # +openai_api_key: Option<String>
├── error.rs             # +EmbeddingError enum
├── server.rs            # AppState gains embedding: Arc<dyn EmbeddingEngine>
├── main.rs              # Engine selection logic at startup
├── db.rs                # Unchanged
└── lib.rs               # +pub mod embedding
```

### Pattern 1: EmbeddingEngine Trait with async-trait

**What:** An async trait using `#[async_trait]` macro to allow `Arc<dyn EmbeddingEngine + Send + Sync>` dynamic dispatch.
**When to use:** Any time the embedding provider is selected at runtime (local vs. OpenAI).

```rust
// Source: async-trait 0.1.89 docs + candle bert example pattern
use async_trait::async_trait;

#[async_trait]
pub trait EmbeddingEngine: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
}
```

The `#[async_trait]` attribute is required on both the trait definition and every `impl` block.

### Pattern 2: LocalEngine — Model Loading with hf-hub Sync API

**What:** Download model files on first call using `hf_hub::api::sync::Api`, then load with candle's VarBuilder.
**When to use:** All local model initialization; must happen in `spawn_blocking` since both hf-hub sync and VarBuilder are blocking.

```rust
// Source: https://github.com/huggingface/candle/blob/main/candle-examples/examples/bert/main.rs
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use candle_nn::VarBuilder;
use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::Tokenizer;

fn load_local_engine() -> Result<(BertModel, Tokenizer), EmbeddingError> {
    let api = Api::new().map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?;
    let repo = api.repo(Repo::with_revision(
        "sentence-transformers/all-MiniLM-L6-v2".to_string(),
        RepoType::Model,
        "refs/pr/21".to_string(), // revision used in official candle example
    ));
    let config_path = repo.get("config.json").map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?;
    let tokenizer_path = repo.get("tokenizer.json").map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?;
    let weights_path = repo.get("model.safetensors").map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?;

    let config_str = std::fs::read_to_string(config_path)?;
    let config: Config = serde_json::from_str(&config_str)?;
    let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?;

    let device = candle_core::Device::Cpu;
    let vb = unsafe {
        VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)?
    };
    let model = BertModel::load(vb, &config)?;

    Ok((model, tokenizer))
}
```

**Note on revision:** The official candle example uses `"refs/pr/21"` for all-MiniLM-L6-v2. This refers to a PR with safetensors weights instead of PyTorch .bin. Using `"main"` also works but may download a larger pytorch_model.bin. Prefer `"refs/pr/21"` or check if `model.safetensors` exists on `"main"` before falling back.

### Pattern 3: Attention-Mask-Weighted Mean Pooling (VERIFIED from official candle source)

**What:** Multiply token embeddings by attention mask, sum across token dimension, divide by valid token count, then L2-normalize.
**When to use:** Any time producing a single sentence vector from BERT token embeddings.

```rust
// Source: https://github.com/huggingface/candle/blob/main/candle-examples/examples/bert/main.rs
// BertModel::forward signature (verified from docs.rs candle-transformers 0.9.2):
//   pub fn forward(&self, input_ids: &Tensor, token_type_ids: &Tensor, attention_mask: Option<&Tensor>) -> Result<Tensor>
// Output shape: (batch_size, sequence_length, hidden_size) = (1, n_tokens, 384) for single input

pub fn mean_pool_and_normalize(
    embeddings: &Tensor,   // shape: (1, n_tokens, 384)
    attention_mask: &Tensor, // shape: (1, n_tokens) — u32 values 0/1
) -> candle_core::Result<Tensor> {
    use candle_transformers::models::bert::DTYPE; // f32
    let attention_mask_f = attention_mask.to_dtype(DTYPE)?.unsqueeze(2)?; // (1, n_tokens, 1)
    let sum_mask = attention_mask_f.sum(1)?;                               // (1, 1)
    let pooled = (embeddings.broadcast_mul(&attention_mask_f)?).sum(1)?;   // (1, 384)
    let pooled = pooled.broadcast_div(&sum_mask)?;                         // (1, 384)
    normalize_l2(&pooled)
}

pub fn normalize_l2(v: &Tensor) -> candle_core::Result<Tensor> {
    Ok(v.broadcast_div(&v.sqr()?.sum_keepdim(1)?.sqrt()?)?)
}
```

### Pattern 4: Single-Text Tokenization for LocalEngine::embed()

**What:** Tokenize a single string, build tensors, run forward, pool, and return Vec<f32>.
**When to use:** Each call to `LocalEngine::embed()`.

```rust
// Source: adapted from candle bert/main.rs single-prompt branch
fn run_inference(
    model: &BertModel,
    tokenizer: &Tokenizer,
    text: &str,
    device: &candle_core::Device,
) -> Result<Vec<f32>, EmbeddingError> {
    // Encode without padding for single string
    let tokenizer = tokenizer
        .encode(text, true)
        .map_err(|e| EmbeddingError::Inference(e.to_string()))?;

    let token_ids_vec = tokenizer.get_ids().to_vec();
    let attention_mask_vec = tokenizer.get_attention_mask().to_vec();

    let token_ids = Tensor::new(token_ids_vec.as_slice(), device)?
        .unsqueeze(0)?;                                           // (1, n_tokens)
    let token_type_ids = token_ids.zeros_like()?;                 // (1, n_tokens)
    let attention_mask = Tensor::new(attention_mask_vec.as_slice(), device)?
        .unsqueeze(0)?;                                           // (1, n_tokens)

    let embeddings = model.forward(&token_ids, &token_type_ids, Some(&attention_mask))?;
    // embeddings shape: (1, n_tokens, 384)

    let pooled = mean_pool_and_normalize(&embeddings, &attention_mask)?;
    // pooled shape: (1, 384)

    let result = pooled.squeeze(0)?.to_vec1::<f32>()?;
    Ok(result) // Vec<f32> with exactly 384 elements
}
```

### Pattern 5: Wrapping Sync Inference in spawn_blocking

**What:** `LocalEngine` holds the model behind `Arc<Mutex<...>>` or uses clone if the model implements `Clone`. Since candle models are not `Send` by default, use `Arc<Mutex<(BertModel, Tokenizer)>>` and lock in spawn_blocking.
**When to use:** Every `embed()` call on `LocalEngine`.

```rust
// LocalEngine struct
pub struct LocalEngine {
    // BertModel and Tokenizer are not Send, so they live in spawn_blocking's closure
    // Two options:
    // 1. Store in Arc<Mutex<...>> — simple, serializes inference
    // 2. Use a dedicated thread with a channel — higher throughput, more complex
    // Option 1 is correct for Phase 2 (single-text, no batch)
    inner: std::sync::Arc<std::sync::Mutex<LocalEngineInner>>,
}

struct LocalEngineInner {
    model: BertModel,
    tokenizer: tokenizers::Tokenizer,
    device: candle_core::Device,
}

#[async_trait]
impl EmbeddingEngine for LocalEngine {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if text.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }
        let inner = Arc::clone(&self.inner);
        let text = text.to_string();
        tokio::task::spawn_blocking(move || {
            let guard = inner.lock().map_err(|_| EmbeddingError::Inference("mutex poisoned".into()))?;
            run_inference(&guard.model, &guard.tokenizer, &text, &guard.device)
        })
        .await
        .map_err(|e| EmbeddingError::Inference(e.to_string()))?
    }
}
```

**Note on Send + Sync:** `BertModel` may or may not implement `Send`. Wrapping in `Mutex` + `Arc` gives `Arc<Mutex<LocalEngineInner>>: Send + Sync` regardless, because `Mutex<T>: Send + Sync` when `T: Send`. Since `LocalEngineInner` only contains candle tensors on CPU (which are heap-allocated `Vec`-like structures), this should satisfy `Send`.

### Pattern 6: OpenAiEngine with reqwest

**What:** POST to OpenAI Embeddings API with JSON body including `dimensions: 384`.
**When to use:** When `OPENAI_API_KEY` / `config.openai_api_key` is set.

```rust
// Source: OpenAI API docs (https://developers.openai.com/api/docs/guides/embeddings)
// reqwest 0.13 JSON feature required

#[derive(serde::Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a str,
    dimensions: u32,
}

#[derive(serde::Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedData>,
}

#[derive(serde::Deserialize)]
struct EmbedData {
    embedding: Vec<f32>,
}

pub struct OpenAiEngine {
    client: reqwest::Client,
    api_key: String,
}

#[async_trait]
impl EmbeddingEngine for OpenAiEngine {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if text.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }
        let req = EmbedRequest {
            model: "text-embedding-3-small",
            input: text,
            dimensions: 384,
        };
        let resp: EmbedResponse = self.client
            .post("https://api.openai.com/v1/embeddings")
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await
            .map_err(|e| EmbeddingError::ApiCall(e.to_string()))?
            .error_for_status()
            .map_err(|e| EmbeddingError::ApiCall(e.to_string()))?
            .json()
            .await
            .map_err(|e| EmbeddingError::ApiCall(e.to_string()))?;

        resp.data.into_iter().next()
            .map(|d| d.embedding)
            .ok_or_else(|| EmbeddingError::ApiCall("empty response data".into()))
    }
}
```

### Pattern 7: Engine Selection in main.rs

**What:** Select engine at startup based on config, log which provider is active.

```rust
// main.rs — after loading config, before serving
let embedding: Arc<dyn EmbeddingEngine + Send + Sync> =
    if let Some(api_key) = config.openai_api_key.clone() {
        tracing::info!(provider = "openai", model = "text-embedding-3-small", "embedding engine ready");
        Arc::new(OpenAiEngine::new(api_key))
    } else {
        let start = std::time::Instant::now();
        tracing::info!(provider = "local", model = "all-MiniLM-L6-v2", "loading embedding model...");
        let engine = LocalEngine::new()
            .map_err(|e| anyhow::anyhow!(e))?;
        tracing::info!(elapsed_ms = start.elapsed().as_millis(), "embedding model loaded");
        Arc::new(engine)
    };
```

### Anti-Patterns to Avoid

- **CLS token pooling:** Using `embeddings.get(0)?` (the [CLS] token at position 0) instead of masked mean pooling. The CLS token is not the correct pooling strategy for all-MiniLM-L6-v2 — it was fine-tuned for mean pooling. Will produce lower-quality embeddings.
- **Simple mean without mask:** Using `embeddings.sum(1)? / n_tokens as f64` (ignoring attention mask). Includes padding token embeddings in the average, degrading quality. The official example demonstrates this is wrong.
- **Reloading model per request:** Calling `BertModel::load()` inside `embed()`. Model load is ~500ms+. Store in `Arc<Mutex<...>>` at startup.
- **Blocking tokio thread with candle:** Calling model.forward() directly in an async fn without spawn_blocking. Blocks the tokio worker thread, hurting all concurrent requests.
- **Forgetting `#[async_trait]` on impl blocks:** The macro must appear on both the trait definition and every `impl EmbeddingEngine for ...` block. Missing it on the impl produces a confusing type error.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Tokenization | Custom BPE/WordPiece tokenizer | `tokenizers` crate | Vocabulary files, special token handling, attention masks all managed correctly |
| HF model download + caching | Custom HTTP download + cache management | `hf-hub` | HF cache dir conventions, partial download resume, etag-based staleness detection |
| BERT forward pass | Custom transformer layers | `candle-transformers::models::bert::BertModel` | Multi-head attention, LayerNorm, weight loading already implemented and validated |
| Async trait objects | Manual `Pin<Box<dyn Future>>` boxing | `async-trait` crate | 320M+ downloads; handles all edge cases of lifetime erasure and Send bounds |
| JSON HTTP client | Custom reqwest wrapper | reqwest `.json()` + `.bearer_auth()` methods | Already handles serialization, connection pooling, TLS |

**Key insight:** The candle BERT example is production-quality reference code. Treat `candle-examples/examples/bert/main.rs` as the spec for tensor operations — do not derive the pooling algorithm from first principles.

---

## Common Pitfalls

### Pitfall 1: all-MiniLM-L6-v2 Model Revision
**What goes wrong:** Downloading from `"main"` may serve `pytorch_model.bin` (PyTorch format) instead of `model.safetensors`. The candle example explicitly uses revision `"refs/pr/21"` to get the safetensors version.
**Why it happens:** The official HuggingFace repo for sentence-transformers/all-MiniLM-L6-v2 originally only had PyTorch weights; safetensors were added via a PR.
**How to avoid:** Use `Repo::with_revision(..., "refs/pr/21")` as the candle example does, OR check whether `model.safetensors` exists on `"main"` (it may by now). If `model.safetensors` is missing, `api.get("model.safetensors")` returns an error.
**Warning signs:** `EmbeddingError::ModelLoad` at startup mentioning "not found" or "safetensors".

### Pitfall 2: BertModel Not Send
**What goes wrong:** `candle_transformers::models::bert::BertModel` may not implement `Send`. Storing it directly in `Arc<dyn EmbeddingEngine>` or attempting to move it across thread boundaries in spawn_blocking will fail to compile.
**Why it happens:** Candle tensors contain raw pointers for GPU device data (even in CPU mode, the abstraction is the same). Pointer types are not `Send` by default.
**How to avoid:** Wrap in `Arc<Mutex<LocalEngineInner>>`. The `Mutex` provides `Send + Sync` for the wrapped value. All inference happens inside the `spawn_blocking` closure holding the lock.
**Warning signs:** Compile error: "BertModel cannot be sent between threads safely", or "future is not Send".

### Pitfall 3: Tokenizer Thread Safety
**What goes wrong:** `tokenizers::Tokenizer` is not `Sync` (cannot be shared across threads without synchronization). Putting it in `Arc<Tokenizer>` alone (without Mutex) and using it from multiple threads will not compile.
**Why it happens:** The tokenizer caches encoding results internally (mutable state), breaking `Sync`.
**How to avoid:** Include `Tokenizer` in the same `Mutex<LocalEngineInner>` as the model. Single-text-per-call means this Mutex is released quickly.
**Warning signs:** Compile error about `Tokenizer: !Sync`.

### Pitfall 4: Mutex Poisoning in spawn_blocking
**What goes wrong:** If a previous `spawn_blocking` call panics while holding the mutex, subsequent lock attempts return `Err(PoisonError)`. Calling `.unwrap()` on the lock result will then panic on every subsequent request.
**Why it happens:** Panics in spawn_blocking tasks poison any mutexes they held.
**How to avoid:** Map the PoisonError to `EmbeddingError::Inference("mutex poisoned")` rather than unwrapping. Consider using `.lock().unwrap_or_else(|e| e.into_inner())` if the data is still valid after a panic.
**Warning signs:** All embedding calls fail after a single inference panic.

### Pitfall 5: OpenAI 8192 Token Limit
**What goes wrong:** OpenAI returns HTTP 400 with `context_length_exceeded` if input exceeds 8192 tokens. This manifests as a reqwest `error_for_status()` error.
**Why it happens:** text-embedding-3-small has a fixed context window.
**How to avoid:** The decided behavior is to propagate this as `EmbeddingError::InputTooLong`. Catch HTTP 400 responses and check for the specific error code before returning a generic API error.
**Warning signs:** `EmbeddingError::ApiCall` with status 400 and body containing "context_length_exceeded".

### Pitfall 6: Dimension Mismatch at vec_memories Insert
**What goes wrong:** If the embedding engine returns a Vec<f32> that is not exactly 384 elements, sqlite-vec will reject the INSERT with a constraint error.
**Why it happens:** OpenAI without `dimensions: 384` returns 1536 elements; local model without correct pooling might return wrong shape.
**How to avoid:** Assert `result.len() == 384` in both engines before returning, and return `EmbeddingError::Inference("expected 384 dimensions, got N")` if wrong.
**Warning signs:** sqlite-vec INSERT failure in Phase 3.

---

## Code Examples

Verified patterns from official sources:

### Complete BertModel Forward + Pooling + Normalize
```rust
// Source: https://github.com/huggingface/candle/blob/main/candle-examples/examples/bert/main.rs
// Confirmed BertModel::forward signature from docs.rs candle-transformers 0.9.2:
//   pub fn forward(&self, input_ids: &Tensor, token_type_ids: &Tensor, attention_mask: Option<&Tensor>) -> Result<Tensor>

let embeddings = model.forward(&token_ids, &token_type_ids, Some(&attention_mask))?;
// embeddings: (batch, n_tokens, 384)

let attention_mask_f = attention_mask.to_dtype(DTYPE)?.unsqueeze(2)?; // (batch, n_tokens, 1)
let sum_mask = attention_mask_f.sum(1)?;                               // (batch, 1)
let pooled = (embeddings.broadcast_mul(&attention_mask_f)?).sum(1)?;   // (batch, 384)
let pooled = pooled.broadcast_div(&sum_mask)?;                         // (batch, 384)

// L2 normalization
let normalized = pooled.broadcast_div(&pooled.sqr()?.sum_keepdim(1)?.sqrt()?)?;
```

### Model Loading with hf-hub Sync
```rust
// Source: https://github.com/huggingface/candle/blob/main/candle-examples/examples/bert/main.rs
use hf_hub::{api::sync::Api, Repo, RepoType};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};

let api = Api::new()?;
let repo = api.repo(Repo::with_revision(
    "sentence-transformers/all-MiniLM-L6-v2".to_string(),
    RepoType::Model,
    "refs/pr/21".to_string(),
));
let config_path = repo.get("config.json")?;
let tokenizer_path = repo.get("tokenizer.json")?;
let weights_path = repo.get("model.safetensors")?;

let config: Config = serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(E::msg)?;
let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)? };
let model = BertModel::load(vb, &config)?;
```

### Async Trait with dyn Dispatch
```rust
// Source: async-trait 0.1.89 — https://docs.rs/async-trait
use async_trait::async_trait;

#[async_trait]
pub trait EmbeddingEngine: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
}

// Usage
let engine: Arc<dyn EmbeddingEngine> = Arc::new(LocalEngine::new()?);
let vec = engine.embed("hello world").await?;
```

### OpenAI Embeddings Request
```rust
// Source: https://developers.openai.com/api/docs/guides/embeddings
// Endpoint: POST https://api.openai.com/v1/embeddings
// text-embedding-3-small max input: 8192 tokens
// Response: data[0].embedding — Vec<f32> of length 384 when dimensions=384

#[derive(serde::Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,    // "text-embedding-3-small"
    input: &'a str,
    dimensions: u32,   // 384
}

let resp = client
    .post("https://api.openai.com/v1/embeddings")
    .bearer_auth(&api_key)
    .json(&EmbedRequest { model: "text-embedding-3-small", input: text, dimensions: 384 })
    .send()
    .await?
    .error_for_status()?
    .json::<EmbedResponse>()
    .await?;
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| BertModel::forward with no attention_mask | BertModel::forward takes `Option<&Tensor>` for attention_mask | Candle PR #3085 (merged 2024) | Correct masked mean pooling is now trivial; no workarounds needed |
| pytorch_model.bin only for all-MiniLM-L6-v2 | model.safetensors available via PR #21 | 2023-2024 | Candle can load without PyTorch; smaller file, mmap-compatible |
| Async fn in traits not stable for dyn | Rust 1.75 stabilized async fn in traits, but dyn is still broken without async-trait | Dec 2023 | async-trait crate remains necessary for dyn dispatch in 2025/2026 |

**Deprecated/outdated:**
- Simple `sum / n_tokens` pooling (ignores padding): Produces lower-quality embeddings; the official candle example demonstrates masked pooling is correct.
- `pytorch_model.bin` loading via `VarBuilder::from_pth`: Works but is larger; prefer `model.safetensors`.

---

## Open Questions

1. **BertModel Send-ness in candle 0.9.2**
   - What we know: Candle tensors on CPU use `Arc<CpuStorage>` internally which should be `Send`. The model struct fields are all owned tensors + Device.
   - What's unclear: Whether `BertModel: Send` is guaranteed in candle 0.9.2 or if the GPU abstractions break it even for CPU-only use.
   - Recommendation: Attempt `Arc<Mutex<LocalEngineInner>>` first. If `BertModel: !Send` prevents compilation even inside Mutex, use a dedicated `std::thread` with `std::sync::mpsc` channel (single background thread owns the model and processes inference requests).

2. **all-MiniLM-L6-v2 revision on "main" branch today**
   - What we know: The candle example uses `"refs/pr/21"`. The PR may have been merged into main.
   - What's unclear: Whether `"main"` now serves `model.safetensors` directly.
   - Recommendation: Try `"main"` first; fall back to `"refs/pr/21"` if `model.safetensors` is missing. Or just use `"refs/pr/21"` to match the validated candle example.

3. **reqwest 0.13 API changes from 0.12**
   - What we know: reqwest 0.13.2 is current. The `.json()`, `.bearer_auth()`, `.error_for_status()` methods exist in the API.
   - What's unclear: Whether any breaking changes from 0.12 affect the patterns above.
   - Recommendation: LOW risk; the methods used are stable API surface. Verified from docs.rs that `.bearer_auth()` is available on `RequestBuilder`.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `#[tokio::test]` |
| Config file | none — uses Cargo.toml `[dev-dependencies]` |
| Quick run command | `cargo test --lib embedding` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| EMBD-01 | LocalEngine::new() succeeds and downloads model to HF cache | integration (network) | `cargo test --test integration test_local_engine_init` | ❌ Wave 0 |
| EMBD-02 | Cosine similarity of "dog"/"puppy" > "dog"/"database" | unit (semantic) | `cargo test --lib embedding::tests::test_semantic_similarity` | ❌ Wave 0 |
| EMBD-02 | embed() returns Vec<f32> with exactly 384 elements | unit | `cargo test --lib embedding::tests::test_embed_returns_384` | ❌ Wave 0 |
| EMBD-02 | L2 norm of returned vector is ~1.0 | unit | `cargo test --lib embedding::tests::test_embed_normalized` | ❌ Wave 0 |
| EMBD-03 | Model is loaded once; LocalEngine can be called twice without reinitializing | unit | `cargo test --lib embedding::tests::test_embed_reuse` | ❌ Wave 0 |
| EMBD-04 | OpenAiEngine returns 384-dim vector when OPENAI_API_KEY set | integration (network, optional) | `cargo test --test integration test_openai_engine -- --ignored` | ❌ Wave 0 |
| EMBD-05 | Both LocalEngine and OpenAiEngine satisfy EmbeddingEngine trait | unit (compile) | `cargo test --lib embedding::tests::test_trait_object_dispatch` | ❌ Wave 0 |

**Note on EMBD-01:** The first-run download test is slow (~100MB download). Mark with `#[ignore]` or gate with an env var; run in CI only. For fast local tests, check that the HF cache directory contains the model files after a single initialization.

**Semantic similarity threshold (EMBD-02):** Cosine similarity of related pair ("dog"/"puppy") should be > 0.5; unrelated pair ("dog"/"database") should be < 0.3. These thresholds are conservative given all-MiniLM-L6-v2's documented quality for semantic similarity.

### Sampling Rate
- **Per task commit:** `cargo test --lib`
- **Per wave merge:** `cargo test`
- **Phase gate:** `cargo test` green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `src/embedding.rs` — `#[cfg(test)] mod tests` with unit tests for EMBD-02, EMBD-03, EMBD-05
- [ ] `tests/integration.rs` — append `test_local_engine_init` (marked `#[ignore]`) and `test_openai_engine` (marked `#[ignore]`, requires env var)
- [ ] No new test framework needed — existing `#[tokio::test]` pattern from Phase 1 applies

---

## Sources

### Primary (HIGH confidence)
- `candle-examples/examples/bert/main.rs` (main branch, HuggingFace candle repo) — complete BertModel loading, tokenization, attention-mask pooling, L2 normalization
- `docs.rs/candle-transformers/0.9.2` — confirmed `BertModel::forward(input_ids, token_type_ids, attention_mask: Option<&Tensor>) -> Result<Tensor>` signature
- `developers.openai.com/api/docs/guides/embeddings` — confirmed `dimensions` parameter, request/response schema, 8192 token limit
- `crates.io` (verified 2026-03-19): candle-core/nn/transformers 0.9.2, hf-hub 0.5.0, tokenizers 0.22.2, async-trait 0.1.89, reqwest 0.13.2

### Secondary (MEDIUM confidence)
- `github.com/huggingface/candle/pull/3085` — PR confirming attention_mask was added to BertModel::forward (merged; behavior verified via docs.rs)
- `github.com/huggingface/hf-hub` — README confirming sync API caches to `~/.cache/huggingface/` and skips downloads for cached files
- `async-trait` docs.rs — confirmed `#[async_trait]` works with `dyn Trait` and `Arc<dyn Trait>`

### Tertiary (LOW confidence)
- GitHub issue #1552 (candle) — early discussion about attention_mask; superseded by PR #3085 and confirmed via docs.rs

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all versions verified against crates.io registry on research date
- Architecture: HIGH — tensor operations taken verbatim from official candle bert/main.rs
- Pitfalls: MEDIUM — Send/Sync behavior of BertModel at 0.9.2 is inferred from candle's architecture; must be confirmed at compile time
- OpenAI API: HIGH — verified from official documentation

**Research date:** 2026-03-19
**Valid until:** 2026-06-19 (stable libraries; candle releases may bump API)
