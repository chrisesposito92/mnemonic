/// Behavioral tests for LlmError variants and MnemonicError::Llm conversion.
///
/// Verifies:
///   - LlmError::ApiCall displays with the expected #[error] string
///   - LlmError::Timeout displays with the expected #[error] string
///   - LlmError::ParseError displays with the expected #[error] string
///   - LlmError converts into MnemonicError::Llm via the #[from] attribute

use mnemonic::error::{LlmError, MnemonicError};

/// LlmError::ApiCall("msg") displays as "LLM API call failed: msg".
#[test]
fn test_llm_error_api_call_display() {
    let err = LlmError::ApiCall("connection refused".to_string());
    let display = format!("{}", err);
    assert!(
        display.contains("LLM API call failed"),
        "ApiCall display should contain 'LLM API call failed', got: {}",
        display
    );
    assert!(
        display.contains("connection refused"),
        "ApiCall display should contain the message, got: {}",
        display
    );
}

/// LlmError::Timeout displays as "LLM request timed out".
#[test]
fn test_llm_error_timeout_display() {
    let err = LlmError::Timeout;
    let display = format!("{}", err);
    assert!(
        display.contains("LLM request timed out"),
        "Timeout display should contain 'LLM request timed out', got: {}",
        display
    );
}

/// LlmError::ParseError("msg") displays as "LLM response could not be parsed: msg".
#[test]
fn test_llm_error_parse_display() {
    let err = LlmError::ParseError("unexpected JSON".to_string());
    let display = format!("{}", err);
    assert!(
        display.contains("LLM response could not be parsed"),
        "ParseError display should contain 'LLM response could not be parsed', got: {}",
        display
    );
    assert!(
        display.contains("unexpected JSON"),
        "ParseError display should contain the message, got: {}",
        display
    );
}

/// LlmError converts into MnemonicError::Llm via the #[from] attribute on MnemonicError::Llm.
#[test]
fn test_llm_error_into_mnemonic() {
    let llm_err = LlmError::ApiCall("x".to_string());
    let mnemonic_err: MnemonicError = llm_err.into();

    // Verify the outer MnemonicError display wraps correctly
    let display = format!("{}", mnemonic_err);
    assert!(
        display.contains("llm error"),
        "MnemonicError::Llm display should contain 'llm error', got: {}",
        display
    );
    assert!(
        display.contains("LLM API call failed"),
        "MnemonicError::Llm display should contain the inner error, got: {}",
        display
    );

    // Verify the variant is indeed Llm by pattern matching
    assert!(
        matches!(mnemonic_err, MnemonicError::Llm(_)),
        "From<LlmError> for MnemonicError should produce the Llm variant"
    );
}
