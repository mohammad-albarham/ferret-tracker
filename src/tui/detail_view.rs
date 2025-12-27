//! Detail view component
//!
//! Displays detailed information about a selected file event.

use crate::tui::app::App;
use chrono::Local;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

/// Detail view for displaying file event information
pub struct DetailView;

impl DetailView {
    /// Draw the detail view
    pub fn draw(app: &App, frame: &mut Frame, area: Rect) {
        let event = match app.selected_event() {
            Some(e) => e,
            None => {
                let empty = Paragraph::new("No file selected")
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .title(" Details ")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    );
                frame.render_widget(empty, area);
                return;
            }
        };

        // Layout: info panel on the left, actions on the right
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area);

        let info_area = chunks[0];
        let actions_area = chunks[1];

        // File information
        let local_time = event.created_at.with_timezone(&Local);
        let utc_time = event.created_at;

        let exists = event.path.exists();
        let exists_indicator = if exists { "✓" } else { "✗" };
        let exists_color = if exists { Color::Green } else { Color::Red };

        let info_lines = vec![
            Line::from(vec![
                Span::styled("Path: ", Style::default().fg(Color::Yellow)),
                Span::raw(event.path.to_string_lossy().to_string()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Filename: ", Style::default().fg(Color::Yellow)),
                Span::raw(&event.filename),
            ]),
            Line::from(vec![
                Span::styled("Directory: ", Style::default().fg(Color::Yellow)),
                Span::raw(event.dir.to_string_lossy().to_string()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Size: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    event.size_display(),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(format!(
                    " ({})",
                    event
                        .size_bytes
                        .map(|s| format!("{} bytes", s))
                        .unwrap_or_else(|| "unknown".to_string())
                )),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Type: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    event.file_type.as_str(),
                    Self::type_style(event.file_type),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("First Seen: ", Style::default().fg(Color::Yellow)),
                Span::raw(local_time.format("%Y-%m-%d %H:%M:%S %Z").to_string()),
            ]),
            Line::from(vec![
                Span::styled("            ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("({})", utc_time.format("%Y-%m-%d %H:%M:%S UTC")),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Exists: ", Style::default().fg(Color::Yellow)),
                Span::styled(exists_indicator, Style::default().fg(exists_color)),
                Span::raw(if exists { " File present" } else { " File missing" }),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Tags: ", Style::default().fg(Color::Yellow)),
                Span::raw(if event.tags.is_empty() {
                    "(none)".to_string()
                } else {
                    event.tags.clone()
                }),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Notes: ", Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![Span::raw(if event.notes.is_empty() {
                "(none)".to_string()
            } else {
                event.notes.clone()
            })]),
        ];

        let info = Paragraph::new(info_lines)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .title(format!(" {} ", event.filename))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );

        frame.render_widget(info, info_area);

        // Actions panel
        let actions = vec![
            ListItem::new(Line::from(vec![
                Span::styled(" o ", Style::default().fg(Color::Green).bold()),
                Span::raw("Open file"),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled(" O ", Style::default().fg(Color::Green).bold()),
                Span::raw("Open folder"),
            ])),
            ListItem::new(Line::from("")),
            ListItem::new(Line::from(vec![
                Span::styled(" t ", Style::default().fg(Color::Yellow).bold()),
                Span::raw("Edit tags"),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled(" n ", Style::default().fg(Color::Yellow).bold()),
                Span::raw("Edit notes"),
            ])),
            ListItem::new(Line::from("")),
            ListItem::new(Line::from(vec![
                Span::styled(" d ", Style::default().fg(Color::Red).bold()),
                Span::raw("Delete file"),
            ])),
            ListItem::new(Line::from("")),
            ListItem::new(Line::from(vec![
                Span::styled(" q ", Style::default().fg(Color::DarkGray).bold()),
                Span::raw("Back to list"),
            ])),
            ListItem::new(Line::from(vec![
                Span::styled(" ? ", Style::default().fg(Color::DarkGray).bold()),
                Span::raw("Help"),
            ])),
        ];

        let actions_list = List::new(actions).block(
            Block::default()
                .title(" Actions ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

        frame.render_widget(actions_list, actions_area);
    }

    /// Get style for file type
    fn type_style(file_type: crate::models::FileType) -> Style {
        use crate::models::FileType;
        match file_type {
            FileType::Executable => Style::default().fg(Color::Red).bold(),
            FileType::Archive => Style::default().fg(Color::Magenta).bold(),
            FileType::Document => Style::default().fg(Color::Blue).bold(),
            FileType::Media => Style::default().fg(Color::Green).bold(),
            FileType::Code => Style::default().fg(Color::Yellow).bold(),
            FileType::Other => Style::default().fg(Color::Gray),
        }
    }
}
