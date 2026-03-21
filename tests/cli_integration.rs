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
