//! CLI integration tests for Phase 14 — keys create/list/revoke subcommands.
//!
//! These tests invoke the compiled binary directly via std::process::Command,
//! using --db to point each test at an isolated temp SQLite file.
//!
//! Requirements covered:
//!   CLI-01 — `mnemonic keys create <name>` prints raw mnk_ token and exits 0
//!   CLI-02 — `mnemonic keys list` prints a table (or empty-state message) and exits 0
//!   CLI-03 — `mnemonic keys revoke <id>` revokes a key by display_id and exits 0

use std::path::PathBuf;
use std::process::Command;

/// Returns the path to the compiled mnemonic binary.
fn binary() -> PathBuf {
    let mut path = std::env::current_exe()
        .expect("current_exe must be accessible")
        .parent()
        .expect("test binary has a parent directory")
        .to_path_buf();

    // cargo test --test cli_integration places test binary in target/debug/deps/
    // The mnemonic binary is two levels up in target/debug/
    if path.ends_with("deps") {
        path.pop();
    }

    path.push("mnemonic");
    path
}

/// Creates a unique temp db path for a test, cleaned up via drop.
struct TempDb {
    pub path: PathBuf,
}

impl TempDb {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("mnemonic_cli_test_{}.db", label));
        // Remove any leftover from a previous run
        let _ = std::fs::remove_file(&path);
        Self { path }
    }

    fn path_str(&self) -> &str {
        self.path.to_str().expect("temp path must be valid UTF-8")
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        // SQLite WAL/SHM companions
        let _ = std::fs::remove_file(format!("{}-wal", self.path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", self.path.display()));
    }
}

// ---- CLI-01: keys create -------------------------------------------------------

/// CLI-01: `mnemonic keys create <name>` exits 0 and prints a raw mnk_ token.
#[test]
fn test_keys_create_exits_zero_and_prints_token() {
    let db = TempDb::new("create_token");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "create", "my-test-key"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "keys create must exit 0; stderr: {}",
        stderr
    );

    // Raw token must be the first line of stdout
    let first_line = stdout.lines().next().unwrap_or("");
    assert!(
        first_line.starts_with("mnk_"),
        "first stdout line must be the raw token (starts with mnk_); got: {:?}",
        first_line
    );
    assert_eq!(
        first_line.len(),
        68,
        "raw token must be 68 chars (mnk_ + 64 hex); got len {}",
        first_line.len()
    );
}

/// CLI-01: `mnemonic keys create` prints key metadata (ID, Name, Scope) to stdout.
#[test]
fn test_keys_create_prints_metadata() {
    let db = TempDb::new("create_metadata");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "create", "metadata-key"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "keys create must exit 0");
    assert!(stdout.contains("ID:"), "stdout must contain 'ID:' field; got: {}", stdout);
    assert!(stdout.contains("Name:"), "stdout must contain 'Name:' field; got: {}", stdout);
    assert!(stdout.contains("Scope:"), "stdout must contain 'Scope:' field; got: {}", stdout);
    assert!(
        stdout.contains("metadata-key"),
        "stdout must contain the key name; got: {}",
        stdout
    );
}

/// CLI-01: `mnemonic keys create` prints the "Save this key" warning to stderr.
#[test]
fn test_keys_create_prints_save_warning_to_stderr() {
    let db = TempDb::new("create_warning");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "create", "warn-key"])
        .output()
        .expect("failed to run mnemonic binary");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "keys create must exit 0");
    assert!(
        stderr.contains("Save this key"),
        "stderr must contain 'Save this key' warning; got: {:?}",
        stderr
    );
}

/// CLI-01: `mnemonic keys create --agent-id <id>` scopes the key and shows agent_id in output.
#[test]
fn test_keys_create_scoped_shows_agent_id() {
    let db = TempDb::new("create_scoped");
    let bin = binary();

    let output = Command::new(&bin)
        .args([
            "--db",
            db.path_str(),
            "keys",
            "create",
            "scoped-key",
            "--agent-id",
            "agent-xyz",
        ])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "keys create --agent-id must exit 0");
    assert!(
        stdout.contains("agent-xyz"),
        "stdout must contain the agent_id; got: {}",
        stdout
    );
}

// ---- CLI-02: keys list ----------------------------------------------------------

/// CLI-02: `mnemonic keys list` exits 0 when no keys exist (empty-state message).
#[test]
fn test_keys_list_empty_state_exits_zero() {
    let db = TempDb::new("list_empty");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "list"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "keys list (empty) must exit 0; stderr: {}",
        stderr
    );
    assert!(
        stdout.contains("No API keys found"),
        "empty list must print 'No API keys found'; got: {}",
        stdout
    );
}

/// CLI-02: `mnemonic keys list` prints a table with required column headers after creating a key.
#[test]
fn test_keys_list_prints_table_with_headers() {
    let db = TempDb::new("list_headers");
    let bin = binary();

    // Create a key first
    let create = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "create", "list-test-key"])
        .output()
        .expect("failed to run mnemonic binary");
    assert!(create.status.success(), "keys create must succeed before list test");

    // Now list
    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "list"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "keys list must exit 0; stderr: {}",
        stderr
    );

    // All five required column headers must appear
    for header in &["ID", "NAME", "SCOPE", "CREATED", "STATUS"] {
        assert!(
            stdout.contains(header),
            "table must contain '{}' header; got:\n{}",
            header,
            stdout
        );
    }
}

/// CLI-02: `mnemonic keys list` shows the display_id and "active" status for a live key.
#[test]
fn test_keys_list_shows_active_key_row() {
    let db = TempDb::new("list_active");
    let bin = binary();

    // Create a key and capture its display_id from the output
    let create = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "create", "active-key"])
        .output()
        .expect("failed to run mnemonic binary");
    assert!(create.status.success());

    let create_stdout = String::from_utf8_lossy(&create.stdout);
    // The display_id appears on the "ID:    <8-hex-chars>" line
    let display_id = create_stdout
        .lines()
        .find(|l| l.starts_with("ID:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .expect("keys create must print an ID: line");

    // List and verify the display_id and "active" status appear
    let list = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "list"])
        .output()
        .expect("failed to run mnemonic binary");

    let list_stdout = String::from_utf8_lossy(&list.stdout);
    assert!(list.status.success(), "keys list must exit 0");
    assert!(
        list_stdout.contains(display_id),
        "list output must contain the display_id {}; got:\n{}",
        display_id,
        list_stdout
    );
    assert!(
        list_stdout.contains("active"),
        "list output must show 'active' status; got:\n{}",
        list_stdout
    );
}

// ---- CLI-03: keys revoke --------------------------------------------------------

/// CLI-03: `mnemonic keys revoke <display_id>` exits 0 and prints confirmation.
#[test]
fn test_keys_revoke_by_display_id_exits_zero() {
    let db = TempDb::new("revoke_success");
    let bin = binary();

    // Create a key
    let create = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "create", "revoke-me"])
        .output()
        .expect("failed to run mnemonic binary");
    assert!(create.status.success());

    let create_stdout = String::from_utf8_lossy(&create.stdout);
    let display_id = create_stdout
        .lines()
        .find(|l| l.starts_with("ID:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .expect("keys create must print an ID: line");

    // Revoke by display_id
    let revoke = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "revoke", display_id])
        .output()
        .expect("failed to run mnemonic binary");

    let revoke_stdout = String::from_utf8_lossy(&revoke.stdout);
    let revoke_stderr = String::from_utf8_lossy(&revoke.stderr);

    assert!(
        revoke.status.success(),
        "keys revoke must exit 0; stderr: {}",
        revoke_stderr
    );
    assert!(
        revoke_stdout.contains("revoked"),
        "revoke output must contain 'revoked'; got: {}",
        revoke_stdout
    );
}

/// CLI-03: After revoke, `keys list` shows the key with "revoked" status.
#[test]
fn test_keys_revoke_key_appears_revoked_in_list() {
    let db = TempDb::new("revoke_list");
    let bin = binary();

    // Create a key
    let create = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "create", "check-revoke"])
        .output()
        .expect("failed to run mnemonic binary");
    assert!(create.status.success());

    let create_stdout = String::from_utf8_lossy(&create.stdout);
    let display_id = create_stdout
        .lines()
        .find(|l| l.starts_with("ID:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .expect("keys create must print an ID: line");

    // Revoke it
    let revoke = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "revoke", display_id])
        .output()
        .expect("failed to run mnemonic binary");
    assert!(revoke.status.success());

    // List and confirm revoked status
    let list = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "list"])
        .output()
        .expect("failed to run mnemonic binary");

    let list_stdout = String::from_utf8_lossy(&list.stdout);
    assert!(list.status.success(), "keys list must exit 0 after revoke");
    assert!(
        list_stdout.contains("revoked"),
        "list must show 'revoked' status after revoke; got:\n{}",
        list_stdout
    );
}

/// CLI-03: `mnemonic keys revoke <nonexistent_display_id>` exits non-zero with error message.
#[test]
fn test_keys_revoke_nonexistent_display_id_exits_nonzero() {
    let db = TempDb::new("revoke_notfound");
    let bin = binary();

    let revoke = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "revoke", "00000000"])
        .output()
        .expect("failed to run mnemonic binary");

    let stderr = String::from_utf8_lossy(&revoke.stderr);

    assert!(
        !revoke.status.success(),
        "keys revoke of nonexistent key must exit non-zero"
    );
    assert!(
        stderr.contains("No key found"),
        "stderr must contain 'No key found'; got: {}",
        stderr
    );
}

/// Seeds a memory row directly into the SQLite database for testing recall.
/// Uses rusqlite synchronously (not tokio) since test setup is blocking.
fn seed_memory(db_path: &str, id: &str, content: &str, agent_id: &str, session_id: &str, tags: &str, created_at: &str) {
    let conn = rusqlite::Connection::open(db_path).expect("open temp db for seeding");
    // Ensure schema exists (db::open creates it, but we need it before first binary run)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS memories (
            id TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            agent_id TEXT NOT NULL DEFAULT '',
            session_id TEXT NOT NULL DEFAULT '',
            tags TEXT NOT NULL DEFAULT '[]',
            embedding_model TEXT NOT NULL DEFAULT '',
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME
        );"
    ).expect("create schema for seeding");
    conn.execute(
        "INSERT INTO memories (id, content, agent_id, session_id, tags, embedding_model, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'test-model', ?6)",
        rusqlite::params![id, content, agent_id, session_id, tags, created_at],
    ).expect("seed memory row");
}

// ---- Phase 15: serve subcommand ------------------------------------------------

/// CLI-01: `mnemonic --help` lists `serve` as a subcommand.
#[test]
fn test_serve_appears_in_help() {
    let bin = binary();
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "--help must exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("serve"),
        "--help must list 'serve' subcommand; got:\n{}",
        stdout
    );
}

/// CLI-01: `mnemonic --help` shows the correct help text for `serve`.
#[test]
fn test_serve_help_text_description() {
    let bin = binary();
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "--help must exit 0");
    assert!(
        stdout.contains("Start the HTTP server"),
        "--help must show 'Start the HTTP server' for serve subcommand; got:\n{}",
        stdout
    );
}

// ---- Phase 16: recall subcommand ------------------------------------------------

/// RCL-01: `mnemonic recall` with no memories prints "No memories found." and exits 0.
#[test]
fn test_recall_empty_state() {
    let db = TempDb::new("recall_empty");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "recall"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "recall (empty) must exit 0");
    assert!(
        stdout.contains("No memories found."),
        "empty recall must print 'No memories found.'; got: {}",
        stdout
    );
}

/// RCL-01: `mnemonic recall` lists memories with table headers ID, CONTENT, AGENT, CREATED.
#[test]
fn test_recall_lists_with_table_headers() {
    let db = TempDb::new("recall_headers");
    let bin = binary();

    seed_memory(db.path_str(), "aaaaaaaa-1111-2222-3333-444444444444",
        "Test memory content", "agent-1", "session-1", "[]", "2026-01-01 00:00:00");

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "recall"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "recall must exit 0; stderr: {}", String::from_utf8_lossy(&output.stderr));

    for header in &["ID", "CONTENT", "AGENT", "CREATED"] {
        assert!(
            stdout.contains(header),
            "table must contain '{}' header; got:\n{}",
            header, stdout
        );
    }
}

/// RCL-01: `mnemonic recall` shows truncated ID (first 8 chars) and memory content.
#[test]
fn test_recall_shows_truncated_id_and_content() {
    let db = TempDb::new("recall_content");
    let bin = binary();

    seed_memory(db.path_str(), "bbbbbbbb-1111-2222-3333-444444444444",
        "Remember to water the plants", "agent-x", "", "[]", "2026-01-02 00:00:00");

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "recall"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "recall must exit 0");
    assert!(stdout.contains("bbbbbbbb"), "output must contain first 8 chars of UUID; got:\n{}", stdout);
    assert!(stdout.contains("Remember to water the plants"), "output must contain memory content; got:\n{}", stdout);
}

/// RCL-01: `mnemonic recall` shows footer "Showing X of Y memories".
#[test]
fn test_recall_shows_footer() {
    let db = TempDb::new("recall_footer");
    let bin = binary();

    seed_memory(db.path_str(), "cccccccc-1111-2222-3333-444444444444",
        "Footer test memory", "", "", "[]", "2026-01-03 00:00:00");

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "recall"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "recall must exit 0");
    assert!(
        stdout.contains("Showing 1 of 1 memories"),
        "footer must show 'Showing 1 of 1 memories'; got:\n{}",
        stdout
    );
}

/// RCL-01: `mnemonic recall` shows `(none)` for empty agent_id.
#[test]
fn test_recall_shows_none_for_empty_agent() {
    let db = TempDb::new("recall_none_agent");
    let bin = binary();

    seed_memory(db.path_str(), "dddddddd-1111-2222-3333-444444444444",
        "Memory with no agent", "", "", "[]", "2026-01-04 00:00:00");

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "recall"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "recall must exit 0");
    assert!(stdout.contains("(none)"), "empty agent_id must display as '(none)'; got:\n{}", stdout);
}

// ---- RCL-02: recall --id --------------------------------------------------------

/// RCL-02: `mnemonic recall --id <uuid>` prints full detail in key-value format.
#[test]
fn test_recall_by_id_shows_detail() {
    let db = TempDb::new("recall_by_id");
    let bin = binary();

    let test_id = "eeeeeeee-1111-2222-3333-444444444444";
    seed_memory(db.path_str(), test_id,
        "Detailed memory content for testing", "agent-detail", "session-detail",
        "[\"tag1\",\"tag2\"]", "2026-02-01 12:00:00");

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "recall", "--id", test_id])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "recall --id must exit 0; stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Verify all key-value labels from D-13
    assert!(stdout.contains("ID:"), "detail must contain 'ID:' label; got:\n{}", stdout);
    assert!(stdout.contains(test_id), "detail must contain the full UUID; got:\n{}", stdout);
    assert!(stdout.contains("Content:"), "detail must contain 'Content:' label; got:\n{}", stdout);
    assert!(stdout.contains("Detailed memory content for testing"), "detail must contain full content; got:\n{}", stdout);
    assert!(stdout.contains("Agent:"), "detail must contain 'Agent:' label; got:\n{}", stdout);
    assert!(stdout.contains("agent-detail"), "detail must contain agent_id; got:\n{}", stdout);
    assert!(stdout.contains("Session:"), "detail must contain 'Session:' label; got:\n{}", stdout);
    assert!(stdout.contains("session-detail"), "detail must contain session_id; got:\n{}", stdout);
    assert!(stdout.contains("Tags:"), "detail must contain 'Tags:' label; got:\n{}", stdout);
    assert!(stdout.contains("tag1"), "detail must contain tag1; got:\n{}", stdout);
    assert!(stdout.contains("tag2"), "detail must contain tag2; got:\n{}", stdout);
    assert!(stdout.contains("Model:"), "detail must contain 'Model:' label; got:\n{}", stdout);
    assert!(stdout.contains("Created:"), "detail must contain 'Created:' label; got:\n{}", stdout);
    assert!(stdout.contains("Updated:"), "detail must contain 'Updated:' label; got:\n{}", stdout);
}

/// RCL-02: `mnemonic recall --id <nonexistent>` exits 1 with error on stderr.
#[test]
fn test_recall_by_id_not_found_exits_one() {
    let db = TempDb::new("recall_notfound");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "recall", "--id", "00000000-0000-0000-0000-000000000000"])
        .output()
        .expect("failed to run mnemonic binary");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "recall --id (not found) must exit non-zero"
    );
    assert!(
        stderr.contains("No memory found with ID"),
        "stderr must contain 'No memory found with ID'; got: {}",
        stderr
    );
}

// ---- RCL-03: recall filters -----------------------------------------------------

/// RCL-03: `mnemonic recall --agent-id <id>` filters results to matching agent.
#[test]
fn test_recall_filter_agent_id() {
    let db = TempDb::new("recall_filter_agent");
    let bin = binary();

    // Seed two memories with different agent_ids
    seed_memory(db.path_str(), "f1111111-1111-2222-3333-444444444444",
        "Agent alpha memory", "alpha", "", "[]", "2026-03-01 00:00:00");
    seed_memory(db.path_str(), "f2222222-1111-2222-3333-444444444444",
        "Agent beta memory", "beta", "", "[]", "2026-03-01 00:00:01");

    // Filter by agent alpha
    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "recall", "--agent-id", "alpha"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "recall --agent-id must exit 0");
    assert!(stdout.contains("Agent alpha memory"), "must show alpha's memory; got:\n{}", stdout);
    assert!(!stdout.contains("Agent beta memory"), "must NOT show beta's memory; got:\n{}", stdout);
    assert!(stdout.contains("Showing 1 of 1 memories"), "footer must reflect filtered count; got:\n{}", stdout);
}

/// RCL-03: `mnemonic recall --session-id <id>` filters results to matching session.
#[test]
fn test_recall_filter_session_id() {
    let db = TempDb::new("recall_filter_session");
    let bin = binary();

    seed_memory(db.path_str(), "f3333333-1111-2222-3333-444444444444",
        "Session X memory", "", "sess-x", "[]", "2026-03-02 00:00:00");
    seed_memory(db.path_str(), "f4444444-1111-2222-3333-444444444444",
        "Session Y memory", "", "sess-y", "[]", "2026-03-02 00:00:01");

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "recall", "--session-id", "sess-x"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "recall --session-id must exit 0");
    assert!(stdout.contains("Session X memory"), "must show session X memory; got:\n{}", stdout);
    assert!(!stdout.contains("Session Y memory"), "must NOT show session Y memory; got:\n{}", stdout);
}

/// RCL-03: `mnemonic recall --limit N` limits output to N rows.
#[test]
fn test_recall_limit() {
    let db = TempDb::new("recall_limit");
    let bin = binary();

    // Seed 3 memories
    seed_memory(db.path_str(), "f5555555-1111-2222-3333-444444444444",
        "Memory one", "", "", "[]", "2026-03-03 00:00:00");
    seed_memory(db.path_str(), "f6666666-1111-2222-3333-444444444444",
        "Memory two", "", "", "[]", "2026-03-03 00:00:01");
    seed_memory(db.path_str(), "f7777777-1111-2222-3333-444444444444",
        "Memory three", "", "", "[]", "2026-03-03 00:00:02");

    // Request only 2
    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "recall", "--limit", "2"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "recall --limit must exit 0");
    assert!(
        stdout.contains("Showing 2 of 3 memories"),
        "footer must show 'Showing 2 of 3 memories'; got:\n{}",
        stdout
    );
}

/// Phase 16: `mnemonic --help` lists `recall` as a subcommand.
#[test]
fn test_recall_appears_in_help() {
    let bin = binary();
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "--help must exit 0");
    assert!(
        stdout.contains("recall"),
        "--help must list 'recall' subcommand; got:\n{}",
        stdout
    );
}

// ---- Phase 17: remember subcommand ------------------------------------------------

/// REM-01: `mnemonic remember 'content'` stores a memory and prints a UUID to stdout.
#[test]
fn test_remember_stores_memory_and_prints_uuid() {
    let db = TempDb::new("remember_basic");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "remember", "Hello world from CLI"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "remember must exit 0; stderr: {}",
        stderr
    );

    // D-14: stdout line 1 must be a full UUID (36 chars)
    let uuid_line = stdout.trim().lines().next().unwrap_or("");
    assert_eq!(
        uuid_line.len(), 36,
        "stdout line 1 must be a 36-char UUID; got: {:?}",
        uuid_line
    );
    assert!(
        uuid_line.contains('-'),
        "UUID must contain dashes; got: {:?}",
        uuid_line
    );

    // D-15: stderr must contain "Stored memory" confirmation
    assert!(
        stderr.contains("Stored memory"),
        "stderr must contain 'Stored memory'; got: {:?}",
        stderr
    );
}

/// REM-02: `echo 'content' | mnemonic remember` reads stdin and stores a memory identically.
#[test]
fn test_remember_stdin_pipe_stores_memory() {
    use std::process::Stdio;
    use std::io::Write;

    let db = TempDb::new("remember_stdin");
    let bin = binary();

    let mut child = Command::new(&bin)
        .args(["--db", db.path_str(), "remember"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn mnemonic binary");

    // Write content to stdin then close it (triggers EOF)
    child.stdin.take().unwrap().write_all(b"Content from stdin pipe").unwrap();
    let output = child.wait_with_output().expect("failed to wait for output");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "remember (stdin) must exit 0; stderr: {}",
        stderr
    );

    // UUID on stdout
    let uuid_line = stdout.trim().lines().next().unwrap_or("");
    assert_eq!(
        uuid_line.len(), 36,
        "stdin remember: stdout line 1 must be a 36-char UUID; got: {:?}",
        uuid_line
    );

    // Confirmation on stderr
    assert!(
        stderr.contains("Stored memory"),
        "stdin remember: stderr must contain 'Stored memory'; got: {:?}",
        stderr
    );
}

/// REM-01: `mnemonic remember ''` exits 1 with error on stderr (D-16/D-17).
#[test]
fn test_remember_empty_content_exits_one() {
    let db = TempDb::new("remember_empty");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "remember", ""])
        .output()
        .expect("failed to run mnemonic binary");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "remember with empty content must exit non-zero"
    );
    assert!(
        stderr.contains("content must not be empty"),
        "stderr must contain 'content must not be empty'; got: {:?}",
        stderr
    );
}

/// REM-01: `mnemonic remember '   '` exits 1 with error (whitespace-only, D-16/D-17).
#[test]
fn test_remember_whitespace_only_content_exits_one() {
    let db = TempDb::new("remember_whitespace");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "remember", "   "])
        .output()
        .expect("failed to run mnemonic binary");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "remember with whitespace-only content must exit non-zero"
    );
    assert!(
        stderr.contains("content must not be empty"),
        "stderr must contain 'content must not be empty'; got: {:?}",
        stderr
    );
}

/// Phase 17: `mnemonic --help` lists `remember` as a subcommand.
#[test]
fn test_remember_appears_in_help() {
    let bin = binary();
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "--help must exit 0");
    assert!(
        stdout.contains("remember"),
        "--help must list 'remember' subcommand; got:\n{}",
        stdout
    );
}

/// REM-03: `mnemonic remember ... --agent-id ... --session-id ...` stores metadata correctly.
/// Verified via `recall --id` to confirm persisted values.
#[test]
fn test_remember_with_agent_and_session_id() {
    let db = TempDb::new("remember_metadata");
    let bin = binary();

    // Store a memory with metadata flags
    let output = Command::new(&bin)
        .args([
            "--db", db.path_str(),
            "remember", "Memory with metadata",
            "--agent-id", "test-agent",
            "--session-id", "test-session",
        ])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "remember with metadata must exit 0; stderr: {}",
        stderr
    );

    // Extract the UUID from stdout
    let uuid = stdout.trim().lines().next().unwrap_or("").to_string();
    assert_eq!(uuid.len(), 36, "must get a valid UUID; got: {:?}", uuid);

    // Verify via recall --id
    let recall = Command::new(&bin)
        .args(["--db", db.path_str(), "recall", "--id", &uuid])
        .output()
        .expect("failed to run recall");

    let recall_stdout = String::from_utf8_lossy(&recall.stdout);
    assert!(recall.status.success(), "recall --id must exit 0");
    assert!(
        recall_stdout.contains("test-agent"),
        "recall must show agent_id 'test-agent'; got:\n{}",
        recall_stdout
    );
    assert!(
        recall_stdout.contains("test-session"),
        "recall must show session_id 'test-session'; got:\n{}",
        recall_stdout
    );
    assert!(
        recall_stdout.contains("Memory with metadata"),
        "recall must show the content; got:\n{}",
        recall_stdout
    );
}

/// REM-04: `mnemonic remember ... --tags 'work,important, review'` stores trimmed tags correctly.
/// Verified via `recall --id` to confirm persisted and trimmed values.
#[test]
fn test_remember_with_tags() {
    let db = TempDb::new("remember_tags");
    let bin = binary();

    // Store a memory with tags
    let output = Command::new(&bin)
        .args([
            "--db", db.path_str(),
            "remember", "Tagged memory content",
            "--tags", "work,important, review",
        ])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "remember with tags must exit 0; stderr: {}",
        stderr
    );

    // Extract UUID
    let uuid = stdout.trim().lines().next().unwrap_or("").to_string();
    assert_eq!(uuid.len(), 36, "must get a valid UUID; got: {:?}", uuid);

    // Verify via recall --id
    let recall = Command::new(&bin)
        .args(["--db", db.path_str(), "recall", "--id", &uuid])
        .output()
        .expect("failed to run recall");

    let recall_stdout = String::from_utf8_lossy(&recall.stdout);
    assert!(recall.status.success(), "recall --id must exit 0");

    // D-11: Tags should be trimmed and stored correctly
    assert!(
        recall_stdout.contains("work"),
        "recall must show tag 'work'; got:\n{}",
        recall_stdout
    );
    assert!(
        recall_stdout.contains("important"),
        "recall must show tag 'important'; got:\n{}",
        recall_stdout
    );
    assert!(
        recall_stdout.contains("review"),
        "recall must show tag 'review' (trimmed from ' review'); got:\n{}",
        recall_stdout
    );
}

// ---- Phase 18: search subcommand ------------------------------------------------

/// SRC-01: `mnemonic search 'query'` returns a table with DIST/ID/CONTENT/AGENT headers and footer.
/// Seeds one memory via `remember`, then searches for it semantically.
/// Note: This test takes ~4-6s (model load for remember + model load for search).
#[test]
fn test_search_returns_ranked_results() {
    let db = TempDb::new("search_basic");
    let bin = binary();

    // Seed a memory
    let seed = Command::new(&bin)
        .args(["--db", db.path_str(), "remember", "Paris is the capital of France"])
        .output()
        .expect("failed to run mnemonic binary");
    assert!(
        seed.status.success(),
        "remember must succeed before search test; stderr: {}",
        String::from_utf8_lossy(&seed.stderr)
    );

    // Search for it semantically
    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "search", "French capital city"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "search must exit 0; stderr: {}",
        stderr
    );
    assert!(stdout.contains("DIST"), "stdout must contain 'DIST' column header; got:\n{}", stdout);
    assert!(stdout.contains("ID"), "stdout must contain 'ID' column header; got:\n{}", stdout);
    assert!(stdout.contains("CONTENT"), "stdout must contain 'CONTENT' column header; got:\n{}", stdout);
    assert!(stdout.contains("AGENT"), "stdout must contain 'AGENT' column header; got:\n{}", stdout);
    assert!(stdout.contains("Found"), "stdout must contain 'Found' footer; got:\n{}", stdout);
    assert!(stdout.contains("Paris"), "stdout must contain 'Paris' from memory content; got:\n{}", stdout);
}

/// SRC-01: `mnemonic search ''` exits 1 with "query must not be empty" on stderr.
/// This test is FAST (no model load due to early validation per D-04).
#[test]
fn test_search_empty_query_exits_one() {
    let db = TempDb::new("search_empty_query");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "search", ""])
        .output()
        .expect("failed to run mnemonic binary");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "search with empty query must exit non-zero"
    );
    assert!(
        stderr.contains("query must not be empty"),
        "stderr must contain 'query must not be empty'; got: {:?}",
        stderr
    );
}

/// SRC-01: `mnemonic search '   '` exits 1 with "query must not be empty" on stderr.
/// This test is FAST (no model load).
#[test]
fn test_search_whitespace_query_exits_one() {
    let db = TempDb::new("search_whitespace_query");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "search", "   "])
        .output()
        .expect("failed to run mnemonic binary");

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "search with whitespace-only query must exit non-zero"
    );
    assert!(
        stderr.contains("query must not be empty"),
        "stderr must contain 'query must not be empty'; got: {:?}",
        stderr
    );
}

/// SRC-01: `mnemonic search 'something that does not exist'` on empty DB exits 0
/// and prints "No matching memories found." (empty results is success, not error).
/// Note: This test takes ~2-3s (model load for search even though no results).
#[test]
fn test_search_no_results_message() {
    let db = TempDb::new("search_no_results");
    let bin = binary();

    // Do NOT seed any memories
    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "search", "something that does not exist"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "search with no results must exit 0; stderr: {}",
        stderr
    );
    assert!(
        stdout.contains("No matching memories found."),
        "stdout must contain 'No matching memories found.'; got:\n{}",
        stdout
    );
}

/// SRC-01: `mnemonic --help` lists `search` as a subcommand.
/// This test is FAST (no model load).
#[test]
fn test_search_appears_in_help() {
    let bin = binary();

    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "--help must exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("search"),
        "--help must list 'search' subcommand; got:\n{}",
        stdout
    );
}

/// SRC-02: `mnemonic search 'query' --limit N` caps results at N.
/// Seeds 3 memories, searches with --limit 2, asserts "Found 2 results".
/// Note: This test takes ~10-12s (3 remember + 1 search model loads).
#[test]
fn test_search_limit_flag() {
    let db = TempDb::new("search_limit");
    let bin = binary();

    // Seed 3 memories with distinct Paris landmarks
    for content in &[
        "The Eiffel Tower is in Paris France",
        "The Louvre Museum is in Paris France",
        "Notre Dame Cathedral is in Paris France",
    ] {
        let seed = Command::new(&bin)
            .args(["--db", db.path_str(), "remember", content])
            .output()
            .expect("failed to run mnemonic binary");
        assert!(
            seed.status.success(),
            "remember must succeed; stderr: {}",
            String::from_utf8_lossy(&seed.stderr)
        );
    }

    // Search with limit 2 — should return exactly 2 results
    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "search", "Paris landmarks", "--limit", "2"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "search --limit must exit 0; stderr: {}",
        stderr
    );
    assert!(
        stdout.contains("Found 2 results"),
        "footer must say 'Found 2 results' (not 3); got:\n{}",
        stdout
    );
}

/// SRC-02: `mnemonic search 'query' --agent-id x` filters results to matching agent.
/// Seeds 2 memories (one with agent-jp, one without), searches with --agent-id agent-jp.
/// Note: This test takes ~8-10s (2 remember + 1 search model loads).
#[test]
fn test_search_agent_id_filter() {
    let db = TempDb::new("search_agent_filter");
    let bin = binary();

    // Seed 1 memory with agent-id
    let seed_agent = Command::new(&bin)
        .args([
            "--db", db.path_str(),
            "remember", "Tokyo is the capital of Japan",
            "--agent-id", "agent-jp",
        ])
        .output()
        .expect("failed to run mnemonic binary");
    assert!(
        seed_agent.status.success(),
        "remember with agent-id must succeed; stderr: {}",
        String::from_utf8_lossy(&seed_agent.stderr)
    );

    // Seed 1 memory without agent-id
    let seed_no_agent = Command::new(&bin)
        .args(["--db", db.path_str(), "remember", "Berlin is the capital of Germany"])
        .output()
        .expect("failed to run mnemonic binary");
    assert!(
        seed_no_agent.status.success(),
        "remember without agent-id must succeed; stderr: {}",
        String::from_utf8_lossy(&seed_no_agent.stderr)
    );

    // Search filtered to agent-jp only
    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "search", "capital city", "--agent-id", "agent-jp"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "search --agent-id must exit 0; stderr: {}",
        stderr
    );
    assert!(
        stdout.contains("Found 1 result"),
        "footer must say 'Found 1 result' (singular, only agent-jp memory); got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("Tokyo"),
        "stdout must contain 'Tokyo' (the agent-jp memory content); got:\n{}",
        stdout
    );
}

/// SRC-02: `mnemonic search 'query' --threshold 0.0001` filters out non-exact matches.
/// Seeds 1 ML memory, searches with very tight threshold for unrelated content.
/// Note: This test takes ~4-6s (1 remember + 1 search model loads).
#[test]
fn test_search_threshold_flag() {
    let db = TempDb::new("search_threshold");
    let bin = binary();

    // Seed 1 memory about machine learning
    let seed = Command::new(&bin)
        .args(["--db", db.path_str(), "remember", "Machine learning uses neural networks"])
        .output()
        .expect("failed to run mnemonic binary");
    assert!(
        seed.status.success(),
        "remember must succeed; stderr: {}",
        String::from_utf8_lossy(&seed.stderr)
    );

    // Search with very tight threshold for unrelated content — threshold filters out the non-exact match
    let output = Command::new(&bin)
        .args([
            "--db", db.path_str(),
            "search", "something completely unrelated to ML",
            "--threshold", "0.0001",
        ])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "search --threshold must exit 0; stderr: {}",
        stderr
    );
    assert!(
        stdout.contains("No matching memories found."),
        "tight threshold must filter out non-exact match, printing 'No matching memories found.'; got:\n{}",
        stdout
    );
}

// ---- CMP-01/02/03: compact subcommand (Phase 19) --------------------------------

/// CMP-01: `mnemonic --help` lists `compact` as a subcommand.
/// FAST test (no model load).
#[test]
fn test_compact_appears_in_help() {
    let bin = binary();

    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "--help must exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("compact"),
        "--help must list 'compact' subcommand; got:\n{}",
        stdout
    );
}

/// CMP-01: `mnemonic compact` on empty DB exits 0 and prints no-results message.
/// Note: Takes ~2-3s (model load even though no compaction occurs).
#[test]
fn test_compact_no_results() {
    let db = TempDb::new("compact_no_results");
    let bin = binary();

    // No seeding -- empty DB
    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "compact"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "compact on empty DB must exit 0; stderr: {}",
        stderr
    );
    assert!(
        stdout.contains("No similar memories found to compact."),
        "stdout must contain 'No similar memories found to compact.'; got:\n{}",
        stdout
    );
    // Verify audit trail on stderr
    assert!(
        stderr.contains("Run: "),
        "stderr must contain 'Run: ' audit trail; got:\n{}",
        stderr
    );
}

/// CMP-01: `mnemonic compact` on 2 similar memories exits 0 and prints "Compacted:" summary.
/// Seeds 2 semantically similar memories, runs compact with --threshold 0.7 for reliability.
/// Note: Takes ~8-10s (2 remember + 1 compact model loads).
#[test]
fn test_compact_basic() {
    let db = TempDb::new("compact_basic");
    let bin = binary();

    // Seed 2 similar memories via `mnemonic remember`
    for content in &[
        "Paris is the capital of France",
        "France's capital city is Paris",
    ] {
        let seed = Command::new(&bin)
            .args(["--db", db.path_str(), "remember", content])
            .output()
            .expect("failed to run mnemonic binary");
        assert!(
            seed.status.success(),
            "remember must succeed before compact test; stderr: {}",
            String::from_utf8_lossy(&seed.stderr)
        );
    }

    // Run compact with relaxed threshold for test reliability
    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "compact", "--threshold", "0.7"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "compact must exit 0; stderr: {}",
        stderr
    );
    assert!(
        stdout.contains("Compacted:"),
        "stdout must contain 'Compacted:' summary; got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("memories merged"),
        "stdout must contain 'memories merged'; got:\n{}",
        stdout
    );
    // Verify audit trail
    assert!(
        stderr.contains("Run: "),
        "stderr must contain 'Run: ' audit trail; got:\n{}",
        stderr
    );
}

/// CMP-02: `mnemonic compact --dry-run` exits 0 and prints "Dry run:" summary without mutating.
/// Seeds 2 similar memories, runs compact --dry-run, verifies "Dry run:" output,
/// then runs recall to confirm memories are NOT deleted.
/// Note: Takes ~12-15s (2 remember + 1 compact + 1 recall).
#[test]
fn test_compact_dry_run() {
    let db = TempDb::new("compact_dry_run");
    let bin = binary();

    // Seed 2 similar memories
    for content in &[
        "The Eiffel Tower is a famous landmark in Paris",
        "Paris is known for the Eiffel Tower landmark",
    ] {
        let seed = Command::new(&bin)
            .args(["--db", db.path_str(), "remember", content])
            .output()
            .expect("failed to run mnemonic binary");
        assert!(
            seed.status.success(),
            "remember must succeed; stderr: {}",
            String::from_utf8_lossy(&seed.stderr)
        );
    }

    // Run compact with --dry-run and relaxed threshold
    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "compact", "--dry-run", "--threshold", "0.7"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "compact --dry-run must exit 0; stderr: {}",
        stderr
    );
    assert!(
        stdout.contains("Dry run:"),
        "stdout must contain 'Dry run:' prefix; got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("would be merged"),
        "stdout must contain 'would be merged'; got:\n{}",
        stdout
    );

    // Verify data was NOT mutated -- recall should still show 2 memories
    let recall_output = Command::new(&bin)
        .args(["--db", db.path_str(), "recall"])
        .output()
        .expect("failed to run mnemonic binary");

    let recall_stdout = String::from_utf8_lossy(&recall_output.stdout);

    assert!(
        recall_output.status.success(),
        "recall after dry-run must succeed; stderr: {}",
        String::from_utf8_lossy(&recall_output.stderr)
    );
    assert!(
        recall_stdout.contains("Showing 2 of 2"),
        "recall must still show 2 memories (dry-run did not delete); got:\n{}",
        recall_stdout
    );
}

/// CMP-03: `mnemonic compact --agent-id <id>` scopes compaction to one agent namespace.
/// Seeds 2 memories for agent-fr and 1 for agent-de. Compacts agent-fr only.
/// Verifies agent-de memory is untouched.
/// Note: Takes ~12-15s (3 remember + 1 compact + 1 recall).
#[test]
fn test_compact_agent_id_flag() {
    let db = TempDb::new("compact_agent_id");
    let bin = binary();

    // Seed 2 similar memories for agent-fr
    for content in &[
        "Paris is the capital of France",
        "France's capital city is Paris",
    ] {
        let seed = Command::new(&bin)
            .args([
                "--db", db.path_str(),
                "remember", content,
                "--agent-id", "agent-fr",
            ])
            .output()
            .expect("failed to run mnemonic binary");
        assert!(
            seed.status.success(),
            "remember for agent-fr must succeed; stderr: {}",
            String::from_utf8_lossy(&seed.stderr)
        );
    }

    // Seed 1 memory for agent-de (should not be affected)
    let seed_de = Command::new(&bin)
        .args([
            "--db", db.path_str(),
            "remember", "Berlin is the capital of Germany",
            "--agent-id", "agent-de",
        ])
        .output()
        .expect("failed to run mnemonic binary");
    assert!(
        seed_de.status.success(),
        "remember for agent-de must succeed; stderr: {}",
        String::from_utf8_lossy(&seed_de.stderr)
    );

    // Compact only agent-fr namespace
    let output = Command::new(&bin)
        .args([
            "--db", db.path_str(),
            "compact",
            "--agent-id", "agent-fr",
            "--threshold", "0.7",
        ])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "compact --agent-id must exit 0; stderr: {}",
        stderr
    );
    assert!(
        stdout.contains("Compacted:"),
        "stdout must contain 'Compacted:' (agent-fr memories were merged); got:\n{}",
        stdout
    );

    // Verify agent-de memory is untouched
    let recall_de = Command::new(&bin)
        .args(["--db", db.path_str(), "recall", "--agent-id", "agent-de"])
        .output()
        .expect("failed to run mnemonic binary");

    let recall_stdout = String::from_utf8_lossy(&recall_de.stdout);

    assert!(
        recall_de.status.success(),
        "recall agent-de must succeed; stderr: {}",
        String::from_utf8_lossy(&recall_de.stderr)
    );
    assert!(
        recall_stdout.contains("Berlin"),
        "agent-de memory must still exist after compacting agent-fr; got:\n{}",
        recall_stdout
    );
}

/// CMP-03: `mnemonic compact --threshold 0.99` with 2 similar-but-not-identical memories
/// finds 0 clusters (threshold too high for non-duplicate content).
/// Note: Takes ~8-10s (2 remember + 1 compact model loads).
#[test]
fn test_compact_threshold_flag() {
    let db = TempDb::new("compact_threshold");
    let bin = binary();

    // Seed 2 similar (but not identical) memories
    for content in &[
        "The weather in London is often rainy and cool",
        "London frequently experiences rain and cold weather",
    ] {
        let seed = Command::new(&bin)
            .args(["--db", db.path_str(), "remember", content])
            .output()
            .expect("failed to run mnemonic binary");
        assert!(
            seed.status.success(),
            "remember must succeed; stderr: {}",
            String::from_utf8_lossy(&seed.stderr)
        );
    }

    // Compact with very high threshold -- should find 0 clusters
    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "compact", "--threshold", "0.99"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "compact --threshold 0.99 must exit 0; stderr: {}",
        stderr
    );
    assert!(
        stdout.contains("No similar memories found to compact."),
        "high threshold must yield 0 clusters; got:\n{}",
        stdout
    );
}

// ---- Phase 20: --json output tests -----------------------------------------------

#[test]
fn test_json_flag_appears_in_help() {
    let bin = binary();
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to run mnemonic binary");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "--help must exit 0");
    assert!(
        stdout.contains("--json"),
        "--help must show --json flag; got:\n{}",
        stdout
    );
}

#[test]
fn test_recall_json_list() {
    let db = TempDb::new("recall_json_list");
    let bin = binary();

    seed_memory(db.path_str(), "aaaaaaaa-1111-2222-3333-444444444444",
        "JSON test memory", "agent-1", "session-1", "[]", "2026-01-01 00:00:00");

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "--json", "recall"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "recall --json must exit 0; stderr: {}", String::from_utf8_lossy(&output.stderr));

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("stdout must be valid JSON");
    assert!(parsed["memories"].is_array(), "must have memories array");
    assert!(parsed["total"].is_number(), "must have total number");
    assert_eq!(parsed["memories"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["total"], 1);
    assert_eq!(parsed["memories"][0]["id"], "aaaaaaaa-1111-2222-3333-444444444444");
    assert_eq!(parsed["memories"][0]["content"], "JSON test memory");
}

#[test]
fn test_recall_json_empty() {
    let db = TempDb::new("recall_json_empty");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "--json", "recall"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "recall --json (empty) must exit 0");

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("stdout must be valid JSON even when empty");
    assert!(parsed["memories"].is_array(), "must have memories array");
    assert_eq!(parsed["memories"].as_array().unwrap().len(), 0);
    assert_eq!(parsed["total"], 0);
}

#[test]
fn test_recall_json_by_id() {
    let db = TempDb::new("recall_json_by_id");
    let bin = binary();

    seed_memory(db.path_str(), "bbbbbbbb-1111-2222-3333-444444444444",
        "Recall by ID JSON test", "agent-x", "session-y", "[\"tag1\"]", "2026-01-02 00:00:00");

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "--json", "recall", "--id", "bbbbbbbb-1111-2222-3333-444444444444"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "recall --id --json must exit 0; stderr: {}", String::from_utf8_lossy(&output.stderr));

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("stdout must be valid JSON");
    assert_eq!(parsed["id"], "bbbbbbbb-1111-2222-3333-444444444444");
    assert_eq!(parsed["content"], "Recall by ID JSON test");
    assert_eq!(parsed["agent_id"], "agent-x");
    assert_eq!(parsed["session_id"], "session-y");
    assert!(parsed["tags"].is_array(), "tags must be array");
}

#[test]
fn test_remember_json() {
    let db = TempDb::new("remember_json");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "--json", "remember", "JSON remember test"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "remember --json must exit 0; stderr: {}",
        stderr
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("stdout must be valid JSON");
    assert!(parsed["id"].is_string(), "must have id string field");
    let id = parsed["id"].as_str().unwrap();
    assert_eq!(id.len(), 36, "id must be a 36-char UUID; got: {:?}", id);

    // In JSON mode, stderr should NOT contain "Stored memory" (human context suppressed)
    // But it MAY contain model loading messages — that's fine
}

#[test]
fn test_search_json() {
    let db = TempDb::new("search_json");
    let bin = binary();

    // Seed via remember (creates embeddings needed for search)
    let seed = Command::new(&bin)
        .args(["--db", db.path_str(), "remember", "The quick brown fox jumps over the lazy dog"])
        .output()
        .expect("failed to seed memory");
    assert!(seed.status.success(), "seed remember must exit 0; stderr: {}", String::from_utf8_lossy(&seed.stderr));

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "--json", "search", "quick brown fox"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "search --json must exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("stdout must be valid JSON");
    assert!(parsed["memories"].is_array(), "must have memories array");
    let memories = parsed["memories"].as_array().unwrap();
    assert!(!memories.is_empty(), "search must return at least 1 result");
    // Each result must have distance field (from SearchResultItem)
    assert!(memories[0]["distance"].is_number(), "result must have distance field");
    assert!(memories[0]["id"].is_string(), "result must have id field");
    assert!(memories[0]["content"].is_string(), "result must have content field");
}

#[test]
fn test_compact_json() {
    let db = TempDb::new("compact_json");
    let bin = binary();

    // Seed two very similar memories for compaction
    let seed1 = Command::new(&bin)
        .args(["--db", db.path_str(), "remember", "The weather today is sunny and warm"])
        .output()
        .expect("failed to seed memory 1");
    assert!(seed1.status.success(), "seed 1 must exit 0");

    let seed2 = Command::new(&bin)
        .args(["--db", db.path_str(), "remember", "Today the weather is warm and sunny"])
        .output()
        .expect("failed to seed memory 2");
    assert!(seed2.status.success(), "seed 2 must exit 0");

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "--json", "compact", "--threshold", "0.7"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "compact --json must exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("stdout must be valid JSON");
    assert!(parsed["run_id"].is_string(), "must have run_id string");
    assert!(parsed["clusters_found"].is_number(), "must have clusters_found number");
    assert!(parsed["memories_merged"].is_number(), "must have memories_merged number");
    assert!(parsed["memories_created"].is_number(), "must have memories_created number");
    assert!(parsed["id_mapping"].is_array(), "must have id_mapping array");
    assert!(parsed["truncated"].is_boolean(), "must have truncated boolean");
}

#[test]
fn test_keys_create_json() {
    let db = TempDb::new("keys_create_json");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "--json", "keys", "create", "json-test-key", "--agent-id", "agent-j"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "keys create --json must exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("stdout must be valid JSON");
    assert!(parsed["token"].is_string(), "must have token string");
    let token = parsed["token"].as_str().unwrap();
    assert!(token.starts_with("mnk_"), "token must start with mnk_; got: {:?}", token);
    assert_eq!(token.len(), 68, "token must be 68 chars");
    assert!(parsed["id"].is_string(), "must have id string (display_id)");
    assert_eq!(parsed["name"], "json-test-key");
    assert_eq!(parsed["scope"], "agent-j");
}

#[test]
fn test_keys_list_json() {
    let db = TempDb::new("keys_list_json");
    let bin = binary();

    // Create a key first
    let create = Command::new(&bin)
        .args(["--db", db.path_str(), "keys", "create", "list-json-key"])
        .output()
        .expect("failed to create key");
    assert!(create.status.success(), "key create must exit 0");

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "--json", "keys", "list"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "keys list --json must exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("stdout must be valid JSON");
    assert!(parsed.is_array(), "keys list --json must output a JSON array");
    let keys = parsed.as_array().unwrap();
    assert_eq!(keys.len(), 1, "must have 1 key");
    assert!(keys[0]["id"].is_string(), "key must have id field");
    assert_eq!(keys[0]["name"], "list-json-key");
    assert!(keys[0]["display_id"].is_string(), "key must have display_id field");
    assert!(keys[0]["created_at"].is_string(), "key must have created_at field");
}

#[test]
fn test_keys_list_json_empty() {
    let db = TempDb::new("keys_list_json_empty");
    let bin = binary();

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "--json", "keys", "list"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "keys list --json (empty) must exit 0");

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("stdout must be valid JSON even when empty");
    assert!(parsed.is_array(), "must be a JSON array");
    assert_eq!(parsed.as_array().unwrap().len(), 0, "must be empty array");
}

#[test]
fn test_json_flag_no_human_output() {
    let db = TempDb::new("json_no_human");
    let bin = binary();

    seed_memory(db.path_str(), "cccccccc-1111-2222-3333-444444444444",
        "No human output test", "", "", "[]", "2026-01-01 00:00:00");

    let output = Command::new(&bin)
        .args(["--db", db.path_str(), "--json", "recall"])
        .output()
        .expect("failed to run mnemonic binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());

    // JSON mode must NOT contain table headers or footer
    assert!(!stdout.contains("ID"), "JSON mode must not contain table header 'ID'");
    assert!(!stdout.contains("CONTENT"), "JSON mode must not contain table header 'CONTENT'");
    assert!(!stdout.contains("Showing"), "JSON mode must not contain footer 'Showing'");

    // But must be valid JSON
    let _: serde_json::Value = serde_json::from_str(&stdout)
        .expect("stdout must be valid JSON");
}
