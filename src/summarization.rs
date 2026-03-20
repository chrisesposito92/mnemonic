use async_trait::async_trait;
use crate::error::LlmError;

// ──────────────────────────────────────────────────────────────────────────────
// Trait
// ──────────────────────────────────────────────────────────────────────────────

/// Trait for consolidating a cluster of memory texts into a single summary.
///
/// Implementations must be Send + Sync so they can be stored in Arc<dyn SummarizationEngine>.
#[async_trait]
pub trait SummarizationEngine: Send + Sync {
    async fn summarize(&self, texts: &[String]) -> Result<String, LlmError>;
}

// ──────────────────────────────────────────────────────────────────────────────
// Serde structs for OpenAI chat completions API (file-private)
// ──────────────────────────────────────────────────────────────────────────────

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

// ──────────────────────────────────────────────────────────────────────────────
// Helper: build the XML-delimited data block (prevents prompt injection)
// ──────────────────────────────────────────────────────────────────────────────

fn build_data_block(texts: &[String]) -> String {
    let mut block = String::from("<memories>\n");
    for (i, text) in texts.iter().enumerate() {
        block.push_str(&format!("<memory index=\"{}\">{}</memory>\n", i, text));
    }
    block.push_str("</memories>");
    block
}

// ──────────────────────────────────────────────────────────────────────────────
// OpenAiSummarizer
// ──────────────────────────────────────────────────────────────────────────────

const SYSTEM_MESSAGE: &str =
    "You are a memory consolidation assistant. Consolidate the provided memories into a single \
     concise summary that preserves all important facts. Output only the summary text.";

/// OpenAI-backed summarization engine using the chat completions API.
///
/// Memory content is always wrapped in XML delimiters (<memory index="N">...</memory>) to
/// prevent prompt injection — the system message contains only instructions, never user data.
///
/// All LLM errors are mapped to typed LlmError variants:
/// - Network timeout → LlmError::Timeout
/// - Non-2xx HTTP status → LlmError::ApiCall
/// - Unparseable response → LlmError::ParseError
pub struct OpenAiSummarizer {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAiSummarizer {
    /// Create a new OpenAiSummarizer.
    ///
    /// Configures a reqwest client with a 30-second timeout.
    pub fn new(api_key: String, base_url: String, model: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to create HTTP client");
        Self {
            client,
            api_key,
            base_url,
            model,
        }
    }
}

#[async_trait]
impl SummarizationEngine for OpenAiSummarizer {
    async fn summarize(&self, texts: &[String]) -> Result<String, LlmError> {
        if texts.is_empty() {
            return Err(LlmError::ApiCall("cannot summarize empty cluster".into()));
        }

        let data_block = build_data_block(texts);
        let system_content = SYSTEM_MESSAGE;
        let user_content = data_block.as_str();

        let req = ChatRequest {
            model: &self.model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: system_content,
                },
                ChatMessage {
                    role: "user",
                    content: user_content,
                },
            ],
            temperature: 0.3,
        };

        tracing::debug!(
            model = %self.model,
            texts_count = texts.len(),
            "sending summarization request"
        );

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

        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| LlmError::ParseError("empty choices array".into()))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// MockSummarizer (deterministic, no network)
// ──────────────────────────────────────────────────────────────────────────────

/// Deterministic summarizer for testing — returns "MOCK_SUMMARY: text1 | text2 | ..."
/// without making any network calls.
pub struct MockSummarizer;

#[async_trait]
impl SummarizationEngine for MockSummarizer {
    async fn summarize(&self, texts: &[String]) -> Result<String, LlmError> {
        if texts.is_empty() {
            return Err(LlmError::ApiCall("cannot summarize empty cluster".into()));
        }
        Ok(format!("MOCK_SUMMARY: {}", texts.join(" | ")))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Unit tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_object_compiles() {
        // Verify SummarizationEngine is object-safe for Arc<dyn SummarizationEngine>
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

    #[tokio::test]
    async fn test_mock_summarizer_output() {
        let mock = MockSummarizer;
        let result = mock
            .summarize(&["fact A".to_string(), "fact B".to_string()])
            .await
            .unwrap();
        assert_eq!(result, "MOCK_SUMMARY: fact A | fact B");
    }

    #[tokio::test]
    async fn test_mock_summarizer_single_input() {
        let mock = MockSummarizer;
        let result = mock
            .summarize(&["only memory".to_string()])
            .await
            .unwrap();
        assert_eq!(result, "MOCK_SUMMARY: only memory");
    }

    #[tokio::test]
    async fn test_mock_summarizer_empty_returns_err() {
        let mock = MockSummarizer;
        let result = mock.summarize(&[]).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::ApiCall(msg) => {
                assert!(msg.contains("empty cluster"), "unexpected message: {}", msg);
            }
            other => panic!("expected LlmError::ApiCall, got {:?}", other),
        }
    }

    #[test]
    fn test_prompt_structure() {
        let block = build_data_block(&["text A".to_string(), "text B".to_string()]);
        assert!(block.contains("<memories>"), "missing <memories> tag");
        assert!(
            block.contains("<memory index=\"0\">text A</memory>"),
            "missing index 0 memory"
        );
        assert!(
            block.contains("<memory index=\"1\">text B</memory>"),
            "missing index 1 memory"
        );
        assert!(block.contains("</memories>"), "missing </memories> closing tag");
    }
}
