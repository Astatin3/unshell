use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use unshell::{
    interface::{RatatuiInterface, RatatuiLeafAreas},
    protocol::LeafMeta,
};

/// Visual settings used by [`DefaultRatatuiInterface`].
///
/// The core interface accepts a renderer trait object rather than a theme object, so
/// theming belongs here in the TUI layer. Callers can either customize this simple
/// value or replace the whole renderer with their own [`RatatuiInterface`]
/// implementation.
#[derive(Debug, Clone, Copy)]
pub struct InterfaceTheme {
    /// Border and title color for the leaf chrome.
    pub accent: Color,

    /// Color used for secondary counters and identifiers.
    pub muted: Color,

    /// Style used for the leaf display name.
    pub title: Style,
}

impl Default for InterfaceTheme {
    fn default() -> Self {
        Self {
            accent: Color::Cyan,
            muted: Color::Gray,
            title: Style::new().fg(Color::White).add_modifier(Modifier::BOLD),
        }
    }
}

/// Default Ratatui renderer for generated leaf interface passes.
///
/// It draws a compact header with leaf metadata and session counts, then reserves two
/// child areas: one for active sessions and one for historical sessions loaded from the
/// interface database. Typed session widgets are still rendered by each leaf/session;
/// this struct only owns shared TUI chrome and layout policy.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultRatatuiInterface {
    theme: InterfaceTheme,
}

impl DefaultRatatuiInterface {
    /// Creates a renderer with the default theme.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a renderer with caller-supplied colors and styles.
    pub fn with_theme(theme: InterfaceTheme) -> Self {
        Self { theme }
    }
}

impl RatatuiInterface for DefaultRatatuiInterface {
    fn render_leaf_chrome(
        &mut self,
        meta: &LeafMeta,
        active_session_count: usize,
        historical_session_count: usize,
        frame: &mut Frame<'_>,
        area: Rect,
    ) -> RatatuiLeafAreas {
        let outer = Block::default()
            .title(Line::from(vec![
                Span::styled(meta.name, self.theme.title),
                Span::raw(" "),
                Span::styled(meta.version, Style::new().fg(self.theme.muted)),
            ]))
            .borders(Borders::ALL)
            .border_style(Style::new().fg(self.theme.accent));
        let inner = outer.inner(area);

        frame.render_widget(outer, area);

        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Percentage(55),
                Constraint::Percentage(45),
            ])
            .split(inner);

        let authors = if meta.authors.is_empty() {
            "unknown".to_owned()
        } else {
            meta.authors.join(", ")
        };

        let summary = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("id ", Style::new().fg(self.theme.muted)),
                Span::raw(meta.identifier),
                Span::styled("  authors ", Style::new().fg(self.theme.muted)),
                Span::raw(authors),
            ]),
            Line::from(vec![
                Span::styled("active ", Style::new().fg(self.theme.muted)),
                Span::raw(active_session_count.to_string()),
                Span::styled("  historical ", Style::new().fg(self.theme.muted)),
                Span::raw(historical_session_count.to_string()),
            ]),
        ]);
        frame.render_widget(summary, vertical[0]);

        RatatuiLeafAreas {
            active_sessions: vertical[1],
            historical_sessions: vertical[2],
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    #[test]
    fn renderer_splits_leaf_area_for_generated_session_widgets() {
        let backend = TestBackend::new(60, 16);
        let mut terminal = Terminal::new(backend).expect("test backend creates terminal");
        let mut renderer = DefaultRatatuiInterface::new();
        let meta = LeafMeta {
            name: "PTY",
            identifier: "dev.unshell.pty",
            version: "0.1.0",
            authors: vec!["ASTATIN3"],
        };

        terminal
            .draw(|frame| {
                let areas = renderer.render_leaf_chrome(&meta, 2, 3, frame, frame.area());

                assert!(areas.active_sessions.height > 0);
                assert!(areas.historical_sessions.height > 0);
                assert_ne!(areas.active_sessions, areas.historical_sessions);
            })
            .expect("renderer draws on the test backend");
    }
}
