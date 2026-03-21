//! CLI module — clap argument structs and handler functions for `mnemonic keys` subcommand.
//!
//! Phase 14 plan 01: builds CLI logic as a self-contained module.
//! Phase 14 plan 02 wires it into main.rs dispatch.

use clap::{Args, Parser, Subcommand};

/// Top-level CLI struct for the mnemonic binary.
#[derive(Parser)]
#[command(name = "mnemonic", version, about = "Agent memory server")]
pub struct Cli {
    /// Override database path (default: from config)
    #[arg(long, global = true, value_name = "PATH")]
    pub db: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Top-level subcommands.
#[derive(Subcommand)]
pub enum Commands {
    /// Manage API keys
    Keys(KeysArgs),
}

/// Arguments for the `keys` subcommand.
#[derive(Args)]
pub struct KeysArgs {
    #[command(subcommand)]
    pub subcommand: KeysSubcommand,
}

/// `keys` subcommands: create, list, revoke.
#[derive(Subcommand)]
pub enum KeysSubcommand {
    /// Create a new API key (shows raw key once)
    Create {
        /// Name for the key
        name: String,
        /// Scope key to a specific agent_id
        #[arg(long, value_name = "AGENT_ID")]
        agent_id: Option<String>,
    },
    /// List all API keys
    List,
    /// Revoke an API key by full UUID or 8-char display prefix
    Revoke {
        /// Full UUID or 8-char display_id
        id: String,
    },
}

/// Entry point called from main.rs — dispatches to the correct handler.
pub async fn run_keys(subcommand: KeysSubcommand, key_service: crate::auth::KeyService) {
    match subcommand {
        KeysSubcommand::Create { name, agent_id } => cmd_create(key_service, name, agent_id).await,
        KeysSubcommand::List => cmd_list(key_service).await,
        KeysSubcommand::Revoke { id } => cmd_revoke(key_service, id).await,
    }
}

/// Returns true if `input` is exactly 8 ASCII hex characters (the display_id format).
pub(crate) fn is_display_id(input: &str) -> bool {
    input.len() == 8 && input.chars().all(|c| c.is_ascii_hexdigit())
}

/// Handler for `mnemonic keys create <name> [--agent-id <AGENT_ID>]`.
///
/// On success:
///   - Prints raw token on its own line to stdout (line 1 — pipeable)
///   - Prints key metadata (ID, Name, Scope) to stdout
///   - Prints a "save this key" warning to stderr
///
/// On error: prints to stderr and exits with code 1.
async fn cmd_create(key_service: crate::auth::KeyService, name: String, agent_id: Option<String>) {
    match key_service.create(name, agent_id).await {
        Ok((api_key, raw_token)) => {
            // Raw token on its own line — easy to pipe
            println!("{}", raw_token);
            println!("ID:    {}", api_key.display_id);
            println!("Name:  {}", api_key.name);
            let scope = api_key
                .agent_id
                .as_deref()
                .unwrap_or("(unscoped)");
            println!("Scope: {}", scope);
            eprintln!();
            eprintln!("Save this key -- it will not be shown again.");
        }
        Err(e) => {
            eprintln!("error: failed to create key: {}", e);
            std::process::exit(1);
        }
    }
}

/// Handler for `mnemonic keys list`.
///
/// Prints a formatted table with columns: ID, NAME, SCOPE, CREATED, STATUS.
/// If no keys exist, prints an actionable empty-state message.
///
/// On error: prints to stderr and exits with code 1.
async fn cmd_list(key_service: crate::auth::KeyService) {
    match key_service.list().await {
        Ok(keys) => {
            if keys.is_empty() {
                println!("No API keys found. Create one with: mnemonic keys create <name>");
                return;
            }

            // Column format: ID(10), NAME(20), SCOPE(20), CREATED(19), STATUS
            let header = format!(
                "{:<10}  {:<20}  {:<20}  {:<19}  {}",
                "ID", "NAME", "SCOPE", "CREATED", "STATUS"
            );
            println!("{}", header);
            println!("{}", "-".repeat(header.len()));

            for key in &keys {
                let name = truncate(&key.name, 20);
                let scope = match &key.agent_id {
                    Some(aid) => truncate(aid, 20),
                    None => "(all)".to_string(),
                };
                let created = if key.created_at.len() >= 19 {
                    key.created_at[..19].to_string()
                } else {
                    key.created_at.clone()
                };
                let status = match &key.revoked_at {
                    None => "active".to_string(),
                    Some(ts) => {
                        let date = if ts.len() >= 10 { &ts[..10] } else { ts.as_str() };
                        format!("revoked ({})", date)
                    }
                };
                println!(
                    "{:<10}  {:<20}  {:<20}  {:<19}  {}",
                    key.display_id, name, scope, created, status
                );
            }
        }
        Err(e) => {
            eprintln!("error: failed to list keys: {}", e);
            std::process::exit(1);
        }
    }
}

/// Truncates a string to `max_len` characters, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Handler for `mnemonic keys revoke <id>`.
///
/// Accepts either:
///   - An 8-char hex display_id (looked up via find_by_display_id)
///   - A full UUID (passed directly to KeyService::revoke)
///
/// On success: prints confirmation to stdout.
/// On error: prints to stderr and exits with code 1.
async fn cmd_revoke(key_service: crate::auth::KeyService, id: String) {
    if is_display_id(&id) {
        // Look up by display_id
        match key_service.find_by_display_id(&id).await {
            Ok(found) => match found.len() {
                0 => {
                    eprintln!("No key found with ID {}", id);
                    std::process::exit(1);
                }
                1 => {
                    let full_id = found[0].id.clone();
                    let display = found[0].display_id.clone();
                    match key_service.revoke(&full_id).await {
                        Ok(()) => println!("Key {} revoked", display),
                        Err(e) => {
                            eprintln!("error: failed to revoke key: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                n => {
                    eprintln!("Ambiguous -- {} keys match prefix. Use full UUID:", n);
                    for key in &found {
                        eprintln!("  {} ({})", key.id, key.name);
                    }
                    std::process::exit(1);
                }
            },
            Err(e) => {
                eprintln!("error: DB lookup failed: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        // Treat as full UUID
        match key_service.revoke(&id).await {
            Ok(()) => println!("Key revoked"),
            Err(e) => {
                eprintln!("error: failed to revoke key: {}", e);
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- is_display_id ----

    #[test]
    fn test_is_display_id_valid() {
        assert!(is_display_id("abcd1234"));
        assert!(is_display_id("ABCD1234"));
        assert!(is_display_id("00000000"));
        assert!(is_display_id("ffffffff"));
    }

    #[test]
    fn test_is_display_id_invalid() {
        assert!(!is_display_id("abcd123")); // 7 chars
        assert!(!is_display_id("abcd12345")); // 9 chars
        assert!(!is_display_id("ghij1234")); // non-hex
        assert!(!is_display_id("")); // empty
        assert!(!is_display_id("not-hex!")); // special chars
    }

    // ---- test helper ----

    async fn test_key_service() -> crate::auth::KeyService {
        crate::db::register_sqlite_vec();
        let config = crate::config::Config {
            port: 0,
            db_path: ":memory:".to_string(),
            embedding_provider: "local".to_string(),
            openai_api_key: None,
            llm_provider: None,
            llm_api_key: None,
            llm_base_url: None,
            llm_model: None,
        };
        let conn = crate::db::open(&config).await.unwrap();
        crate::auth::KeyService::new(std::sync::Arc::new(conn))
    }

    // ---- cmd_create ----

    #[tokio::test]
    async fn test_cmd_create_creates_key() {
        let ks = test_key_service().await;
        // Verify no keys exist initially
        let keys = ks.list().await.unwrap();
        assert!(keys.is_empty());
        // cmd_create prints to stdout/stderr — verify side effect: key must exist after
        cmd_create(ks.clone(), "test-key".to_string(), None).await;
        let keys = ks.list().await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "test-key");
    }

    #[tokio::test]
    async fn test_cmd_create_scoped() {
        let ks = test_key_service().await;
        cmd_create(ks.clone(), "scoped-key".to_string(), Some("agent-x".to_string())).await;
        let keys = ks.list().await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].agent_id, Some("agent-x".to_string()));
    }

    // ---- cmd_list ----

    #[tokio::test]
    async fn test_cmd_list_empty_does_not_panic() {
        let ks = test_key_service().await;
        // Should not panic on empty list
        cmd_list(ks).await;
    }

    #[tokio::test]
    async fn test_cmd_list_with_keys_does_not_panic() {
        let ks = test_key_service().await;
        ks.create("key-a".to_string(), None).await.unwrap();
        ks.create("key-b".to_string(), Some("agent-1".to_string()))
            .await
            .unwrap();
        // Should not panic with multiple keys
        cmd_list(ks).await;
    }

    // ---- cmd_revoke ----

    #[tokio::test]
    async fn test_cmd_revoke_by_display_id() {
        let ks = test_key_service().await;
        let (api_key, _) = ks.create("revoke-me".to_string(), None).await.unwrap();
        // Revoke by display_id (success path — does not call exit)
        cmd_revoke(ks.clone(), api_key.display_id.clone()).await;
        // Verify the key is now revoked
        let keys = ks.list().await.unwrap();
        assert_eq!(keys.len(), 1);
        assert!(keys[0].revoked_at.is_some());
    }

    // ---- truncate helper ----

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 20), "hello");
    }

    #[test]
    fn test_truncate_exact() {
        assert_eq!(truncate("12345678901234567890", 20), "12345678901234567890");
    }

    #[test]
    fn test_truncate_long() {
        let result = truncate("123456789012345678901", 20);
        assert_eq!(result.len(), 20);
        assert!(result.ends_with("..."));
    }
}
