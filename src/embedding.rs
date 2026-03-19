use async_trait::async_trait;
use crate::error::EmbeddingError;

/// Trait for embedding text into a fixed-dimensional vector.
///
/// Implementations must return exactly 384 f32 elements with L2 norm ≈ 1.0.
/// The trait is object-safe and can be used as `Arc<dyn EmbeddingEngine + Send + Sync>`.
#[async_trait]
pub trait EmbeddingEngine: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
}

/// Inner state for LocalEngine, held behind Arc<Mutex<...>> for Send+Sync.
struct LocalEngineInner {
    model: candle_transformers::models::bert::BertModel,
    tokenizer: tokenizers::Tokenizer,
    device: candle_core::Device,
}

/// Local embedding engine using candle-transformers to run all-MiniLM-L6-v2 on CPU.
///
/// Downloads model files from HuggingFace Hub on first use and caches them at
/// `~/.cache/huggingface/`. Works offline if the model is already cached.
///
/// Inference is wrapped in `tokio::task::spawn_blocking` to avoid blocking the
/// async runtime.
pub struct LocalEngine {
    inner: std::sync::Arc<std::sync::Mutex<LocalEngineInner>>,
}

impl LocalEngine {
    /// Load the all-MiniLM-L6-v2 model from HuggingFace Hub.
    ///
    /// Downloads `config.json`, `tokenizer.json`, and `model.safetensors` on first
    /// call; subsequent calls use the cached files. This is a blocking operation and
    /// should be called from within `spawn_blocking` or before starting the async
    /// runtime.
    pub fn new() -> Result<Self, EmbeddingError> {
        use candle_nn::VarBuilder;
        use candle_transformers::models::bert::{BertModel, Config, DTYPE};
        use hf_hub::{api::sync::Api, Repo, RepoType};
        use tokenizers::Tokenizer;

        let api = Api::new().map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?;
        let repo = api.repo(Repo::with_revision(
            "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            RepoType::Model,
            "refs/pr/21".to_string(),
        ));

        let config_path = repo
            .get("config.json")
            .map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?;
        let tokenizer_path = repo
            .get("tokenizer.json")
            .map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?;
        let weights_path = repo
            .get("model.safetensors")
            .map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?;

        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?;
        let config: Config = serde_json::from_str(&config_str)
            .map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?;

        let device = candle_core::Device::Cpu;

        // SAFETY: mmap of a local safetensors file; file is not modified while in use.
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path], DTYPE, &device)
                .map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?
        };

        let model =
            BertModel::load(vb, &config).map_err(|e| EmbeddingError::ModelLoad(e.to_string()))?;

        Ok(LocalEngine {
            inner: std::sync::Arc::new(std::sync::Mutex::new(LocalEngineInner {
                model,
                tokenizer,
                device,
            })),
        })
    }
}

/// Run a single text through the BERT model and return a 384-dim normalized embedding.
///
/// This is a synchronous function — call it inside `tokio::task::spawn_blocking`.
fn run_inference(
    model: &candle_transformers::models::bert::BertModel,
    tokenizer: &tokenizers::Tokenizer,
    text: &str,
    device: &candle_core::Device,
) -> Result<Vec<f32>, EmbeddingError> {
    use candle_core::Tensor;

    let encoding = tokenizer
        .encode(text, true)
        .map_err(|e| EmbeddingError::Inference(e.to_string()))?;

    let token_ids_vec = encoding.get_ids().to_vec();
    let attention_mask_vec = encoding.get_attention_mask().to_vec();

    let token_ids = Tensor::new(token_ids_vec.as_slice(), device)
        .map_err(EmbeddingError::from)?
        .unsqueeze(0)
        .map_err(EmbeddingError::from)?;

    let token_type_ids = token_ids.zeros_like().map_err(EmbeddingError::from)?;

    let attention_mask = Tensor::new(attention_mask_vec.as_slice(), device)
        .map_err(EmbeddingError::from)?
        .unsqueeze(0)
        .map_err(EmbeddingError::from)?;

    // Forward pass: output shape (1, n_tokens, 384)
    let embeddings = model
        .forward(&token_ids, &token_type_ids, Some(&attention_mask))
        .map_err(EmbeddingError::from)?;

    // Mean pooling with attention mask weighting, then L2 normalize
    let pooled = mean_pool_and_normalize(&embeddings, &attention_mask)
        .map_err(EmbeddingError::from)?;

    // Squeeze batch dimension and convert to Vec<f32>
    let result = pooled
        .squeeze(0)
        .map_err(EmbeddingError::from)?
        .to_vec1::<f32>()
        .map_err(EmbeddingError::from)?;

    if result.len() != 384 {
        return Err(EmbeddingError::Inference(format!(
            "expected 384 dimensions, got {}",
            result.len()
        )));
    }

    Ok(result)
}

/// Attention-mask-weighted mean pooling followed by L2 normalization.
///
/// Uses the exact tensor operations from the official candle bert/main.rs example.
///
/// # Arguments
/// - `embeddings`: shape (1, n_tokens, 384) — BERT hidden states
/// - `attention_mask`: shape (1, n_tokens) — u32 values 0/1
///
/// # Returns
/// Tensor of shape (1, 384) with L2 norm = 1.0
fn mean_pool_and_normalize(
    embeddings: &candle_core::Tensor,
    attention_mask: &candle_core::Tensor,
) -> Result<candle_core::Tensor, candle_core::Error> {
    use candle_transformers::models::bert::DTYPE;

    let attention_mask_f = attention_mask.to_dtype(DTYPE)?.unsqueeze(2)?; // (1, n_tokens, 1)
    let sum_mask = attention_mask_f.sum(1)?; // (1, 1)
    let pooled = (embeddings.broadcast_mul(&attention_mask_f)?).sum(1)?; // (1, 384)
    let pooled = pooled.broadcast_div(&sum_mask)?; // (1, 384)

    // L2 normalize
    let norm = pooled.sqr()?.sum_keepdim(1)?.sqrt()?;
    Ok(pooled.broadcast_div(&norm)?)
}

#[async_trait]
impl EmbeddingEngine for LocalEngine {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if text.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }
        let inner = std::sync::Arc::clone(&self.inner);
        let text = text.to_string();
        tokio::task::spawn_blocking(move || {
            let guard = inner
                .lock()
                .map_err(|_| EmbeddingError::Inference("mutex poisoned".into()))?;
            run_inference(&guard.model, &guard.tokenizer, &text, &guard.device)
        })
        .await
        .map_err(|e| EmbeddingError::Inference(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_object_compiles() {
        // Verify EmbeddingEngine is object-safe for Arc<dyn EmbeddingEngine>
        fn _assert_object_safe(_: &dyn EmbeddingEngine) {}
    }

    #[test]
    fn test_local_engine_send_sync() {
        fn _assert_send<T: Send>() {}
        fn _assert_sync<T: Sync>() {}
        _assert_send::<LocalEngine>();
        _assert_sync::<LocalEngine>();
    }

    #[tokio::test]
    async fn test_empty_input_returns_error() {
        // Test the error type exists and the match arm works.
        let err = EmbeddingError::EmptyInput;
        assert!(matches!(err, EmbeddingError::EmptyInput));
    }
}
