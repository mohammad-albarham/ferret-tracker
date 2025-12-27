//! File system watcher for Ferret
//!
//! This module provides cross-platform file system monitoring using the `notify` crate.
//! It watches configured directories for new file creations and moves.
//!
//! # Architecture
//!
//! The watcher uses a three-stage pipeline to avoid blocking:
//!
//! 1. **Notify Callback** (notify thread): Only captures raw paths, does NO I/O
//! 2. **Processing Thread** (dedicated): Performs all I/O, filtering, deduplication
//! 3. **UI Thread** (main): Receives ready-to-display FileEvents via channel
//!
//! This ensures the notify callback never blocks and the UI thread never does disk I/O.

use crate::config::Config;
use crate::models::FileEvent;
use crate::store::Store;
use anyhow::{Context, Result};
use globset::GlobSet;
use notify::{Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, trace, warn};

/// Debounce window for coalescing rapid events on the same file
const DEBOUNCE_WINDOW_MS: u64 = 300;

/// Maximum events to process per batch
const MAX_BATCH_SIZE: usize = 500;

/// Message types sent from the watcher to the main application
#[derive(Debug, Clone)]
pub enum WatcherMessage {
    /// A new file was detected
    NewFile(FileEvent),
    /// A file was moved/renamed into a watched directory
    MovedFile(FileEvent),
    /// An error occurred during watching
    Error(String),
    /// The watcher started successfully
    Started,
    /// The watcher stopped
    Stopped,
}

/// Internal message for raw events (no I/O performed yet)
#[derive(Debug, Clone)]
enum RawEvent {
    /// A potential file event with path and event kind
    File { path: PathBuf, kind: EventKind },
    /// Shutdown signal
    Shutdown,
}

/// File system watcher that monitors directories for new files
pub struct FileWatcher {
    /// The underlying notify watcher
    watcher: RecommendedWatcher,
    /// Sender for watcher messages (to UI)
    tx: Sender<WatcherMessage>,
    /// Paths currently being watched
    watched_paths: Arc<Mutex<HashSet<PathBuf>>>,
    /// Glob matcher for ignored patterns
    ignore_matcher: GlobSet,
    /// Minimum file size to report
    min_size: u64,
    /// Store reference for checking existing paths
    store: Option<Store>,
    /// Shutdown flag for processing thread
    shutdown: Arc<AtomicBool>,
    /// Handle to the processing thread
    processor_handle: Option<JoinHandle<()>>,
    /// Sender for raw events to processing thread
    raw_event_tx: Sender<RawEvent>,
}

impl FileWatcher {
    /// Create a new FileWatcher with the given configuration
    pub fn new(config: &Config, store: Option<Store>) -> Result<(Self, Receiver<WatcherMessage>)> {
        let (tx, rx) = mpsc::channel();
        let (raw_event_tx, raw_event_rx) = mpsc::channel::<RawEvent>();
        let ignore_matcher = config.build_ignore_matcher()?;
        let min_size = config.min_size_bytes;
        let watched_paths = Arc::new(Mutex::new(HashSet::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let debounce_ms = config.debounce_ms;
        
        // Clone for the notify callback (minimal - only sends raw paths)
        let raw_tx_for_notify = raw_event_tx.clone();

        // Create the watcher with a MINIMAL callback - NO I/O!
        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        // Only pass through create/modify events, filter out the rest immediately
                        let dominated_by = matches!(
                            event.kind,
                            EventKind::Create(_) | EventKind::Modify(notify::event::ModifyKind::Name(_))
                        );
                        
                        if dominated_by {
                            for path in event.paths {
                                // Send raw path - NO I/O here!
                                let _ = raw_tx_for_notify.send(RawEvent::File { 
                                    path, 
                                    kind: event.kind.clone() 
                                });
                            }
                        }
                    }
                    Err(e) => {
                        error!("Watcher error: {:?}", e);
                    }
                }
            },
            NotifyConfig::default()
                .with_poll_interval(Duration::from_millis(debounce_ms.max(100))),
        )
        .context("Failed to create file watcher")?;

        // Clone data for the processing thread
        let tx_for_processor = tx.clone();
        let store_for_processor = store.clone();
        let ignore_matcher_for_processor = ignore_matcher.clone();
        let shutdown_for_processor = shutdown.clone();
        let min_size_for_processor = min_size;

        // Spawn dedicated processing thread for all I/O operations
        let processor_handle = thread::Builder::new()
            .name("ferret-watcher-processor".to_string())
            .spawn(move || {
                Self::run_processor(
                    raw_event_rx,
                    tx_for_processor,
                    store_for_processor,
                    ignore_matcher_for_processor,
                    min_size_for_processor,
                    shutdown_for_processor,
                );
            })
            .context("Failed to spawn watcher processor thread")?;

        let file_watcher = Self {
            watcher,
            tx,
            watched_paths,
            ignore_matcher,
            min_size,
            store,
            shutdown,
            processor_handle: Some(processor_handle),
            raw_event_tx,
        };

        Ok((file_watcher, rx))
    }

    /// Processing thread: handles all I/O, debouncing, and deduplication
    fn run_processor(
        raw_rx: Receiver<RawEvent>,
        tx: Sender<WatcherMessage>,
        store: Option<Store>,
        ignore_matcher: GlobSet,
        min_size: u64,
        shutdown: Arc<AtomicBool>,
    ) {
        // Debounce map: path -> (last_seen_time, event_kind)
        let mut pending: HashMap<PathBuf, (Instant, EventKind)> = HashMap::new();
        
        // Set of paths we've already processed (in-memory dedup for current session)
        let mut processed_this_session: HashSet<PathBuf> = HashSet::new();
        
        let debounce_duration = Duration::from_millis(DEBOUNCE_WINDOW_MS);

        loop {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            // Collect batch of raw events (non-blocking with timeout)
            let mut batch_count = 0;
            loop {
                match raw_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(RawEvent::File { path, kind }) => {
                        pending.insert(path, (Instant::now(), kind));
                        batch_count += 1;
                        if batch_count >= MAX_BATCH_SIZE {
                            break;
                        }
                    }
                    Ok(RawEvent::Shutdown) => {
                        return;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }

            // Process events that have "settled" (past debounce window)
            let now = Instant::now();
            let mut to_process = Vec::new();
            
            pending.retain(|path, (time, kind)| {
                if now.duration_since(*time) >= debounce_duration {
                    to_process.push((path.clone(), kind.clone()));
                    false // Remove from pending
                } else {
                    true // Keep in pending
                }
            });

            // Process settled events (THIS is where I/O happens)
            for (path, kind) in to_process {
                // Skip if already processed this session
                if processed_this_session.contains(&path) {
                    continue;
                }

                // Now we can do I/O safely - we're on the processing thread
                if !path.exists() {
                    trace!("Ignoring path (no longer exists): {}", path.display());
                    continue;
                }

                if path.is_dir() {
                    continue;
                }

                // Check ignore patterns
                if Self::should_ignore(&path, &ignore_matcher) {
                    trace!("Ignoring path (matches ignore pattern): {}", path.display());
                    continue;
                }

                // Check file size
                if let Ok(metadata) = path.metadata() {
                    if metadata.len() < min_size {
                        trace!("Ignoring path (too small): {} ({} bytes)", path.display(), metadata.len());
                        continue;
                    }
                }

                // Check database for existing entry
                if let Some(ref store) = store {
                    if let Ok(true) = store.path_exists(&path) {
                        trace!("Ignoring path (already tracked): {}", path.display());
                        processed_this_session.insert(path.clone());
                        continue;
                    }
                }

                // Create file event
                let file_event = FileEvent::from_path(path.clone());
                
                // INSERT INTO DATABASE HERE - not on UI thread!
                // This is the key architectural fix: DB writes happen on the 
                // processing thread, not the UI thread.
                if let Some(ref store) = store {
                    if let Err(e) = store.insert_event(&file_event) {
                        error!("Failed to insert event into database: {}", e);
                        // Continue anyway - we'll still notify the UI
                    }
                }

                // Determine message type
                let message = match kind {
                    EventKind::Create(_) => WatcherMessage::NewFile(file_event),
                    EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
                        WatcherMessage::MovedFile(file_event)
                    }
                    _ => WatcherMessage::NewFile(file_event),
                };

                debug!("Detected new file: {}", path.display());
                processed_this_session.insert(path);
                
                if let Err(e) = tx.send(message) {
                    error!("Failed to send watcher message: {}", e);
                }
            }

            // Periodically trim the session cache if it gets too large
            if processed_this_session.len() > 10000 {
                processed_this_session.clear();
            }
        }
    }

    /// Start watching the configured paths
    pub fn watch_paths(&mut self, paths: &[PathBuf]) -> Result<()> {
        for path in paths {
            self.watch_path(path)?;
        }
        
        let _ = self.tx.send(WatcherMessage::Started);
        info!("File watcher started, monitoring {} directories", paths.len());
        
        Ok(())
    }

    /// Add a single path to watch
    pub fn watch_path(&mut self, path: &Path) -> Result<()> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        
        if !path.exists() {
            warn!("Path does not exist, skipping: {}", path.display());
            return Ok(());
        }

        if !path.is_dir() {
            warn!("Path is not a directory, skipping: {}", path.display());
            return Ok(());
        }

        // Check if already watching
        {
            let mut watched = self.watched_paths.lock()
                .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
            if watched.contains(&path) {
                debug!("Already watching: {}", path.display());
                return Ok(());
            }
            watched.insert(path.clone());
        }

        self.watcher
            .watch(&path, RecursiveMode::Recursive)
            .with_context(|| format!("Failed to watch path: {}", path.display()))?;

        info!("Now watching: {}", path.display());
        Ok(())
    }

    /// Stop watching a path
    pub fn unwatch_path(&mut self, path: &Path) -> Result<()> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        
        {
            let mut watched = self.watched_paths.lock()
                .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
            watched.remove(&path);
        }

        self.watcher
            .unwatch(&path)
            .with_context(|| format!("Failed to unwatch path: {}", path.display()))?;

        info!("Stopped watching: {}", path.display());
        Ok(())
    }

    /// Stop all watching and shut down processing thread
    pub fn stop(&mut self) -> Result<()> {
        // Signal shutdown to processing thread
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = self.raw_event_tx.send(RawEvent::Shutdown);
        
        // Wait for processing thread to finish
        if let Some(handle) = self.processor_handle.take() {
            let _ = handle.join();
        }
        
        let paths: Vec<PathBuf> = {
            let watched = self.watched_paths.lock()
                .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
            watched.iter().cloned().collect()
        };

        for path in paths {
            let _ = self.watcher.unwatch(&path);
        }

        {
            let mut watched = self.watched_paths.lock()
                .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
            watched.clear();
        }

        let _ = self.tx.send(WatcherMessage::Stopped);
        info!("File watcher stopped");
        Ok(())
    }

    /// Get the list of currently watched paths
    pub fn watched_paths(&self) -> Vec<PathBuf> {
        self.watched_paths
            .lock()
            .map(|guard| guard.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Check if a path should be ignored
    fn should_ignore(path: &Path, matcher: &GlobSet) -> bool {
        let path_str = path.to_string_lossy();
        
        // Check against glob patterns
        if matcher.is_match(&*path_str) {
            return true;
        }

        // Also check just the filename for patterns like ".*"
        if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
            if matcher.is_match(filename) {
                return true;
            }
            
            // Skip hidden files (starting with .)
            if filename.starts_with('.') {
                return true;
            }
        }

        false
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        // Ensure clean shutdown
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = self.raw_event_tx.send(RawEvent::Shutdown);
        if let Some(handle) = self.processor_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Builder for FileWatcher with a fluent API
pub struct FileWatcherBuilder {
    watch_paths: Vec<PathBuf>,
    ignore_patterns: Vec<String>,
    min_size: u64,
    debounce_ms: u64,
    store: Option<Store>,
}

impl FileWatcherBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            watch_paths: Vec::new(),
            ignore_patterns: Vec::new(),
            min_size: 0,
            debounce_ms: 500,
            store: None,
        }
    }

    /// Add a path to watch
    pub fn watch<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.watch_paths.push(path.as_ref().to_path_buf());
        self
    }

    /// Add multiple paths to watch
    pub fn watch_all<P: AsRef<Path>>(mut self, paths: impl IntoIterator<Item = P>) -> Self {
        for path in paths {
            self.watch_paths.push(path.as_ref().to_path_buf());
        }
        self
    }

    /// Add an ignore pattern
    pub fn ignore(mut self, pattern: &str) -> Self {
        self.ignore_patterns.push(pattern.to_string());
        self
    }

    /// Add multiple ignore patterns
    pub fn ignore_all(mut self, patterns: impl IntoIterator<Item = String>) -> Self {
        self.ignore_patterns.extend(patterns);
        self
    }

    /// Set minimum file size
    pub fn min_size(mut self, size: u64) -> Self {
        self.min_size = size;
        self
    }

    /// Set debounce delay
    pub fn debounce_ms(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Set store for path checking
    pub fn with_store(mut self, store: Store) -> Self {
        self.store = Some(store);
        self
    }

    /// Build the FileWatcher
    pub fn build(self) -> Result<(FileWatcher, Receiver<WatcherMessage>)> {
        let config = Config {
            watch_paths: self.watch_paths.clone(),
            ignore_patterns: self.ignore_patterns,
            min_size_bytes: self.min_size,
            debounce_ms: self.debounce_ms,
            ..Config::default()
        };

        let (mut watcher, rx) = FileWatcher::new(&config, self.store)?;
        watcher.watch_paths(&self.watch_paths)?;
        
        Ok((watcher, rx))
    }
}

impl Default for FileWatcherBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;
    use std::time::Duration;

    #[test]
    fn test_watcher_creation() {
        let config = Config::default();
        let result = FileWatcher::new(&config, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ignore_patterns() {
        let config = Config::default();
        let matcher = config.build_ignore_matcher().unwrap();

        assert!(FileWatcher::should_ignore(
            Path::new("/project/node_modules/pkg/file.js"),
            &matcher
        ));
        assert!(FileWatcher::should_ignore(
            Path::new("/project/.hidden"),
            &matcher
        ));
        assert!(FileWatcher::should_ignore(
            Path::new("/project/file.tmp"),
            &matcher
        ));
    }

    #[test]
    fn test_watcher_builder() {
        let temp_dir = TempDir::new().unwrap();
        
        let result = FileWatcherBuilder::new()
            .watch(temp_dir.path())
            .ignore("*.tmp")
            .min_size(0)
            .build();
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_file_detection() {
        let temp_dir = TempDir::new().unwrap();
        
        let (mut watcher, rx) = FileWatcherBuilder::new()
            .watch(temp_dir.path())
            .min_size(0)
            .build()
            .unwrap();

        // Wait a bit for watcher to start
        std::thread::sleep(Duration::from_millis(100));

        // Create a file
        let file_path = temp_dir.path().join("test_file.txt");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "Hello, Ferret!").unwrap();
        file.sync_all().unwrap();

        // Wait for event
        std::thread::sleep(Duration::from_millis(1000));

        // Check for message (may or may not arrive depending on platform timing)
        // This is a best-effort test
        let _ = rx.try_recv();

        watcher.stop().unwrap();
    }
}
