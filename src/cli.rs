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

    /// Output as JSON (machine-readable)
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Top-level subcommands.
#[derive(Subcommand)]
pub enum Commands {
    /// Start the HTTP server
    Serve,
    /// Manage API keys
    Keys(KeysArgs),
    /// Retrieve and list memories
    Recall(RecallArgs),
    /// Store a new memory
    Remember(RememberArgs),
    /// Semantic search over memories
    Search(SearchArgs),
    /// Compact similar memories
    Compact(CompactArgs),
    /// View configuration
    Config(ConfigArgs),
}

/// Arguments for the `keys` subcommand.
#[derive(Args)]
pub struct KeysArgs {
    #[command(subcommand)]
    pub subcommand: KeysSubcommand,
}

/// Arguments for the `recall` subcommand.
#[derive(Args)]
pub struct RecallArgs {
    /// Fetch a single memory by full UUID
    #[arg(long, value_name = "UUID")]
    pub id: Option<String>,

    /// Filter by agent_id
    #[arg(long, value_name = "ID")]
    pub agent_id: Option<String>,

    /// Filter by session_id
    #[arg(long, value_name = "ID")]
    pub session_id: Option<String>,

    /// Maximum number of results (default: 20)
    #[arg(long, value_name = "N", default_value_t = 20)]
    pub limit: u32,
}

/// Arguments for the `remember` subcommand.
#[derive(Args)]
pub struct RememberArgs {
    /// Memory content (or pipe via stdin)
    pub content: Option<String>,

    /// Associate memory with an agent
    #[arg(long, value_name = "ID")]
    pub agent_id: Option<String>,

    /// Associate memory with a session
    #[arg(long, value_name = "ID")]
    pub session_id: Option<String>,

    /// Comma-separated tags (e.g. "work,important")
    #[arg(long, value_name = "TAGS")]
    pub tags: Option<String>,
}

/// Arguments for the `search` subcommand.
#[derive(Args)]
pub struct SearchArgs {
    /// Search query
    pub query: String,

    /// Filter by agent_id
    #[arg(long, value_name = "ID")]
    pub agent_id: Option<String>,

    /// Filter by session_id
    #[arg(long, value_name = "ID")]
    pub session_id: Option<String>,

    /// Maximum number of results (default: 10)
    #[arg(long, value_name = "N", default_value_t = 10)]
    pub limit: u32,

    /// Maximum distance threshold (0.0 = exact match, higher = less similar)
    #[arg(long, value_name = "F")]
    pub threshold: Option<f32>,
}

/// Arguments for the `compact` subcommand.
#[derive(Args)]
pub struct CompactArgs {
    /// Scope compaction to a specific agent (default: compacts default namespace)
    #[arg(long, value_name = "ID")]
    pub agent_id: Option<String>,

    /// Similarity threshold for merging (default: 0.85)
    #[arg(long, value_name = "F")]
    pub threshold: Option<f32>,

    /// Max candidate memories to evaluate (default: 100)
    #[arg(long, value_name = "N")]
    pub max_candidates: Option<u32>,

    /// Preview what would be compacted without mutating data
    #[arg(long)]
    pub dry_run: bool,
}

/// Arguments for the `config` subcommand.
#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub subcommand: ConfigSubcommand,
}

/// Config subcommands.
#[derive(Subcommand)]
pub enum ConfigSubcommand {
    /// Display current configuration
    Show,
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
pub async fn run_keys(subcommand: KeysSubcommand, key_service: crate::auth::KeyService, json: bool) {
    match subcommand {
        KeysSubcommand::Create { name, agent_id } => cmd_create(key_service, name, agent_id, json).await,
        KeysSubcommand::List => cmd_list(key_service, json).await,
        KeysSubcommand::Revoke { id } => cmd_revoke(key_service, id, json).await,
    }
}

/// Entry point for `mnemonic config show`. No DB, no embedding, no validation needed (per D-17).
pub fn run_config_show(json_mode: bool) {
    let config = match crate::config::load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    if json_mode {
        // JSON output with redaction (per D-20)
        let obj = serde_json::json!({
            "port": config.port,
            "grpc_port": config.grpc_port,
            "db_path": config.db_path,
            "storage_provider": config.storage_provider,
            "embedding_provider": config.embedding_provider,
            "openai_api_key": redact_option(&config.openai_api_key),
            "llm_provider": config.llm_provider,
            "llm_api_key": redact_option(&config.llm_api_key),
            "llm_base_url": config.llm_base_url,
            "llm_model": config.llm_model,
            "qdrant_url": config.qdrant_url,
            "qdrant_api_key": redact_option(&config.qdrant_api_key),
            "postgres_url": redact_option(&config.postgres_url),
        });
        println!("{}", serde_json::to_string_pretty(&obj).unwrap());
    } else {
        // Human-readable output grouped logically (per D-18)
        println!("Server:");
        println!("  port             {}", config.port);
        println!("  grpc_port          {}", config.grpc_port);
        println!("  db_path          {}", config.db_path);
        println!();
        println!("Storage:");
        println!("  storage_provider {}", config.storage_provider);
        if let Some(ref url) = config.qdrant_url {
            println!("  qdrant_url       {}", url);
        }
        if config.qdrant_api_key.is_some() {
            println!("  qdrant_api_key   ****");
        }
        if config.postgres_url.is_some() {
            println!("  postgres_url     ****");
        }
        println!();
        println!("Embedding:");
        println!("  embedding_provider {}", config.embedding_provider);
        if config.openai_api_key.is_some() {
            println!("  openai_api_key     ****");
        }
        println!();
        println!("LLM:");
        println!("  llm_provider     {}", config.llm_provider.as_deref().unwrap_or("(none)"));
        if config.llm_api_key.is_some() {
            println!("  llm_api_key      ****");
        }
        if let Some(ref url) = config.llm_base_url {
            println!("  llm_base_url     {}", url);
        }
        if let Some(ref model) = config.llm_model {
            println!("  llm_model        {}", model);
        }
    }
}

/// Redact secret fields: any Some(value) becomes Some("****") (per D-19).
fn redact_option(opt: &Option<String>) -> serde_json::Value {
    match opt {
        Some(_) => serde_json::Value::String("****".to_string()),
        None => serde_json::Value::Null,
    }
}

/// Shared DB-only init sequence for fast-path subcommands (keys, recall).
/// Encapsulates: register_sqlite_vec -> load_config -> apply --db override -> open DB.
/// Deliberately skips validate_config() — fast-path commands don't use embeddings.
pub async fn init_db(db_override: Option<String>)
    -> anyhow::Result<(std::sync::Arc<tokio_rusqlite::Connection>, crate::config::Config)>
{
    crate::db::register_sqlite_vec();
    let mut config = crate::config::load_config()
        .map_err(|e| anyhow::anyhow!(e))?;
    if let Some(ref db_path) = db_override {
        config.db_path = db_path.clone();
    }
    let conn = crate::db::open(&config).await
        .map_err(|e| anyhow::anyhow!(e))?;
    let conn_arc = std::sync::Arc::new(conn);
    Ok((conn_arc, config))
}

/// Fast-path init for `mnemonic recall` -- DB + backend, no embedding.
/// Returns Arc<dyn StorageBackend> for trait-based list/get_by_id.
pub async fn init_recall(
    db_override: Option<String>,
) -> anyhow::Result<(std::sync::Arc<dyn crate::storage::StorageBackend>, crate::config::Config)> {
    let (conn_arc, config) = init_db(db_override).await?;
    let backend = crate::storage::create_backend(&config, conn_arc).await
        .map_err(|e| anyhow::anyhow!("backend creation failed: {}", e))?;
    Ok((backend, config))
}

/// Medium-init: DB + embedding engine for commands that need to embed content.
/// Reused by `remember` (Phase 17) and `search` (Phase 18).
pub async fn init_db_and_embedding(
    db_override: Option<String>,
) -> anyhow::Result<(crate::service::MemoryService, crate::config::Config)> {
    crate::db::register_sqlite_vec();
    let mut config = crate::config::load_config()
        .map_err(|e| anyhow::anyhow!(e))?;
    if let Some(ref db_path) = db_override {
        config.db_path = db_path.clone();
    }
    crate::config::validate_config(&config)?;  // required for embedding provider validation

    let conn = crate::db::open(&config).await
        .map_err(|e| anyhow::anyhow!(e))?;
    let conn_arc = std::sync::Arc::new(conn);

    let embedding_model = match config.embedding_provider.as_str() {
        "openai" => "text-embedding-3-small".to_string(),
        _        => "all-MiniLM-L6-v2".to_string(),
    };

    let embedding: std::sync::Arc<dyn crate::embedding::EmbeddingEngine> =
        match config.embedding_provider.as_str() {
            "local" => {
                eprintln!("Loading embedding model...");
                let start = std::time::Instant::now();
                let engine = tokio::task::spawn_blocking(|| {
                    crate::embedding::LocalEngine::new()
                })
                .await?
                .map_err(|e| anyhow::anyhow!(e))?;
                eprintln!("Model loaded ({}ms)", start.elapsed().as_millis());
                std::sync::Arc::new(engine)
            }
            "openai" => {
                let api_key = config.openai_api_key.as_ref().unwrap(); // safe: validate_config passed
                std::sync::Arc::new(crate::embedding::OpenAiEngine::new(api_key.clone()))
            }
            _ => unreachable!(), // validate_config rejects unknown providers
        };

    let backend: std::sync::Arc<dyn crate::storage::StorageBackend> =
        crate::storage::create_backend(&config, conn_arc).await
            .map_err(|e| anyhow::anyhow!("backend creation failed: {}", e))?;
    let service = crate::service::MemoryService::new(backend, embedding, embedding_model);
    Ok((service, config))
}

/// Full-init: DB + embedding + optional LLM for commands that need CompactionService.
/// Used by `compact` (Phase 19). Cannot reuse init_db_and_embedding() because that
/// returns MemoryService -- compact needs the individual components for CompactionService.
pub async fn init_compaction(
    db_override: Option<String>,
) -> anyhow::Result<(crate::compaction::CompactionService, crate::config::Config)> {
    crate::db::register_sqlite_vec();
    let mut config = crate::config::load_config()
        .map_err(|e| anyhow::anyhow!(e))?;
    if let Some(ref db_path) = db_override {
        config.db_path = db_path.clone();
    }
    crate::config::validate_config(&config)?;

    let conn = crate::db::open(&config).await
        .map_err(|e| anyhow::anyhow!(e))?;
    let conn_arc = std::sync::Arc::new(conn);

    let embedding_model = match config.embedding_provider.as_str() {
        "openai" => "text-embedding-3-small".to_string(),
        _        => "all-MiniLM-L6-v2".to_string(),
    };

    let embedding: std::sync::Arc<dyn crate::embedding::EmbeddingEngine> =
        match config.embedding_provider.as_str() {
            "local" => {
                eprintln!("Loading embedding model...");
                let start = std::time::Instant::now();
                let engine = tokio::task::spawn_blocking(|| {
                    crate::embedding::LocalEngine::new()
                })
                .await?
                .map_err(|e| anyhow::anyhow!(e))?;
                eprintln!("Model loaded ({}ms)", start.elapsed().as_millis());
                std::sync::Arc::new(engine)
            }
            "openai" => {
                let api_key = config.openai_api_key.as_ref().unwrap();
                std::sync::Arc::new(crate::embedding::OpenAiEngine::new(api_key.clone()))
            }
            _ => unreachable!(),
        };

    // LLM summarization engine -- optional (per D-05)
    let llm_engine: Option<std::sync::Arc<dyn crate::summarization::SummarizationEngine>> =
        match config.llm_provider.as_deref() {
            Some("openai") => {
                let api_key = config.llm_api_key.as_ref().unwrap();
                let base_url = config.llm_base_url.clone()
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
                let model = config.llm_model.clone()
                    .unwrap_or_else(|| "gpt-4o-mini".to_string());
                eprintln!("LLM summarization: enabled (openai/{})", model);
                Some(std::sync::Arc::new(
                    crate::summarization::OpenAiSummarizer::new(api_key.clone(), base_url, model)
                ))
            }
            None => {
                eprintln!("LLM summarization: disabled (algorithmic merge only)");
                None
            }
            _ => unreachable!(),
        };

    let backend: std::sync::Arc<dyn crate::storage::StorageBackend> =
        crate::storage::create_backend(&config, conn_arc.clone()).await
            .map_err(|e| anyhow::anyhow!("backend creation failed: {}", e))?;
    let compaction = crate::compaction::CompactionService::new(
        backend,
        conn_arc,
        embedding,
        llm_engine,
        embedding_model,
    );
    Ok((compaction, config))
}

/// Entry point for `mnemonic compact` -- triggers compaction via CompactionService.
/// All args are optional with sensible defaults -- no early validation needed (per D-19).
pub async fn run_compact(args: CompactArgs, compaction: crate::compaction::CompactionService, json: bool) {
    let dry_run = args.dry_run;
    let max_candidates_val = args.max_candidates;
    let agent_id = args.agent_id.unwrap_or_default();  // "" for default namespace (per D-09)

    let req = crate::compaction::CompactRequest {
        agent_id,
        threshold: args.threshold,
        max_candidates: max_candidates_val,
        dry_run: Some(dry_run),
    };

    match compaction.compact(req).await {
        Ok(resp) => {
            // Audit trail to stderr (per D-15) — always printed regardless of json mode
            let run_id_short = &resp.run_id[..8.min(resp.run_id.len())];
            eprintln!("Run: {}", run_id_short);

            // Truncation warning to stderr (per D-14) — always printed regardless of json mode
            if resp.truncated {
                let max = max_candidates_val.unwrap_or(100);
                eprintln!(
                    "Note: only {} most recent memories were evaluated. \
                     Increase --max-candidates for broader coverage.",
                    max
                );
            }

            if json {
                println!("{}", serde_json::to_string_pretty(&resp).unwrap());
            } else {
                if resp.clusters_found == 0 {
                    // Per D-13: exit 0, no error
                    println!("No similar memories found to compact.");
                    return;
                }

                if dry_run {
                    // Per D-12: dry-run uses clusters_found for new memory count
                    // (memories_created is 0 in dry-run mode -- Pitfall 2)
                    println!(
                        "Dry run: {} clusters, {} memories would be merged \u{2192} {} new memories",
                        resp.clusters_found, resp.memories_merged, resp.clusters_found
                    );
                } else {
                    // Per D-12: actual compaction uses memories_created
                    println!(
                        "Compacted: {} clusters, {} memories merged \u{2192} {} new memories",
                        resp.clusters_found, resp.memories_merged, resp.memories_created
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("error: compaction failed: {}", e);
            std::process::exit(1);
        }
    }
}

/// Entry point for `mnemonic recall` — dispatches to list or get-by-id.
pub async fn run_recall(args: RecallArgs, backend: std::sync::Arc<dyn crate::storage::StorageBackend>, json: bool) {
    if let Some(id) = args.id {
        // Runtime mutual exclusivity check: --id cannot be combined with filter flags
        if args.agent_id.is_some() || args.session_id.is_some() || args.limit != 20 {
            eprintln!("error: --id cannot be combined with --agent-id, --session-id, or --limit");
            std::process::exit(1);
        }
        cmd_get_memory(backend.clone(), id, json).await;
    } else {
        cmd_list_memories(backend, args.agent_id, args.session_id, args.limit, json).await;
    }
}

/// Entry point for `mnemonic remember` -- stores a memory via MemoryService.
/// Content must already be resolved (from positional arg or stdin) before calling.
pub async fn run_remember(content: String, args: RememberArgs, service: crate::service::MemoryService, json: bool) {
    // Parse tags from comma-separated string
    let tags: Vec<String> = args.tags
        .unwrap_or_default()
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    let tags_opt = if tags.is_empty() { None } else { Some(tags) };

    let req = crate::service::CreateMemoryRequest {
        content,
        agent_id: args.agent_id,     // already Option<String> from clap
        session_id: args.session_id,  // already Option<String> from clap
        tags: tags_opt,
    };

    match service.create_memory(req).await {
        Ok(memory) => {
            if json {
                let obj = serde_json::json!({"id": memory.id});
                println!("{}", serde_json::to_string_pretty(&obj).unwrap());
            } else {
                println!("{}", memory.id);  // full UUID on stdout line 1
                let short_id = &memory.id[..8.min(memory.id.len())];
                eprintln!("Stored memory {}", short_id);  // human context to stderr
            }
        }
        Err(e) => {
            eprintln!("error: failed to store memory: {}", e);
            std::process::exit(1);
        }
    }
}

/// Entry point for `mnemonic search` -- performs semantic search via MemoryService.
/// query must already be validated (not empty) before calling.
pub async fn run_search(query: String, args: SearchArgs, service: crate::service::MemoryService, json: bool) {
    let params = crate::service::SearchParams {
        q: Some(query),
        agent_id: args.agent_id,
        session_id: args.session_id,
        tag: None,
        limit: Some(args.limit),
        threshold: args.threshold,
        after: None,
        before: None,
    };

    match service.search_memories(params).await {
        Ok(resp) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&resp).unwrap());
            } else {
                if resp.memories.is_empty() {
                    println!("No matching memories found.");
                    return;
                }

                // Table header per D-10: DIST(6), ID(8), CONTENT(50), AGENT(15)
                let header = format!("{:<6}  {:<8}  {:<50}  {}", "DIST", "ID", "CONTENT", "AGENT");
                println!("{}", header);
                println!("{}", "-".repeat(header.len()));

                for item in &resp.memories {
                    let dist = format!("{:.4}", item.distance);
                    let id_short = if item.memory.id.len() >= 8 {
                        &item.memory.id[..8]
                    } else {
                        &item.memory.id
                    };
                    let content = truncate(&item.memory.content, 50);
                    let agent = if item.memory.agent_id.is_empty() {
                        "(none)".to_string()
                    } else {
                        truncate(&item.memory.agent_id, 15)
                    };
                    println!("{:<6}  {:<8}  {:<50}  {}", dist, id_short, content, agent);
                }

                // Footer per D-15: singular/plural
                let n = resp.memories.len();
                if n == 1 {
                    println!("Found 1 result");
                } else {
                    println!("Found {} results", n);
                }
            }
        }
        Err(e) => {
            eprintln!("error: search failed: {}", e);
            std::process::exit(1);
        }
    }
}

/// Handler for `mnemonic recall` (no --id) — lists memories in table format.
async fn cmd_list_memories(
    backend: std::sync::Arc<dyn crate::storage::StorageBackend>,
    agent_id: Option<String>,
    session_id: Option<String>,
    limit: u32,
    json: bool,
) {
    let params = crate::service::ListParams {
        agent_id,
        session_id,
        limit: Some(limit),
        tag: None,
        after: None,
        before: None,
        offset: None,
    };
    let result = backend.list(params).await;

    match result {
        Ok(resp) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&resp).unwrap());
            } else {
                if resp.memories.is_empty() {
                    println!("No memories found.");
                    return;
                }

                // Table format: ID(8), CONTENT(60), AGENT(15), CREATED(19)
                let header = format!("{:<8}  {:<60}  {:<15}  {}", "ID", "CONTENT", "AGENT", "CREATED");
                println!("{}", header);
                println!("{}", "-".repeat(header.len()));

                for mem in &resp.memories {
                    let id_short = if mem.id.len() >= 8 { &mem.id[..8] } else { &mem.id };
                    let content = truncate(&mem.content, 60);
                    let agent = if mem.agent_id.is_empty() {
                        "(none)".to_string()
                    } else {
                        truncate(&mem.agent_id, 15)
                    };
                    let created = if mem.created_at.len() >= 19 {
                        &mem.created_at[..19]
                    } else {
                        &mem.created_at
                    };
                    println!("{:<8}  {:<60}  {:<15}  {}", id_short, content, agent, created);
                }

                // Footer
                println!("Showing {} of {} memories", resp.memories.len(), resp.total);
            }
        }
        Err(e) => {
            eprintln!("error: failed to list memories: {}", e);
            std::process::exit(1);
        }
    }
}

/// Handler for `mnemonic recall --id <uuid>` — displays full memory detail.
async fn cmd_get_memory(backend: std::sync::Arc<dyn crate::storage::StorageBackend>, id: String, json: bool) {
    let result = backend.get_by_id(&id).await;

    match result {
        Ok(Some(mem)) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&mem).unwrap());
            } else {
                println!("ID:       {}", mem.id);
                println!("Content:  {}", mem.content);
                println!("Agent:    {}", if mem.agent_id.is_empty() { "(none)" } else { &mem.agent_id });
                println!("Session:  {}", if mem.session_id.is_empty() { "(none)" } else { &mem.session_id });
                let tags_display = if mem.tags.is_empty() {
                    "(none)".to_string()
                } else {
                    mem.tags.join(", ")
                };
                println!("Tags:     {}", tags_display);
                println!("Model:    {}", mem.embedding_model);
                println!("Created:  {}", mem.created_at);
                println!("Updated:  {}", mem.updated_at.as_deref().unwrap_or("(never)"));
            }
        }
        Ok(None) => {
            eprintln!("No memory found with ID {}", id);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("error: failed to get memory: {}", e);
            std::process::exit(1);
        }
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
async fn cmd_create(key_service: crate::auth::KeyService, name: String, agent_id: Option<String>, json: bool) {
    match key_service.create(name, agent_id).await {
        Ok((api_key, raw_token)) => {
            if json {
                let obj = serde_json::json!({
                    "token": raw_token,
                    "id": api_key.display_id,
                    "name": api_key.name,
                    "scope": api_key.agent_id,
                });
                println!("{}", serde_json::to_string_pretty(&obj).unwrap());
            } else {
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
async fn cmd_list(key_service: crate::auth::KeyService, json: bool) {
    match key_service.list().await {
        Ok(keys) => {
            if json {
                println!("{}", serde_json::to_string_pretty(&keys).unwrap());
            } else {
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
async fn cmd_revoke(key_service: crate::auth::KeyService, id: String, json: bool) {
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
                        Ok(()) => {
                            if json {
                                println!("{}", serde_json::to_string_pretty(&serde_json::json!({"revoked": true})).unwrap());
                            } else {
                                println!("Key {} revoked", display);
                            }
                        }
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
            Ok(()) => {
                if json {
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"revoked": true})).unwrap());
                } else {
                    println!("Key revoked");
                }
            }
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
            ..crate::config::Config::default()
        };
        let conn = crate::db::open(&config).await.unwrap();
        crate::auth::KeyService::new(std::sync::Arc::new(conn))
    }

    async fn test_backend() -> std::sync::Arc<dyn crate::storage::StorageBackend> {
        crate::db::register_sqlite_vec();
        let config = crate::config::Config {
            port: 0,
            db_path: ":memory:".to_string(),
            ..crate::config::Config::default()
        };
        let conn = crate::db::open(&config).await.unwrap();
        let conn_arc = std::sync::Arc::new(conn);
        crate::storage::create_backend(&config, conn_arc).await.unwrap()
    }

    // ---- recall delegation tests ----

    #[tokio::test]
    async fn test_recall_list_delegates_to_backend() {
        let backend = test_backend().await;
        // Seed a memory via the trait (not raw SQL)
        let store_req = crate::storage::StoreRequest {
            id: uuid::Uuid::now_v7().to_string(),
            content: "test memory content".to_string(),
            agent_id: "agent-1".to_string(),
            session_id: "sess-1".to_string(),
            tags: vec![],
            embedding: vec![0.0; 384],
            embedding_model: "test".to_string(),
        };
        backend.store(store_req).await.unwrap();

        // Verify backend.list() returns the seeded memory
        let params = crate::service::ListParams {
            agent_id: Some("agent-1".to_string()),
            session_id: None,
            tag: None,
            after: None,
            before: None,
            limit: Some(20),
            offset: None,
        };
        let resp = backend.list(params).await.unwrap();
        assert_eq!(resp.memories.len(), 1);
        assert_eq!(resp.memories[0].content, "test memory content");
        assert_eq!(resp.total, 1);

        // Call cmd_list_memories with the backend -- verifies it compiles and runs
        // against Arc<dyn StorageBackend> (not raw Connection)
        cmd_list_memories(backend, Some("agent-1".to_string()), None, 20, false).await;
    }

    #[tokio::test]
    async fn test_recall_get_by_id_delegates_to_backend() {
        let backend = test_backend().await;
        // Seed a memory
        let id = uuid::Uuid::now_v7().to_string();
        let store_req = crate::storage::StoreRequest {
            id: id.clone(),
            content: "specific memory".to_string(),
            agent_id: "agent-2".to_string(),
            session_id: "".to_string(),
            tags: vec![],
            embedding: vec![0.0; 384],
            embedding_model: "test".to_string(),
        };
        let stored = backend.store(store_req).await.unwrap();

        // Verify backend.get_by_id() returns the memory
        let fetched = backend.get_by_id(&stored.id).await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().content, "specific memory");

        // Call cmd_get_memory with the backend -- verifies trait delegation
        cmd_get_memory(backend, stored.id, false).await;
    }

    // ---- cmd_create ----

    #[tokio::test]
    async fn test_cmd_create_creates_key() {
        let ks = test_key_service().await;
        // Verify no keys exist initially
        let keys = ks.list().await.unwrap();
        assert!(keys.is_empty());
        // cmd_create prints to stdout/stderr — verify side effect: key must exist after
        cmd_create(ks.clone(), "test-key".to_string(), None, false).await;
        let keys = ks.list().await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "test-key");
    }

    #[tokio::test]
    async fn test_cmd_create_scoped() {
        let ks = test_key_service().await;
        cmd_create(ks.clone(), "scoped-key".to_string(), Some("agent-x".to_string()), false).await;
        let keys = ks.list().await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].agent_id, Some("agent-x".to_string()));
    }

    // ---- cmd_list ----

    #[tokio::test]
    async fn test_cmd_list_empty_does_not_panic() {
        let ks = test_key_service().await;
        // Should not panic on empty list
        cmd_list(ks, false).await;
    }

    #[tokio::test]
    async fn test_cmd_list_with_keys_does_not_panic() {
        let ks = test_key_service().await;
        ks.create("key-a".to_string(), None).await.unwrap();
        ks.create("key-b".to_string(), Some("agent-1".to_string()))
            .await
            .unwrap();
        // Should not panic with multiple keys
        cmd_list(ks, false).await;
    }

    // ---- cmd_revoke ----

    #[tokio::test]
    async fn test_cmd_revoke_by_display_id() {
        let ks = test_key_service().await;
        let (api_key, _) = ks.create("revoke-me".to_string(), None).await.unwrap();
        // Revoke by display_id (success path — does not call exit)
        cmd_revoke(ks.clone(), api_key.display_id.clone(), false).await;
        // Verify the key is now revoked
        let keys = ks.list().await.unwrap();
        assert_eq!(keys.len(), 1);
        assert!(keys[0].revoked_at.is_some());
    }

    // ---- redact_option ----

    #[test]
    fn test_redact_option_some_returns_stars() {
        let opt = Some("super-secret-key".to_string());
        let result = redact_option(&opt);
        assert_eq!(
            result,
            serde_json::Value::String("****".to_string()),
            "Some value must be redacted as ****"
        );
    }

    #[test]
    fn test_redact_option_none_returns_null() {
        let opt: Option<String> = None;
        let result = redact_option(&opt);
        assert_eq!(
            result,
            serde_json::Value::Null,
            "None value must produce JSON null"
        );
    }

    #[test]
    fn test_redact_option_some_hides_actual_value() {
        // The actual secret must NOT appear in the output regardless of content
        let opt = Some("sk-1234567890abcdef".to_string());
        let result = redact_option(&opt);
        let serialized = result.to_string();
        assert!(
            !serialized.contains("sk-1234567890abcdef"),
            "redacted output must not contain original secret; got: {}",
            serialized
        );
        assert!(serialized.contains("****"), "output must contain ****; got: {}", serialized);
    }

    // ---- CONF-03: postgres_url redaction ----

    #[test]
    fn test_conf03_postgres_url_redacted_in_json() {
        let dsn = Some("postgres://user:secret@localhost/mnemonic".to_string());
        let result = redact_option(&dsn);
        assert_eq!(
            result,
            serde_json::Value::String("****".to_string()),
            "postgres_url must be redacted as ****"
        );
        let serialized = result.to_string();
        assert!(
            !serialized.contains("secret"),
            "redacted output must not contain password; got: {}",
            serialized
        );
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
