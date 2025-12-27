//! List view component
//!
//! Displays the main list of file events in a table format.

use crate::models::FileType;
use crate::tui::app::App;
use chrono::Local;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table},
};

/// List view for displaying file events
pub struct ListView;

impl ListView {
    /// Draw the list view
    pub fn draw(app: &mut App, frame: &mut Frame, area: Rect) {
        // Create the main layout with scrollbar
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        let list_area = chunks[0];
        let scrollbar_area = chunks[1];

        // Calculate visible rows
        let header_height = 1;
        let border_height = 2;
        let visible_rows = (list_area.height as usize).saturating_sub(header_height + border_height);

        // Adjust scroll offset to keep selection visible
        if app.selected_index < app.scroll_offset {
            app.scroll_offset = app.selected_index;
        } else if app.selected_index >= app.scroll_offset + visible_rows {
            app.scroll_offset = app.selected_index - visible_rows + 1;
        }

        // Create table headers
        let header_cells = ["Time", "Size", "Type", "Path"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).bold()));
        let header = Row::new(header_cells).height(1);

        // Create table rows
        let rows: Vec<Row> = app
            .events
            .iter()
            .enumerate()
            .skip(app.scroll_offset)
            .take(visible_rows)
            .map(|(idx, event)| {
                let is_selected = idx == app.selected_index;

                // Format time
                let local_time = event.created_at.with_timezone(&Local);
                let time_str = if local_time.date_naive() == Local::now().date_naive() {
                    local_time.format("%H:%M:%S").to_string()
                } else {
                    local_time.format("%Y-%m-%d %H:%M").to_string()
                };

                // Format size
                let size_str = event.size_display();

                // File type with color
                let type_style = Self::type_style(event.file_type);
                let type_cell = Cell::from(event.file_type.as_label()).style(type_style);

                // Path (truncated)
                let path_str = Self::truncate_path(&event.path.to_string_lossy(), 60);

                let row_style = if is_selected {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(time_str),
                    Cell::from(size_str).style(Style::default().fg(Color::Cyan)),
                    type_cell,
                    Cell::from(path_str),
                ])
                .style(row_style)
            })
            .collect();

        // Column widths
        let widths = [
            Constraint::Length(17),  // Time
            Constraint::Length(10),  // Size
            Constraint::Length(6),   // Type
            Constraint::Min(20),     // Path
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .title(format!(" Files ({}) ", app.events.len()))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

        frame.render_widget(table, list_area);

        // Render scrollbar
        if app.events.len() > visible_rows {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            let mut scrollbar_state = ScrollbarState::new(app.events.len())
                .position(app.selected_index);

            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }

    /// Get style for file type
    fn type_style(file_type: FileType) -> Style {
        match file_type {
            FileType::Executable => Style::default().fg(Color::Red),
            FileType::Archive => Style::default().fg(Color::Magenta),
            FileType::Document => Style::default().fg(Color::Blue),
            FileType::Media => Style::default().fg(Color::Green),
            FileType::Code => Style::default().fg(Color::Yellow),
            FileType::Other => Style::default().fg(Color::Gray),
        }
    }

    /// Truncate path intelligently, keeping the important parts
    fn truncate_path(path: &str, max_len: usize) -> String {
        if path.len() <= max_len {
            return path.to_string();
        }

        // Try to keep the filename and as much of the path as possible
        let parts: Vec<&str> = path.split('/').collect();
        if parts.is_empty() {
            return path[..max_len].to_string();
        }

        let filename = parts.last().unwrap_or(&"");
        let filename_len = filename.len();

        if filename_len >= max_len - 3 {
            // Filename itself is too long
            return format!("...{}", &filename[filename.len().saturating_sub(max_len - 3)..]);
        }

        // Build path from the end, adding directories until we run out of space
        let mut result = filename.to_string();
        let available = max_len - filename_len - 4; // Reserve space for ".../""

        for part in parts[..parts.len() - 1].iter().rev() {
            if result.len() + part.len() + 1 > available {
                break;
            }
            result = format!("{}/{}", part, result);
        }

        if result.len() < path.len() {
            format!(".../{}", result)
        } else {
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_path_short() {
        let path = "/home/user/file.txt";
        assert_eq!(ListView::truncate_path(path, 50), path);
    }

    #[test]
    fn test_truncate_path_long() {
        let path = "/home/user/very/long/path/to/some/deeply/nested/directory/file.txt";
        let truncated = ListView::truncate_path(path, 40);
        assert!(truncated.len() <= 40);
        assert!(truncated.ends_with("file.txt"));
    }
}
