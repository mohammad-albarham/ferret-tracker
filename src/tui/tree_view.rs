//! Tree view component
//!
//! Displays files in a nested directory hierarchy with expand/collapse.

use crate::models::{FileType, FlattenedNode, FolderGroup, TreeNode, ViewMode};
use crate::tui::app::App;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Cell, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table},
};

/// Tree view for displaying files in nested hierarchy
pub struct TreeView;

impl TreeView {
    /// Draw the tree view based on current view mode
    pub fn draw(app: &mut App, frame: &mut Frame, area: Rect) {
        match app.view_mode {
            ViewMode::Flat => Self::draw_flat(app, frame, area),
            ViewMode::GroupByFolder => Self::draw_grouped(app, frame, area),
            ViewMode::TreeView => Self::draw_tree(app, frame, area),
        }
    }

    /// Draw flat list view (original behavior)
    fn draw_flat(app: &mut App, frame: &mut Frame, area: Rect) {
        // Delegate to the original ListView for flat mode
        super::list_view::ListView::draw(app, frame, area);
    }

    /// Draw grouped by folder view
    fn draw_grouped(app: &mut App, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        let list_area = chunks[0];
        let scrollbar_area = chunks[1];

        let header_height = 1;
        let border_height = 2;
        let visible_rows = (list_area.height as usize).saturating_sub(header_height + border_height);

        // Build display rows from folder groups
        let mut display_rows: Vec<GroupedRow> = Vec::new();
        for group in &app.folder_groups {
            // Folder header
            display_rows.push(GroupedRow::FolderHeader {
                path: group.path.clone(),
                name: group.name.clone(),
                file_count: group.files.len(),
                total_size: group.total_size,
                expanded: group.expanded,
            });
            
            // Files in folder (if expanded)
            if group.expanded {
                for file in &group.files {
                    display_rows.push(GroupedRow::File {
                        event_index: app.events.iter().position(|e| e.path == file.path),
                        filename: file.filename.clone(),
                        size_bytes: file.size_bytes,
                        file_type: file.file_type,
                    });
                }
            }
        }

        // Adjust scroll offset
        if app.grouped_selected_index < app.grouped_scroll_offset {
            app.grouped_scroll_offset = app.grouped_selected_index;
        } else if app.grouped_selected_index >= app.grouped_scroll_offset + visible_rows {
            app.grouped_scroll_offset = app.grouped_selected_index - visible_rows + 1;
        }

        let total_rows = display_rows.len();

        // Create table rows
        let rows: Vec<Row> = display_rows
            .iter()
            .enumerate()
            .skip(app.grouped_scroll_offset)
            .take(visible_rows)
            .map(|(idx, row)| {
                let is_selected = idx == app.grouped_selected_index;
                let style = if is_selected {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };

                match row {
                    GroupedRow::FolderHeader { name, file_count, total_size, expanded, .. } => {
                        let icon = if *expanded { "▼" } else { "▶" };
                        let size_str = format_size(*total_size);
                        Row::new(vec![
                            Cell::from(format!("{} 📁 {} ({} files, {})", icon, name, file_count, size_str))
                                .style(Style::default().fg(Color::Cyan).bold()),
                        ]).style(style)
                    }
                    GroupedRow::File { filename, size_bytes, file_type, .. } => {
                        let icon = Self::file_icon(*file_type);
                        let size_str = size_bytes.map(format_size).unwrap_or_else(|| "?".to_string());
                        let type_style = Self::type_style(*file_type);
                        Row::new(vec![
                            Cell::from(format!("    {} {} ({})", icon, filename, size_str))
                                .style(type_style),
                        ]).style(style)
                    }
                }
            })
            .collect();

        let table = Table::new(rows, [Constraint::Percentage(100)])
            .block(
                Block::default()
                    .title(format!(" Grouped View ({} folders) [Tab: switch view] ", app.folder_groups.len()))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );

        frame.render_widget(table, list_area);

        // Render scrollbar
        if total_rows > visible_rows {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            let mut scrollbar_state = ScrollbarState::new(total_rows)
                .position(app.grouped_selected_index);

            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }

    /// Draw full tree hierarchy view
    fn draw_tree(app: &mut App, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        let list_area = chunks[0];
        let scrollbar_area = chunks[1];

        let header_height = 1;
        let border_height = 2;
        let visible_rows = (list_area.height as usize).saturating_sub(header_height + border_height);

        // Ensure selection is visible
        app.tree_state.ensure_visible(visible_rows);

        let flattened = &app.tree_state.flattened;
        let total_rows = flattened.len();
        let selected_idx = app.tree_state.get_selected_index();

        // Create table rows
        let rows: Vec<Row> = flattened
            .iter()
            .enumerate()
            .skip(app.tree_state.scroll_offset)
            .take(visible_rows)
            .map(|(idx, node)| {
                let is_selected = idx == selected_idx;
                let style = if is_selected {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };

                // Build tree branch characters
                let indent = Self::build_tree_indent(node);
                
                // Expand/collapse indicator for directories
                let expand_indicator = if node.is_dir {
                    if node.is_expanded { "▼ " } else { "▶ " }
                } else {
                    "  "
                };

                // Icon
                let icon = if node.is_dir {
                    "📁"
                } else {
                    Self::file_icon(node.file_type.unwrap_or(FileType::Other))
                };

                // Size/count info
                let info = if node.is_dir {
                    format!("({} files)", node.file_count)
                } else {
                    node.size_bytes.map(format_size).unwrap_or_default()
                };

                let display = format!("{}{}{} {} {}", indent, expand_indicator, icon, node.name, info);

                let cell_style = if node.is_dir {
                    Style::default().fg(Color::Cyan)
                } else {
                    Self::type_style(node.file_type.unwrap_or(FileType::Other))
                };

                Row::new(vec![
                    Cell::from(display).style(cell_style),
                ]).style(style)
            })
            .collect();

        let table = Table::new(rows, [Constraint::Percentage(100)])
            .block(
                Block::default()
                    .title(format!(" Tree View ({} items) [Tab: switch, ←→: expand/collapse] ", total_rows))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );

        frame.render_widget(table, list_area);

        // Render scrollbar
        if total_rows > visible_rows {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            let mut scrollbar_state = ScrollbarState::new(total_rows)
                .position(selected_idx);

            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }

    /// Build tree indentation string with branch characters
    fn build_tree_indent(node: &FlattenedNode) -> String {
        let mut indent = String::new();
        
        // Add vertical lines for ancestors
        for &is_last in &node.ancestor_is_last {
            if is_last {
                indent.push_str("   ");
            } else {
                indent.push_str("│  ");
            }
        }
        
        // Add branch for current node
        if node.depth > 0 {
            if node.is_last_sibling {
                indent.push_str("└─");
            } else {
                indent.push_str("├─");
            }
        }
        
        indent
    }

    /// Get icon for file type
    fn file_icon(file_type: FileType) -> &'static str {
        match file_type {
            FileType::Executable => "⚙️ ",
            FileType::Archive => "📦",
            FileType::Document => "📄",
            FileType::Media => "🎬",
            FileType::Code => "💻",
            FileType::Other => "📎",
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
}

/// Row type for grouped view
enum GroupedRow {
    FolderHeader {
        path: std::path::PathBuf,
        name: String,
        file_count: usize,
        total_size: u64,
        expanded: bool,
    },
    File {
        event_index: Option<usize>,
        filename: String,
        size_bytes: Option<u64>,
        file_type: FileType,
    },
}

/// Format file size in human-readable format
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}
