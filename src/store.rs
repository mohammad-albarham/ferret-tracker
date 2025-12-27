//! SQLite storage layer for Ferret
//!
//! This module handles all database operations including schema management,
//! event insertion, querying, and statistics generation.

use crate::models::{EventFilter, EventStats, FileEvent, FileType};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

/// Database schema version for migrations
const SCHEMA_VERSION: i32 = 1;

/// The file event store backed by SQLite
pub struct Store {
    /// Connection wrapped in Arc<Mutex> for thread-safe access
    conn: Arc<Mutex<Connection>>,
    /// Path to the database file
    db_path: PathBuf,
}

impl Store {
    /// Create a new Store, initializing the database if needed
    pub fn new(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create database directory: {}", parent.display()))?;
        }

        let conn = Connection::open(db_path)
            .with_context(|| format!("Failed to open database: {}", db_path.display()))?;

        // Configure SQLite for high-concurrency access
        // These pragmas are critical for preventing "database is locked" errors
        conn.execute_batch("
            -- WAL mode allows concurrent readers and one writer
            PRAGMA journal_mode=WAL;
            
            -- Wait up to 5 seconds for locks instead of failing immediately
            PRAGMA busy_timeout=5000;
            
            -- NORMAL is safe with WAL and much faster than FULL
            PRAGMA synchronous=NORMAL;
            
            -- Enable foreign keys
            PRAGMA foreign_keys=ON;
            
            -- Use memory for temp storage (faster)
            PRAGMA temp_store=MEMORY;
            
            -- Larger cache for better read performance
            PRAGMA cache_size=-64000;
            
            -- Enable memory-mapped I/O (256MB)
            PRAGMA mmap_size=268435456;
        ")?;

        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: db_path.to_path_buf(),
        };

        store.initialize_schema()?;
        
        info!("Database initialized at {}", db_path.display());
        Ok(store)
    }

    /// Create an in-memory store (useful for testing or fallback)
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("Failed to create in-memory database")?;

        // Configure for performance (less strict for in-memory)
        conn.execute_batch("
            PRAGMA busy_timeout=5000;
            PRAGMA synchronous=OFF;
            PRAGMA foreign_keys=ON;
            PRAGMA temp_store=MEMORY;
        ")?;

        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: PathBuf::from(":memory:"),
        };

        store.initialize_schema()?;
        
        debug!("In-memory database initialized");
        Ok(store)
    }

    /// Initialize database schema
    fn initialize_schema(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

        // Create schema version table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            )",
            [],
        )?;

        // Check current schema version
        let current_version: Option<i32> = conn
            .query_row(
                "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;

        let version = current_version.unwrap_or(0);

        if version < SCHEMA_VERSION {
            self.migrate_schema(&conn, version)?;
        }

        Ok(())
    }

    /// Run schema migrations
    fn migrate_schema(&self, conn: &Connection, from_version: i32) -> Result<()> {
        info!("Migrating database schema from v{} to v{}", from_version, SCHEMA_VERSION);

        if from_version < 1 {
            // Initial schema
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    path TEXT NOT NULL UNIQUE,
                    dir TEXT NOT NULL,
                    filename TEXT NOT NULL,
                    size_bytes INTEGER,
                    created_at TEXT NOT NULL,
                    file_type TEXT NOT NULL,
                    tags TEXT DEFAULT '',
                    notes TEXT DEFAULT ''
                );

                CREATE INDEX IF NOT EXISTS idx_events_created_at ON events(created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_events_dir ON events(dir);
                CREATE INDEX IF NOT EXISTS idx_events_file_type ON events(file_type);
                CREATE INDEX IF NOT EXISTS idx_events_filename ON events(filename);
                ",
            )?;
        }

        // Record the new version
        conn.execute(
            "INSERT OR REPLACE INTO schema_version (version) VALUES (?)",
            params![SCHEMA_VERSION],
        )?;

        info!("Schema migration complete");
        Ok(())
    }

    /// Insert a new file event (or update if path already exists)
    pub fn insert_event(&self, event: &FileEvent) -> Result<i64> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

        // Try to insert, or update size if the path already exists
        conn.execute(
            "INSERT INTO events (path, dir, filename, size_bytes, created_at, file_type, tags, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(path) DO UPDATE SET
                size_bytes = COALESCE(excluded.size_bytes, size_bytes)",
            params![
                event.path.to_string_lossy(),
                event.dir.to_string_lossy(),
                event.filename,
                event.size_bytes.map(|s| s as i64),
                event.created_at.to_rfc3339(),
                event.file_type.as_str(),
                event.tags,
                event.notes,
            ],
        )?;

        let id = conn.last_insert_rowid();
        debug!("Inserted event for {}: id={}", event.path.display(), id);
        Ok(id)
    }

    /// Get an event by ID
    pub fn get_event(&self, id: i64) -> Result<Option<FileEvent>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

        let result = conn
            .query_row(
                "SELECT id, path, dir, filename, size_bytes, created_at, file_type, tags, notes
                 FROM events WHERE id = ?",
                params![id],
                |row| self.row_to_event(row),
            )
            .optional()?;

        Ok(result)
    }

    /// Get an event by path
    pub fn get_event_by_path(&self, path: &Path) -> Result<Option<FileEvent>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

        let result = conn
            .query_row(
                "SELECT id, path, dir, filename, size_bytes, created_at, file_type, tags, notes
                 FROM events WHERE path = ?",
                params![path.to_string_lossy()],
                |row| self.row_to_event(row),
            )
            .optional()?;

        Ok(result)
    }

    /// Query events with optional filtering
    pub fn query_events(&self, filter: &EventFilter) -> Result<Vec<FileEvent>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

        let mut sql = String::from(
            "SELECT id, path, dir, filename, size_bytes, created_at, file_type, tags, notes
             FROM events WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ft) = &filter.file_type {
            sql.push_str(" AND file_type = ?");
            params.push(Box::new(ft.as_str().to_string()));
        }

        if let Some(min) = filter.min_size {
            sql.push_str(" AND size_bytes >= ?");
            params.push(Box::new(min as i64));
        }

        if let Some(max) = filter.max_size {
            sql.push_str(" AND size_bytes <= ?");
            params.push(Box::new(max as i64));
        }

        if let Some(pattern) = &filter.path_contains {
            sql.push_str(" AND path LIKE ?");
            params.push(Box::new(format!("%{}%", pattern)));
        }

        if let Some(since) = &filter.since {
            sql.push_str(" AND created_at >= ?");
            params.push(Box::new(since.to_rfc3339()));
        }

        if let Some(until) = &filter.until {
            sql.push_str(" AND created_at <= ?");
            params.push(Box::new(until.to_rfc3339()));
        }

        if let Some(dir) = &filter.dir {
            sql.push_str(" AND dir = ?");
            params.push(Box::new(dir.to_string_lossy().to_string()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        // Always use LIMIT and OFFSET for pagination
        sql.push_str(&format!(" LIMIT {} OFFSET {}", filter.limit, filter.offset));

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        
        let mut stmt = conn.prepare(&sql)?;
        let events = stmt
            .query_map(params_refs.as_slice(), |row| self.row_to_event(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(events)
    }

    /// Count events matching filter (for pagination info)
    pub fn count_filtered_events(&self, filter: &EventFilter) -> Result<usize> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

        let mut sql = String::from("SELECT COUNT(*) FROM events WHERE 1=1");
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ft) = &filter.file_type {
            sql.push_str(" AND file_type = ?");
            params.push(Box::new(ft.as_str().to_string()));
        }

        if let Some(min) = filter.min_size {
            sql.push_str(" AND size_bytes >= ?");
            params.push(Box::new(min as i64));
        }

        if let Some(max) = filter.max_size {
            sql.push_str(" AND size_bytes <= ?");
            params.push(Box::new(max as i64));
        }

        if let Some(pattern) = &filter.path_contains {
            sql.push_str(" AND path LIKE ?");
            params.push(Box::new(format!("%{}%", pattern)));
        }

        if let Some(since) = &filter.since {
            sql.push_str(" AND created_at >= ?");
            params.push(Box::new(since.to_rfc3339()));
        }

        if let Some(until) = &filter.until {
            sql.push_str(" AND created_at <= ?");
            params.push(Box::new(until.to_rfc3339()));
        }

        if let Some(dir) = &filter.dir {
            sql.push_str(" AND dir = ?");
            params.push(Box::new(dir.to_string_lossy().to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        
        let count: i64 = conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Get recent events (convenience method)
    pub fn get_recent_events(&self, limit: usize) -> Result<Vec<FileEvent>> {
        self.query_events(&EventFilter::new().with_limit(limit))
    }

    /// Update tags for an event
    pub fn update_tags(&self, id: i64, tags: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

        conn.execute(
            "UPDATE events SET tags = ? WHERE id = ?",
            params![tags, id],
        )?;

        debug!("Updated tags for event {}", id);
        Ok(())
    }

    /// Update notes for an event
    pub fn update_notes(&self, id: i64, notes: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

        conn.execute(
            "UPDATE events SET notes = ? WHERE id = ?",
            params![notes, id],
        )?;

        debug!("Updated notes for event {}", id);
        Ok(())
    }

    /// Delete an event by ID
    pub fn delete_event(&self, id: i64) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

        let rows = conn.execute("DELETE FROM events WHERE id = ?", params![id])?;

        if rows > 0 {
            debug!("Deleted event {}", id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Delete events older than a given number of days
    pub fn cleanup_old_events(&self, retention_days: u32) -> Result<usize> {
        if retention_days == 0 {
            return Ok(0);
        }

        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        let cutoff = Utc::now() - Duration::days(retention_days as i64);

        let rows = conn.execute(
            "DELETE FROM events WHERE created_at < ?",
            params![cutoff.to_rfc3339()],
        )?;

        if rows > 0 {
            info!("Cleaned up {} events older than {} days", rows, retention_days);
        }

        Ok(rows)
    }

    /// Get statistics about tracked events
    pub fn get_stats(&self) -> Result<EventStats> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

        let mut stats = EventStats::default();

        // Total count and size
        let (total_count, total_size): (i64, Option<i64>) = conn.query_row(
            "SELECT COUNT(*), SUM(size_bytes) FROM events",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        stats.total_count = total_count as u64;
        stats.total_size = total_size.unwrap_or(0) as u64;

        // Stats for time periods
        let periods = [
            (Duration::hours(24), &mut stats.count_24h, &mut stats.size_24h),
            (Duration::days(7), &mut stats.count_7d, &mut stats.size_7d),
            (Duration::days(30), &mut stats.count_30d, &mut stats.size_30d),
        ];

        for (duration, count, size) in periods {
            let since = Utc::now() - duration;
            let (c, s): (i64, Option<i64>) = conn.query_row(
                "SELECT COUNT(*), SUM(size_bytes) FROM events WHERE created_at >= ?",
                params![since.to_rfc3339()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            *count = c as u64;
            *size = s.unwrap_or(0) as u64;
        }

        // Breakdown by file type
        let mut stmt = conn.prepare(
            "SELECT file_type, COUNT(*), COALESCE(SUM(size_bytes), 0)
             FROM events GROUP BY file_type ORDER BY COUNT(*) DESC",
        )?;
        let type_rows = stmt.query_map([], |row| {
            let type_str: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            let size: i64 = row.get(2)?;
            Ok((type_str, count as u64, size as u64))
        })?;

        for row in type_rows {
            if let Ok((type_str, count, size)) = row {
                if let Ok(file_type) = type_str.parse::<FileType>() {
                    stats.by_type.push((file_type, count, size));
                }
            }
        }

        // Top directories by volume
        let mut stmt = conn.prepare(
            "SELECT dir, COUNT(*), COALESCE(SUM(size_bytes), 0)
             FROM events GROUP BY dir ORDER BY SUM(size_bytes) DESC LIMIT 10",
        )?;
        let dir_rows = stmt.query_map([], |row| {
            let dir: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            let size: i64 = row.get(2)?;
            Ok((PathBuf::from(dir), count as u64, size as u64))
        })?;

        for row in dir_rows {
            if let Ok((dir, count, size)) = row {
                stats.top_dirs.push((dir, count, size));
            }
        }

        Ok(stats)
    }

    /// Get total event count
    pub fn count_events(&self) -> Result<u64> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    /// Check if a path already exists in the database
    pub fn path_exists(&self, path: &Path) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;

        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM events WHERE path = ?)",
            params![path.to_string_lossy()],
            |row| row.get(0),
        )?;

        Ok(exists)
    }

    /// Helper to convert a database row to FileEvent
    fn row_to_event(&self, row: &rusqlite::Row) -> rusqlite::Result<FileEvent> {
        let id: i64 = row.get(0)?;
        let path: String = row.get(1)?;
        let dir: String = row.get(2)?;
        let filename: String = row.get(3)?;
        let size_bytes: Option<i64> = row.get(4)?;
        let created_at: String = row.get(5)?;
        let file_type: String = row.get(6)?;
        let tags: String = row.get(7)?;
        let notes: String = row.get(8)?;

        let created_at = DateTime::parse_from_rfc3339(&created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let file_type = file_type.parse().unwrap_or(FileType::Other);

        Ok(FileEvent {
            id: Some(id),
            path: PathBuf::from(path),
            dir: PathBuf::from(dir),
            filename,
            size_bytes: size_bytes.map(|s| s as u64),
            created_at,
            file_type,
            tags,
            notes,
        })
    }

    /// Get database path
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Clone the connection for multi-threaded access
    pub fn clone_connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }
}

impl Clone for Store {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            db_path: self.db_path.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(path: &str) -> FileEvent {
        FileEvent {
            id: None,
            path: PathBuf::from(path),
            dir: PathBuf::from("/test"),
            filename: path.split('/').last().unwrap_or("test").to_string(),
            size_bytes: Some(1024),
            created_at: Utc::now(),
            file_type: FileType::Document,
            tags: String::new(),
            notes: String::new(),
        }
    }

    #[test]
    fn test_insert_and_get_event() {
        let store = Store::in_memory().unwrap();
        let event = create_test_event("/test/file.txt");

        let id = store.insert_event(&event).unwrap();
        assert!(id > 0);

        let retrieved = store.get_event(id).unwrap();
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.filename, "file.txt");
        assert_eq!(retrieved.size_bytes, Some(1024));
    }

    #[test]
    fn test_get_event_by_path() {
        let store = Store::in_memory().unwrap();
        let event = create_test_event("/test/document.pdf");

        store.insert_event(&event).unwrap();

        let retrieved = store.get_event_by_path(Path::new("/test/document.pdf")).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().file_type, FileType::Document);
    }

    #[test]
    fn test_query_with_filter() {
        let store = Store::in_memory().unwrap();

        // Insert events of different types
        store.insert_event(&{
            let mut e = create_test_event("/test/doc.pdf");
            e.file_type = FileType::Document;
            e.size_bytes = Some(500);
            e
        }).unwrap();

        store.insert_event(&{
            let mut e = create_test_event("/test/code.rs");
            e.file_type = FileType::Code;
            e.size_bytes = Some(2000);
            e
        }).unwrap();

        // Filter by type
        let docs = store.query_events(&EventFilter::new().with_type(FileType::Document)).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].file_type, FileType::Document);

        // Filter by size
        let large = store.query_events(&EventFilter::new().with_min_size(1000)).unwrap();
        assert_eq!(large.len(), 1);
        assert_eq!(large[0].file_type, FileType::Code);
    }

    #[test]
    fn test_update_tags_and_notes() {
        let store = Store::in_memory().unwrap();
        let event = create_test_event("/test/file.txt");

        let id = store.insert_event(&event).unwrap();

        store.update_tags(id, "important, backup").unwrap();
        store.update_notes(id, "This is a test note").unwrap();

        let retrieved = store.get_event(id).unwrap().unwrap();
        assert_eq!(retrieved.tags, "important, backup");
        assert_eq!(retrieved.notes, "This is a test note");
    }

    #[test]
    fn test_delete_event() {
        let store = Store::in_memory().unwrap();
        let event = create_test_event("/test/file.txt");

        let id = store.insert_event(&event).unwrap();
        assert!(store.get_event(id).unwrap().is_some());

        let deleted = store.delete_event(id).unwrap();
        assert!(deleted);

        assert!(store.get_event(id).unwrap().is_none());
    }

    #[test]
    fn test_stats() {
        let store = Store::in_memory().unwrap();

        store.insert_event(&{
            let mut e = create_test_event("/test/a.txt");
            e.size_bytes = Some(1000);
            e.file_type = FileType::Document;
            e
        }).unwrap();

        store.insert_event(&{
            let mut e = create_test_event("/test/b.rs");
            e.size_bytes = Some(2000);
            e.file_type = FileType::Code;
            e
        }).unwrap();

        let stats = store.get_stats().unwrap();
        assert_eq!(stats.total_count, 2);
        assert_eq!(stats.total_size, 3000);
        assert_eq!(stats.count_24h, 2);
    }

    #[test]
    fn test_upsert_behavior() {
        let store = Store::in_memory().unwrap();

        let event1 = {
            let mut e = create_test_event("/test/file.txt");
            e.size_bytes = Some(100);
            e
        };

        let event2 = {
            let mut e = create_test_event("/test/file.txt");
            e.size_bytes = Some(200);
            e
        };

        store.insert_event(&event1).unwrap();
        store.insert_event(&event2).unwrap();

        // Should only have one entry
        assert_eq!(store.count_events().unwrap(), 1);

        // Size should be updated
        let retrieved = store.get_event_by_path(Path::new("/test/file.txt")).unwrap().unwrap();
        assert_eq!(retrieved.size_bytes, Some(200));
    }
}
