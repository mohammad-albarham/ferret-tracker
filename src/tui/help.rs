//! Help overlay component
//!
//! Displays keybinding help information.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

/// Help overlay state
pub struct HelpOverlay {
    /// Current scroll position
    pub scroll: u16,
}

impl HelpOverlay {
    pub fn new() -> Self {
        Self { scroll: 0 }
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    /// Draw the help overlay
    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        // Calculate overlay size and position
        let overlay_width = 60.min(area.width - 4);
        let overlay_height = 30.min(area.height - 4);
        let overlay_area = Rect::new(
            (area.width - overlay_width) / 2,
            (area.height - overlay_height) / 2,
            overlay_width,
            overlay_height,
        );

        // Clear the area behind the overlay
        frame.render_widget(Clear, overlay_area);

        let help_text = vec![
            Line::from(Span::styled(
                "🦡 Ferret - File Tracker",
                Style::default().fg(Color::Cyan).bold(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "View Modes",
                Style::default().fg(Color::Yellow).bold(),
            )),
            Line::from("  Tab        Switch view (Flat → Grouped → Tree)"),
            Line::from("  ←/h        Collapse dir / Back (Tree/Grouped)"),
            Line::from("  →/l        Expand dir / Enter (Tree/Grouped)"),
            Line::from("  Space      Toggle expand/collapse"),
            Line::from("  e          Expand all (Tree view)"),
            Line::from("  E          Collapse all (Tree view)"),
            Line::from(""),
            Line::from(Span::styled(
                "Navigation",
                Style::default().fg(Color::Yellow).bold(),
            )),
            Line::from("  ↑/k        Move selection up"),
            Line::from("  ↓/j        Move selection down"),
            Line::from("  PgUp/PgDn  Scroll by page"),
            Line::from("  Home/g     Jump to start"),
            Line::from("  End/G      Jump to end"),
            Line::from("  Enter      View details / Toggle folder"),
            Line::from(""),
            Line::from(Span::styled(
                "Filtering & Search",
                Style::default().fg(Color::Yellow).bold(),
            )),
            Line::from("  /          Search by path"),
            Line::from("  f          Open filter menu"),
            Line::from("  c          Clear all filters"),
            Line::from("  r          Refresh list"),
            Line::from(""),
            Line::from(Span::styled(
                "Actions",
                Style::default().fg(Color::Yellow).bold(),
            )),
            Line::from("  o          Open file"),
            Line::from("  O          Open containing folder"),
            Line::from("  t          Edit tags"),
            Line::from("  n          Edit notes"),
            Line::from("  d          Delete file"),
            Line::from(""),
            Line::from(Span::styled(
                "General",
                Style::default().fg(Color::Yellow).bold(),
            )),
            Line::from("  ?          Toggle this help"),
            Line::from("  q/Esc      Quit / Close overlay"),
            Line::from("  Ctrl+C     Force quit"),
            Line::from(""),
            Line::from(Span::styled(
                "File Types",
                Style::default().fg(Color::Yellow).bold(),
            )),
            Line::from(vec![
                Span::styled("  exec  ", Style::default().fg(Color::Red)),
                Span::raw("Executables (.exe, .sh, binaries)"),
            ]),
            Line::from(vec![
                Span::styled("  arch  ", Style::default().fg(Color::Magenta)),
                Span::raw("Archives (.zip, .tar, .gz)"),
            ]),
            Line::from(vec![
                Span::styled("  doc   ", Style::default().fg(Color::Blue)),
                Span::raw("Documents (.pdf, .doc, .txt)"),
            ]),
            Line::from(vec![
                Span::styled("  media ", Style::default().fg(Color::Green)),
                Span::raw("Media (.jpg, .mp3, .mp4)"),
            ]),
            Line::from(vec![
                Span::styled("  code  ", Style::default().fg(Color::Yellow)),
                Span::raw("Source code (.rs, .py, .js)"),
            ]),
            Line::from(vec![
                Span::styled("  other ", Style::default().fg(Color::Gray)),
                Span::raw("Other files"),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Tips",
                Style::default().fg(Color::Yellow).bold(),
            )),
            Line::from("  • Use tags to organize files"),
            Line::from("  • Notes support any text"),
            Line::from("  • Filters can be combined"),
            Line::from("  • Press 'r' to see new files"),
        ];

        let help = Paragraph::new(help_text)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0))
            .block(
                Block::default()
                    .title(" Help (↑↓ to scroll, q to close) ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );

        frame.render_widget(help, overlay_area);
    }
}

impl Default for HelpOverlay {
    fn default() -> Self {
        Self::new()
    }
}
