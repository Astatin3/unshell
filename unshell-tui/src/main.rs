//! Binary entry point for the UnShell Ratatui interface.
//!
//! This currently boots the terminal shell and renders the shared interface chrome.
//! Leaf attachment and endpoint driving can be added around the same
//! `InterfaceContext` services without changing the core protocol crate.

use std::{io, time::Duration};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};
use unshell::{interface::RatatuiInterface, protocol::LeafMeta};
use unshell_tui::{DefaultRatatuiInterface, MemoryInterfaceDatabase};

/// Runs the TUI and maps terminal errors onto normal I/O errors.
fn main() -> io::Result<()> {
    run_terminal_app()
}

/// Boots terminal mode, runs the draw loop, and restores the terminal on exit.
fn run_terminal_app() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let mut terminal = ratatui::init();
    let mut app = App::new();
    let result = app.run(&mut terminal);

    ratatui::restore();
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    result
}

/// Minimal operator shell state for the first real `unshell-tui` binary.
struct App {
    database: MemoryInterfaceDatabase,
    renderer: DefaultRatatuiInterface,
    meta: LeafMeta,
}

impl App {
    /// Creates the initial TUI state.
    fn new() -> Self {
        Self {
            database: MemoryInterfaceDatabase::new(),
            renderer: DefaultRatatuiInterface::new(),
            meta: LeafMeta {
                name: "UnShell TUI",
                identifier: "dev.unshell.tui",
                version: env!("CARGO_PKG_VERSION"),
                authors: vec!["ASTATIN3"],
            },
        }
    }

    /// Draws until the operator presses `q` or Escape.
    fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;

            if !event::poll(Duration::from_millis(100))? {
                continue;
            }

            let Event::Key(key) = event::read()? else {
                continue;
            };

            if key.kind != KeyEventKind::Press {
                continue;
            }

            if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                break;
            }
        }

        Ok(())
    }

    /// Renders the static shell chrome while endpoint/leaf wiring is still evolving.
    fn render(&mut self, frame: &mut Frame<'_>) {
        let areas = self.renderer.render_leaf_chrome(
            &self.meta,
            0,
            self.database.len(),
            frame,
            frame.area(),
        );

        render_placeholder(
            frame,
            areas.active_sessions,
            "Active Sessions",
            "No endpoint is attached yet. Generated leaves will render live sessions here.",
        );
        render_placeholder(
            frame,
            areas.historical_sessions,
            "Historical Sessions",
            "Serialized session blobs loaded from the interface database will render here.",
        );
    }
}

/// Draws one bounded placeholder panel for an unconnected interface area.
fn render_placeholder(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    message: &'static str,
) {
    let panel = Paragraph::new(Line::from(message))
        .style(Style::new().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(panel, area);
}
