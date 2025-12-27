//! Input overlay component
//!
//! Provides text input overlays for search, tags, and notes editing.

use crate::tui::app::App;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};

/// Input overlay for text entry
pub struct InputOverlay;

impl InputOverlay {
    /// Draw search input overlay
    pub fn draw_search(app: &App, frame: &mut Frame, area: Rect) {
        let overlay_width = 50.min(area.width - 4);
        let overlay_height = 3;
        let overlay_area = Rect::new(
            (area.width - overlay_width) / 2,
            (area.height - overlay_height) / 2,
            overlay_width,
            overlay_height,
        );

        // Clear the area behind the overlay
        frame.render_widget(Clear, overlay_area);

        let input = Paragraph::new(format!("{}_", app.input_buffer))
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .title(" Search (Enter to apply, Esc to cancel) ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            );

        frame.render_widget(input, overlay_area);
    }

    /// Draw edit input overlay (for tags or notes)
    pub fn draw_edit(app: &App, frame: &mut Frame, area: Rect, title: &str, hint: &str) {
        let overlay_width = 60.min(area.width - 4);
        let overlay_height = 5;
        let overlay_area = Rect::new(
            (area.width - overlay_width) / 2,
            (area.height - overlay_height) / 2,
            overlay_width,
            overlay_height,
        );

        // Clear the area behind the overlay
        frame.render_widget(Clear, overlay_area);

        let text = vec![
            Line::from(vec![
                Span::styled(
                    format!("{}: ", hint),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(format!("{}_", app.input_buffer)),
        ];

        let input = Paragraph::new(text)
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .title(format!(" {} (Enter to save, Esc to cancel) ", title))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );

        frame.render_widget(input, overlay_area);
    }
}
