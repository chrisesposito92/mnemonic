use rusqlite::ffi::sqlite3_auto_extension;
use sqlite_vec::sqlite3_vec_init;
use std::sync::Once;

static SQLITE_VEC_REGISTERED: Once = Once::new();

/// Registers the sqlite-vec extension with SQLite's global auto-extension list.
///
/// Must be called exactly once at process startup, before any `Connection::open` call.
/// The `Once` guard prevents double-registration which would cause initialization errors.
pub fn register_sqlite_vec() {
    SQLITE_VEC_REGISTERED.call_once(|| {
        unsafe {
            sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite3_vec_init as *const (),
            )));
        }
    });
}

/// Opens a SQLite database connection and initializes the schema.
///
/// Applies WAL journal mode, creates the `memories` table with all required columns,
/// three indexes, and the `vec_memories` virtual table for vector search.
///
/// All SQL executes inside `conn.call()` — no direct rusqlite calls from async context.
pub async fn open(
    config: &crate::config::Config,
) -> Result<tokio_rusqlite::Connection, crate::error::DbError> {
    let conn = tokio_rusqlite::Connection::open(&config.db_path)
        .await
        .map_err(|e| crate::error::DbError::Open(format!("{}", e)))?;

    conn.call(|c| -> Result<(), rusqlite::Error> {
        c.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;

            CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                agent_id TEXT NOT NULL DEFAULT '',
                session_id TEXT NOT NULL DEFAULT '',
                tags TEXT NOT NULL DEFAULT '[]',
                embedding_model TEXT NOT NULL DEFAULT '',
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME
            );

            CREATE INDEX IF NOT EXISTS idx_memories_agent_id ON memories(agent_id);
            CREATE INDEX IF NOT EXISTS idx_memories_session_id ON memories(session_id);
            CREATE INDEX IF NOT EXISTS idx_memories_created_at ON memories(created_at);

            CREATE VIRTUAL TABLE IF NOT EXISTS vec_memories USING vec0(
                memory_id TEXT PRIMARY KEY,
                embedding float[384]
            );

            -- v1.1: compaction audit log
            CREATE TABLE IF NOT EXISTS compact_runs (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                completed_at DATETIME,
                clusters_found INTEGER NOT NULL DEFAULT 0,
                memories_merged INTEGER NOT NULL DEFAULT 0,
                memories_created INTEGER NOT NULL DEFAULT 0,
                dry_run BOOLEAN NOT NULL DEFAULT 0,
                threshold REAL NOT NULL,
                status TEXT NOT NULL DEFAULT 'running'
            );

            CREATE INDEX IF NOT EXISTS idx_compact_runs_agent_id ON compact_runs(agent_id);
            ",
        )?;

        // v1.1: Add source_ids column to memories table.
        // SQLite does not support ALTER TABLE ADD COLUMN IF NOT EXISTS, so we
        // attempt the migration and ignore "duplicate column name" — the only
        // error that can occur when the column already exists.
        match c.execute_batch(
            "ALTER TABLE memories ADD COLUMN source_ids TEXT NOT NULL DEFAULT '[]';",
        ) {
            Ok(()) => {}
            Err(rusqlite::Error::SqliteFailure(ref err, _))
                if err.extended_code == 1 => {}  // SQLITE_ERROR: duplicate column name
            Err(e) => return Err(e),
        }

        Ok(())
    })
    .await
    .map_err(|e| crate::error::DbError::Schema(format!("{}", e)))?;

    Ok(conn)
}
