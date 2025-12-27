//! Main TUI application state and logic
//!
//! This module contains the core application structure that manages
//! the TUI state, handles input, and coordinates between views.

use crate::models::{EventFilter, FileEvent};
use crate::store::Store;
use crate::watcher::WatcherMessage;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use super::detail_view::DetailView;
use super::filters::FilterOverlay;
use super::help::HelpOverlay;
use super::list_view::ListView;
use super::input::InputOverlay;

/// Default page size for pagination
const DEFAULT_PAGE_SIZE: usize = 100;

/// Batch delay for collecting watcher events (milliseconds)
const BATCH_DELAY_MS: u64 = 200;  // Reduced from 500ms for faster updates

/// Current view/screen being displayed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Main list view showing all events
    List,
    /// Detail view for a selected event
    Detail,
}

/// Current input mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Normal navigation mode
    Normal,
    /// Search input mode
    Search,
    /// Filter overlay is open
    Filter,
    /// Help overlay is open
    Help,
    /// Editing tags
    EditTags,
    /// Editing notes
    EditNotes,
    /// Confirmation dialog (e.g., delete)
    Confirm,
}

/// Application state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    /// Application is running normally
    Running,
    /// Application should quit
    Quit,
}

/// Main TUI application
pub struct App {
    /// Current application state
    pub state: AppState,
    /// Current view
    pub view: View,
    /// Current input mode
    pub input_mode: InputMode,
    /// Database store
    pub store: Store,
    /// Current list of events
    pub events: Vec<FileEvent>,
    /// Currently selected event index
    pub selected_index: usize,
    /// Scroll offset for the list
    pub scroll_offset: usize,
    /// Active filter
    pub filter: EventFilter,
    /// Search query
    pub search_query: String,
    /// Input buffer for various input modes
    pub input_buffer: String,
    /// Message to display in status bar
    pub status_message: Option<(String, Instant)>,
    /// Number of watched directories
    pub watched_dirs: usize,
    /// Filter overlay state
    pub filter_overlay: FilterOverlay,
    /// Help overlay state
    pub help_overlay: HelpOverlay,
    /// Confirmation action pending
    pub pending_action: Option<PendingAction>,
    /// Number of visible events after filtering
    pub visible_count: usize,
    
    // Pagination state
    /// Page size for lazy loading
    pub page_size: usize,
    /// Current pagination offset
    pub current_offset: usize,
    /// Total count of matching events (for pagination info)
    pub total_count: usize,
    
    // Dirty flag and batching
    /// Whether a refresh is needed
    pub needs_refresh: bool,
    /// Count of pending new files (batch counter)
    pub pending_new_files: usize,
    /// Last time we batched watcher events
    pub last_batch_time: Instant,
}

/// Actions that require confirmation
#[derive(Debug, Clone)]
pub enum PendingAction {
    /// Delete a file
    DeleteFile(i64, String),
}

impl App {
    /// Create a new App instance
    pub fn new(store: Store) -> Result<Self> {
        // Start with default pagination filter
        let filter = EventFilter::new().with_limit(DEFAULT_PAGE_SIZE).with_offset(0);
        let total_count = store.count_filtered_events(&filter)?;
        let events = store.query_events(&filter)?;
        let visible_count = events.len();

        Ok(Self {
            state: AppState::Running,
            view: View::List,
            input_mode: InputMode::Normal,
            store,
            events,
            selected_index: 0,
            scroll_offset: 0,
            filter,
            search_query: String::new(),
            input_buffer: String::new(),
            status_message: None,
            watched_dirs: 0,
            filter_overlay: FilterOverlay::new(),
            help_overlay: HelpOverlay::new(),
            pending_action: None,
            visible_count,
            // Pagination
            page_size: DEFAULT_PAGE_SIZE,
            current_offset: 0,
            total_count,
            // Dirty flag and batching
            needs_refresh: false,
            pending_new_files: 0,
            last_batch_time: Instant::now(),
        })
    }

    /// Set the number of watched directories
    pub fn set_watched_dirs(&mut self, count: usize) {
        self.watched_dirs = count;
    }

    /// Refresh events from the database with current pagination
    pub fn refresh_events(&mut self) -> Result<()> {
        // Update filter with current pagination settings
        self.filter.limit = self.page_size;
        self.filter.offset = self.current_offset;
        
        // Query events and count
        self.total_count = self.store.count_filtered_events(&self.filter)?;
        self.events = self.store.query_events(&self.filter)?;
        self.visible_count = self.events.len();
        
        // Adjust selection if needed
        if !self.events.is_empty() && self.selected_index >= self.events.len() {
            self.selected_index = self.events.len() - 1;
        }
        
        // Clear refresh flag
        self.needs_refresh = false;
        self.pending_new_files = 0;
        
        Ok(())
    }
    
    /// Schedule a refresh (for batched updates)
    fn schedule_refresh(&mut self) {
        self.needs_refresh = true;
    }
    
    /// Process batched refresh if needed
    pub fn process_batched_refresh(&mut self) -> Result<()> {
        if self.needs_refresh && self.last_batch_time.elapsed() > Duration::from_millis(BATCH_DELAY_MS) {
            self.refresh_events()?;
            if self.pending_new_files > 0 {
                self.set_status(format!("{} new file(s) added", self.pending_new_files));
            }
        }
        Ok(())
    }
    
    /// Go to next page
    pub fn next_page(&mut self) -> Result<()> {
        let max_offset = self.total_count.saturating_sub(self.page_size);
        let new_offset = (self.current_offset + self.page_size).min(max_offset);
        if new_offset != self.current_offset {
            self.current_offset = new_offset;
            self.selected_index = 0;
            self.refresh_events()?;
        }
        Ok(())
    }
    
    /// Go to previous page
    pub fn prev_page(&mut self) -> Result<()> {
        if self.current_offset > 0 {
            self.current_offset = self.current_offset.saturating_sub(self.page_size);
            self.selected_index = 0;
            self.refresh_events()?;
        }
        Ok(())
    }
    
    /// Go to first page
    pub fn first_page(&mut self) -> Result<()> {
        if self.current_offset != 0 {
            self.current_offset = 0;
            self.selected_index = 0;
            self.refresh_events()?;
        }
        Ok(())
    }
    
    /// Go to last page
    pub fn last_page(&mut self) -> Result<()> {
        let max_offset = self.total_count.saturating_sub(self.page_size);
        if self.current_offset != max_offset {
            self.current_offset = max_offset;
            self.selected_index = 0;
            self.refresh_events()?;
        }
        Ok(())
    }
    
    /// Get current page number (1-indexed)
    pub fn current_page(&self) -> usize {
        (self.current_offset / self.page_size) + 1
    }
    
    /// Get total number of pages
    pub fn total_pages(&self) -> usize {
        (self.total_count + self.page_size - 1) / self.page_size
    }

    /// Get the currently selected event
    pub fn selected_event(&self) -> Option<&FileEvent> {
        self.events.get(self.selected_index)
    }

    /// Handle watcher messages
    /// 
    /// Note: The watcher's processing thread already inserts events into the DB.
    /// The UI thread just needs to schedule a refresh to display them.
    pub fn handle_watcher_message(&mut self, msg: WatcherMessage) -> Result<()> {
        match msg {
            WatcherMessage::NewFile(_event) | WatcherMessage::MovedFile(_event) => {
                // Event is already in the database (inserted by watcher processing thread)
                // Just schedule a UI refresh - NO DB I/O on the UI thread!
                self.pending_new_files += 1;
                self.schedule_refresh();
                self.last_batch_time = Instant::now();
            }
            WatcherMessage::Error(err) => {
                self.set_status(format!("Watcher error: {}", err));
            }
            WatcherMessage::Started => {
                self.set_status("File watcher started".to_string());
            }
            WatcherMessage::Stopped => {
                self.set_status("File watcher stopped".to_string());
            }
        }
        Ok(())
    }

    /// Set a status message that will auto-clear
    pub fn set_status(&mut self, message: String) {
        self.status_message = Some((message, Instant::now()));
    }

    /// Clear expired status message
    pub fn clear_expired_status(&mut self) {
        if let Some((_, time)) = &self.status_message {
            if time.elapsed() > Duration::from_secs(5) {
                self.status_message = None;
            }
        }
    }

    /// Handle keyboard input
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        // Handle quit shortcuts globally
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.state = AppState::Quit;
            return Ok(());
        }

        match self.input_mode {
            InputMode::Normal => self.handle_normal_input(key)?,
            InputMode::Search => self.handle_search_input(key)?,
            InputMode::Filter => self.handle_filter_input(key)?,
            InputMode::Help => self.handle_help_input(key)?,
            InputMode::EditTags => self.handle_edit_tags_input(key)?,
            InputMode::EditNotes => self.handle_edit_notes_input(key)?,
            InputMode::Confirm => self.handle_confirm_input(key)?,
        }

        Ok(())
    }

    /// Handle input in normal mode
    fn handle_normal_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            // Quit
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.view == View::Detail {
                    self.view = View::List;
                } else {
                    self.state = AppState::Quit;
                }
            }

            // Navigation within current page
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            
            // Pagination with Ctrl modifier
            KeyCode::PageUp if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.prev_page()?;
                self.set_status(format!("Page {}/{}", self.current_page(), self.total_pages()));
            }
            KeyCode::PageDown if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.next_page()?;
                self.set_status(format!("Page {}/{}", self.current_page(), self.total_pages()));
            }
            KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.first_page()?;
                self.set_status(format!("Page {}/{}", self.current_page(), self.total_pages()));
            }
            KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.last_page()?;
                self.set_status(format!("Page {}/{}", self.current_page(), self.total_pages()));
            }
            
            // Regular page navigation (within page)
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::Home | KeyCode::Char('g') => self.selected_index = 0,
            KeyCode::End | KeyCode::Char('G') => {
                if !self.events.is_empty() {
                    self.selected_index = self.events.len() - 1;
                }
            }

            // View details
            KeyCode::Enter | KeyCode::Char('l') => {
                if self.selected_event().is_some() {
                    self.view = View::Detail;
                }
            }

            // Back from detail view
            KeyCode::Char('h') if self.view == View::Detail => {
                self.view = View::List;
            }

            // Search
            KeyCode::Char('/') => {
                self.input_mode = InputMode::Search;
                self.input_buffer = self.search_query.clone();
            }

            // Filter
            KeyCode::Char('f') => {
                self.input_mode = InputMode::Filter;
                self.filter_overlay.reset();
            }

            // Clear filters and reset pagination
            KeyCode::Char('c') => {
                self.filter = EventFilter::new().with_limit(self.page_size).with_offset(0);
                self.current_offset = 0;
                self.search_query.clear();
                self.refresh_events()?;
                self.set_status("Filters cleared".to_string());
            }

            // Help
            KeyCode::Char('?') => {
                self.input_mode = InputMode::Help;
            }

            // Refresh
            KeyCode::Char('r') => {
                self.refresh_events()?;
                self.set_status("Refreshed".to_string());
            }

            // Open file/folder
            KeyCode::Char('o') => {
                if let Some(event) = self.selected_event() {
                    let path = event.path.clone();
                    if path.exists() {
                        if let Err(e) = open::that(&path) {
                            self.set_status(format!("Failed to open: {}", e));
                        } else {
                            self.set_status(format!("Opened: {}", path.display()));
                        }
                    } else {
                        self.set_status("File no longer exists".to_string());
                    }
                }
            }

            // Open containing folder
            KeyCode::Char('O') => {
                if let Some(event) = self.selected_event() {
                    let dir = event.dir.clone();
                    if dir.exists() {
                        if let Err(e) = open::that(&dir) {
                            self.set_status(format!("Failed to open folder: {}", e));
                        } else {
                            self.set_status(format!("Opened folder: {}", dir.display()));
                        }
                    } else {
                        self.set_status("Folder no longer exists".to_string());
                    }
                }
            }

            // Edit tags
            KeyCode::Char('t') => {
                if let Some(event) = self.selected_event() {
                    self.input_buffer = event.tags.clone();
                    self.input_mode = InputMode::EditTags;
                }
            }

            // Edit notes
            KeyCode::Char('n') => {
                if let Some(event) = self.selected_event() {
                    self.input_buffer = event.notes.clone();
                    self.input_mode = InputMode::EditNotes;
                }
            }

            // Delete file
            KeyCode::Char('d') => {
                if let Some(event) = self.selected_event() {
                    if let Some(id) = event.id {
                        self.pending_action = Some(PendingAction::DeleteFile(
                            id,
                            event.path.to_string_lossy().to_string(),
                        ));
                        self.input_mode = InputMode::Confirm;
                    }
                }
            }

            _ => {}
        }

        Ok(())
    }

    /// Handle input in search mode
    fn handle_search_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                self.search_query = self.input_buffer.clone();
                if self.search_query.is_empty() {
                    self.filter.path_contains = None;
                } else {
                    self.filter.path_contains = Some(self.search_query.clone());
                }
                // Reset pagination when search changes
                self.current_offset = 0;
                self.refresh_events()?;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                self.input_buffer.clear();
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle input in filter mode
    fn handle_filter_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('f') => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                // Apply selected filters and reset pagination
                let mut new_filter = self.filter_overlay.build_filter();
                new_filter.limit = self.page_size;
                new_filter.offset = 0;
                self.filter = new_filter;
                self.current_offset = 0;
                self.refresh_events()?;
                self.input_mode = InputMode::Normal;
                self.set_status(format!("Filter applied: {}", self.filter.summary()));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.filter_overlay.previous();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.filter_overlay.next();
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.filter_overlay.decrease_value();
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.filter_overlay.increase_value();
            }
            KeyCode::Char(' ') => {
                self.filter_overlay.toggle_current();
            }
            KeyCode::Char('c') => {
                self.filter_overlay.reset();
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle input in help mode
    fn handle_help_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Enter => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.help_overlay.scroll_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.help_overlay.scroll_down();
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle input when editing tags
    fn handle_edit_tags_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                if let Some(event) = self.selected_event() {
                    if let Some(id) = event.id {
                        self.store.update_tags(id, &self.input_buffer)?;
                        self.refresh_events()?;
                        self.set_status("Tags updated".to_string());
                    }
                }
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle input when editing notes
    fn handle_edit_notes_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                if let Some(event) = self.selected_event() {
                    if let Some(id) = event.id {
                        self.store.update_notes(id, &self.input_buffer)?;
                        self.refresh_events()?;
                        self.set_status("Notes updated".to_string());
                    }
                }
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle confirmation input
    fn handle_confirm_input(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(action) = self.pending_action.take() {
                    match action {
                        PendingAction::DeleteFile(id, path) => {
                            // Delete from database
                            self.store.delete_event(id)?;
                            
                            // Try to delete the actual file
                            let path = std::path::Path::new(&path);
                            if path.exists() {
                                if let Err(e) = std::fs::remove_file(path) {
                                    self.set_status(format!("Removed from ledger, but failed to delete file: {}", e));
                                } else {
                                    self.set_status("File deleted".to_string());
                                }
                            } else {
                                self.set_status("Removed from ledger (file already gone)".to_string());
                            }
                            
                            self.refresh_events()?;
                        }
                    }
                }
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.pending_action = None;
                self.input_mode = InputMode::Normal;
                self.set_status("Cancelled".to_string());
            }
            _ => {}
        }
        Ok(())
    }

    /// Move selection by delta
    fn move_selection(&mut self, delta: i32) {
        if self.events.is_empty() {
            return;
        }

        let new_index = if delta < 0 {
            self.selected_index.saturating_sub((-delta) as usize)
        } else {
            (self.selected_index + delta as usize).min(self.events.len() - 1)
        };

        self.selected_index = new_index;
    }

    /// Draw the application
    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Main layout: header, content, footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(1), // Footer/status
            ])
            .split(area);

        // Draw header
        self.draw_header(frame, chunks[0]);

        // Draw main content based on current view
        match self.view {
            View::List => ListView::draw(self, frame, chunks[1]),
            View::Detail => DetailView::draw(self, frame, chunks[1]),
        }

        // Draw footer/status
        self.draw_footer(frame, chunks[2]);

        // Draw overlays
        match self.input_mode {
            InputMode::Search => {
                InputOverlay::draw_search(self, frame, area);
            }
            InputMode::Filter => {
                self.filter_overlay.draw(frame, area);
            }
            InputMode::Help => {
                self.help_overlay.draw(frame, area);
            }
            InputMode::EditTags => {
                InputOverlay::draw_edit(self, frame, area, "Edit Tags", "Comma-separated tags");
            }
            InputMode::EditNotes => {
                InputOverlay::draw_edit(self, frame, area, "Edit Notes", "Enter note text");
            }
            InputMode::Confirm => {
                self.draw_confirm_dialog(frame, area);
            }
            InputMode::Normal => {}
        }
    }

    /// Draw the header
    fn draw_header(&self, frame: &mut Frame, area: Rect) {
        let page_info = if self.total_pages() > 1 {
            format!(" │ Page {}/{}", self.current_page(), self.total_pages())
        } else {
            String::new()
        };
        
        let header_text = format!(
            " 🦡 Ferret │ {}/{} files{} │ Watching {} dirs │ {}",
            self.events.len(),
            self.total_count,
            page_info,
            self.watched_dirs,
            self.filter.summary()
        );

        let header = Paragraph::new(header_text)
            .style(Style::default().fg(Color::Cyan).bold())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );

        frame.render_widget(header, area);
    }

    /// Draw the footer/status bar
    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let status = if let Some((msg, _)) = &self.status_message {
            msg.clone()
        } else {
            match self.input_mode {
                InputMode::Normal => {
                    let page_hint = if self.total_pages() > 1 {
                        " │ C-PgUp/Dn:page"
                    } else {
                        ""
                    };
                    format!(" j/k:nav │ Enter:detail │ f:filter │ /:search │ o:open │ ?:help{} │ q:quit ", page_hint)
                }
                InputMode::Search => " Type to search │ Enter:apply │ Esc:cancel ".to_string(),
                InputMode::Filter => " ↑↓:select │ ←→:adjust │ Space:toggle │ Enter:apply │ Esc:cancel ".to_string(),
                InputMode::Help => " ↑↓:scroll │ q/Esc:close ".to_string(),
                InputMode::EditTags | InputMode::EditNotes => " Type to edit │ Enter:save │ Esc:cancel ".to_string(),
                InputMode::Confirm => " y:confirm │ n:cancel ".to_string(),
            }
        };

        let style = if self.status_message.is_some() {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let footer = Paragraph::new(status).style(style);
        frame.render_widget(footer, area);
    }

    /// Draw confirmation dialog
    fn draw_confirm_dialog(&self, frame: &mut Frame, area: Rect) {
        let message = match &self.pending_action {
            Some(PendingAction::DeleteFile(_, path)) => {
                format!("Delete file?\n\n{}\n\n(y)es / (n)o", path)
            }
            None => "Confirm?".to_string(),
        };

        // Center the dialog
        let dialog_width = 60.min(area.width - 4);
        let dialog_height = 7;
        let dialog_area = Rect::new(
            (area.width - dialog_width) / 2,
            (area.height - dialog_height) / 2,
            dialog_width,
            dialog_height,
        );

        // Clear the area behind the dialog
        frame.render_widget(Clear, dialog_area);

        let dialog = Paragraph::new(message)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .title(" Confirm ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)),
            );

        frame.render_widget(dialog, dialog_area);
    }
}

/// Restore terminal to normal state - MUST be called on exit or panic
fn restore_terminal() {
    // Best effort - ignore errors during cleanup
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::cursor::Show
    );
}

/// Install a panic hook that restores the terminal
fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore_terminal();
        original_hook(panic_info);
    }));
}

/// RAII guard that restores terminal on drop
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Run the TUI application
pub fn run_tui(
    mut app: App,
    watcher_rx: Option<Receiver<WatcherMessage>>,
) -> Result<()> {
    // Install panic hook FIRST before any terminal manipulation
    install_panic_hook();
    
    // Setup terminal
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::cursor::Hide
    )?;
    
    // RAII guard ensures cleanup even if we return early via ?
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // Clear and reset terminal state completely
    terminal.clear()?;
    terminal.hide_cursor()?;

    let tick_rate = Duration::from_millis(33); // ~30 FPS for smoother UI
    let mut last_tick = Instant::now();

    loop {
        // Draw first to ensure responsive UI
        // Ratatui will automatically clear and draw the full frame
        terminal.draw(|f| app.draw(f))?;

        // Check for watcher messages (non-blocking)
        if let Some(ref rx) = watcher_rx {
            // Process up to 100 messages per frame to prevent starvation
            for _ in 0..100 {
                match rx.try_recv() {
                    Ok(msg) => {
                        if let Err(_e) = app.handle_watcher_message(msg) {
                            // Silently ignore watcher errors in TUI mode
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        app.set_status("Watcher disconnected".to_string());
                        break;
                    }
                }
            }
        }
        
        // Process batched refresh if needed
        let _ = app.process_batched_refresh();

        // Clear expired status messages
        app.clear_expired_status();

        // Handle input with shorter poll for responsiveness
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                app.handle_key_event(key)?;
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        // Check if we should quit
        if app.state == AppState::Quit {
            break;
        }
    }

    // Guard will handle cleanup via Drop
    Ok(())
}
