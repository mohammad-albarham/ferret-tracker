//! Filter overlay component
//!
//! Provides an interactive overlay for setting filter criteria.

use crate::models::{EventFilter, FileType};
use chrono::{Duration, Utc};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, List, ListItem},
};

/// Filter option types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOption {
    FileType,
    TimePeriod,
    MinSize,
}

/// Time period options for filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimePeriod {
    All,
    LastHour,
    Last24Hours,
    Last7Days,
    Last30Days,
}

impl TimePeriod {
    pub fn as_str(&self) -> &'static str {
        match self {
            TimePeriod::All => "All time",
            TimePeriod::LastHour => "Last hour",
            TimePeriod::Last24Hours => "Last 24 hours",
            TimePeriod::Last7Days => "Last 7 days",
            TimePeriod::Last30Days => "Last 30 days",
        }
    }

    pub fn all() -> &'static [TimePeriod] {
        &[
            TimePeriod::All,
            TimePeriod::LastHour,
            TimePeriod::Last24Hours,
            TimePeriod::Last7Days,
            TimePeriod::Last30Days,
        ]
    }

    pub fn next(&self) -> TimePeriod {
        match self {
            TimePeriod::All => TimePeriod::LastHour,
            TimePeriod::LastHour => TimePeriod::Last24Hours,
            TimePeriod::Last24Hours => TimePeriod::Last7Days,
            TimePeriod::Last7Days => TimePeriod::Last30Days,
            TimePeriod::Last30Days => TimePeriod::All,
        }
    }

    pub fn prev(&self) -> TimePeriod {
        match self {
            TimePeriod::All => TimePeriod::Last30Days,
            TimePeriod::LastHour => TimePeriod::All,
            TimePeriod::Last24Hours => TimePeriod::LastHour,
            TimePeriod::Last7Days => TimePeriod::Last24Hours,
            TimePeriod::Last30Days => TimePeriod::Last7Days,
        }
    }
}

/// Size threshold options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeThreshold {
    Any,
    AtLeast1KB,
    AtLeast100KB,
    AtLeast1MB,
    AtLeast10MB,
    AtLeast100MB,
}

impl SizeThreshold {
    pub fn as_str(&self) -> &'static str {
        match self {
            SizeThreshold::Any => "Any size",
            SizeThreshold::AtLeast1KB => "≥ 1 KB",
            SizeThreshold::AtLeast100KB => "≥ 100 KB",
            SizeThreshold::AtLeast1MB => "≥ 1 MB",
            SizeThreshold::AtLeast10MB => "≥ 10 MB",
            SizeThreshold::AtLeast100MB => "≥ 100 MB",
        }
    }

    pub fn to_bytes(&self) -> Option<u64> {
        match self {
            SizeThreshold::Any => None,
            SizeThreshold::AtLeast1KB => Some(1024),
            SizeThreshold::AtLeast100KB => Some(100 * 1024),
            SizeThreshold::AtLeast1MB => Some(1024 * 1024),
            SizeThreshold::AtLeast10MB => Some(10 * 1024 * 1024),
            SizeThreshold::AtLeast100MB => Some(100 * 1024 * 1024),
        }
    }

    pub fn next(&self) -> SizeThreshold {
        match self {
            SizeThreshold::Any => SizeThreshold::AtLeast1KB,
            SizeThreshold::AtLeast1KB => SizeThreshold::AtLeast100KB,
            SizeThreshold::AtLeast100KB => SizeThreshold::AtLeast1MB,
            SizeThreshold::AtLeast1MB => SizeThreshold::AtLeast10MB,
            SizeThreshold::AtLeast10MB => SizeThreshold::AtLeast100MB,
            SizeThreshold::AtLeast100MB => SizeThreshold::Any,
        }
    }

    pub fn prev(&self) -> SizeThreshold {
        match self {
            SizeThreshold::Any => SizeThreshold::AtLeast100MB,
            SizeThreshold::AtLeast1KB => SizeThreshold::Any,
            SizeThreshold::AtLeast100KB => SizeThreshold::AtLeast1KB,
            SizeThreshold::AtLeast1MB => SizeThreshold::AtLeast100KB,
            SizeThreshold::AtLeast10MB => SizeThreshold::AtLeast1MB,
            SizeThreshold::AtLeast100MB => SizeThreshold::AtLeast10MB,
        }
    }
}

/// Filter overlay state
pub struct FilterOverlay {
    /// Currently selected option index
    pub selected: usize,
    /// Selected file types (toggle each)
    pub selected_types: Vec<bool>,
    /// Selected time period
    pub time_period: TimePeriod,
    /// Selected size threshold
    pub size_threshold: SizeThreshold,
}

impl FilterOverlay {
    pub fn new() -> Self {
        Self {
            selected: 0,
            selected_types: vec![false; FileType::all().len()],
            time_period: TimePeriod::All,
            size_threshold: SizeThreshold::Any,
        }
    }

    /// Reset all filter selections
    pub fn reset(&mut self) {
        self.selected = 0;
        self.selected_types = vec![false; FileType::all().len()];
        self.time_period = TimePeriod::All;
        self.size_threshold = SizeThreshold::Any;
    }

    /// Get total number of options
    fn total_options(&self) -> usize {
        // File types + time period + size threshold
        FileType::all().len() + 2
    }

    /// Move to next option
    pub fn next(&mut self) {
        self.selected = (self.selected + 1) % self.total_options();
    }

    /// Move to previous option
    pub fn previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        } else {
            self.selected = self.total_options() - 1;
        }
    }

    /// Toggle current selection or increase value
    pub fn toggle_current(&mut self) {
        let type_count = FileType::all().len();
        
        if self.selected < type_count {
            // Toggle file type
            self.selected_types[self.selected] = !self.selected_types[self.selected];
        }
    }

    /// Increase value for current selection
    pub fn increase_value(&mut self) {
        let type_count = FileType::all().len();
        
        if self.selected == type_count {
            // Time period
            self.time_period = self.time_period.next();
        } else if self.selected == type_count + 1 {
            // Size threshold
            self.size_threshold = self.size_threshold.next();
        } else {
            // Toggle file type
            self.toggle_current();
        }
    }

    /// Decrease value for current selection
    pub fn decrease_value(&mut self) {
        let type_count = FileType::all().len();
        
        if self.selected == type_count {
            // Time period
            self.time_period = self.time_period.prev();
        } else if self.selected == type_count + 1 {
            // Size threshold
            self.size_threshold = self.size_threshold.prev();
        } else {
            // Toggle file type
            self.toggle_current();
        }
    }

    /// Build an EventFilter from current selections
    pub fn build_filter(&self) -> EventFilter {
        let mut filter = EventFilter::new();

        // Check if any file type is selected
        let selected_type_indices: Vec<usize> = self
            .selected_types
            .iter()
            .enumerate()
            .filter(|(_, &selected)| selected)
            .map(|(i, _)| i)
            .collect();

        // If exactly one type is selected, filter by it
        if selected_type_indices.len() == 1 {
            filter.file_type = Some(FileType::all()[selected_type_indices[0]]);
        }

        // Time period
        match self.time_period {
            TimePeriod::All => {}
            TimePeriod::LastHour => {
                filter.since = Some(Utc::now() - Duration::hours(1));
            }
            TimePeriod::Last24Hours => {
                filter.since = Some(Utc::now() - Duration::hours(24));
            }
            TimePeriod::Last7Days => {
                filter.since = Some(Utc::now() - Duration::days(7));
            }
            TimePeriod::Last30Days => {
                filter.since = Some(Utc::now() - Duration::days(30));
            }
        }

        // Size threshold
        if let Some(min_size) = self.size_threshold.to_bytes() {
            filter.min_size = Some(min_size);
        }

        filter
    }

    /// Draw the filter overlay
    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        // Calculate overlay size and position
        let overlay_width = 50.min(area.width - 4);
        let overlay_height = (self.total_options() as u16 + 8).min(area.height - 4);
        let overlay_area = Rect::new(
            (area.width - overlay_width) / 2,
            (area.height - overlay_height) / 2,
            overlay_width,
            overlay_height,
        );

        // Clear the area behind the overlay
        frame.render_widget(Clear, overlay_area);

        // Build list items
        let mut items: Vec<ListItem> = Vec::new();

        // Section header for file types
        items.push(ListItem::new(Line::from(vec![
            Span::styled("─ File Type ", Style::default().fg(Color::Yellow).bold()),
            Span::styled("─".repeat(30), Style::default().fg(Color::DarkGray)),
        ])));

        // File type options
        for (i, file_type) in FileType::all().iter().enumerate() {
            let selected = self.selected_types[i];
            let checkbox = if selected { "[✓]" } else { "[ ]" };
            let style = if i == self.selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", checkbox),
                    if selected {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(file_type.as_str(), style),
            ])));
        }

        // Section header for time
        items.push(ListItem::new(Line::from("")));
        items.push(ListItem::new(Line::from(vec![
            Span::styled("─ Time Period ", Style::default().fg(Color::Yellow).bold()),
            Span::styled("─".repeat(28), Style::default().fg(Color::DarkGray)),
        ])));

        // Time period option
        let type_count = FileType::all().len();
        let time_style = if self.selected == type_count {
            Style::default().bg(Color::DarkGray).fg(Color::White)
        } else {
            Style::default()
        };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(" ◄ ", Style::default().fg(Color::Cyan)),
            Span::styled(self.time_period.as_str(), time_style),
            Span::styled(" ►", Style::default().fg(Color::Cyan)),
        ])));

        // Section header for size
        items.push(ListItem::new(Line::from("")));
        items.push(ListItem::new(Line::from(vec![
            Span::styled("─ Minimum Size ", Style::default().fg(Color::Yellow).bold()),
            Span::styled("─".repeat(27), Style::default().fg(Color::DarkGray)),
        ])));

        // Size threshold option
        let size_style = if self.selected == type_count + 1 {
            Style::default().bg(Color::DarkGray).fg(Color::White)
        } else {
            Style::default()
        };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(" ◄ ", Style::default().fg(Color::Cyan)),
            Span::styled(self.size_threshold.as_str(), size_style),
            Span::styled(" ►", Style::default().fg(Color::Cyan)),
        ])));

        // Instructions
        items.push(ListItem::new(Line::from("")));
        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                " ↑↓:select  ←→:change  Space:toggle  Enter:apply  Esc:cancel",
                Style::default().fg(Color::DarkGray),
            ),
        ])));

        let list = List::new(items).block(
            Block::default()
                .title(" Filter ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

        frame.render_widget(list, overlay_area);
    }
}

impl Default for FilterOverlay {
    fn default() -> Self {
        Self::new()
    }
}
