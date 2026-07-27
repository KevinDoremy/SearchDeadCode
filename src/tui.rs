//! Full-screen findings triage (ratatui). The state machine and the
//! rendering are pure and unit-tested against a TestBackend; only the
//! terminal event loop touches a real TTY.

use crate::analysis::DeadCode;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;
use std::path::Path;

/// What a key press means for the loop.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Quit,
}

pub struct TuiApp {
    rows: Vec<RowData>,
    pub selected: usize,
}

struct RowData {
    label: String,
    detail: String,
}

impl TuiApp {
    pub fn new(findings: &[DeadCode], root: &Path) -> Self {
        let rows = findings
            .iter()
            .map(|dc| {
                let rel = dc
                    .declaration
                    .location
                    .file
                    .strip_prefix(root)
                    .unwrap_or(&dc.declaration.location.file);
                RowData {
                    label: format!(
                        "{} {:<30} {}:{}",
                        dc.issue.code(),
                        dc.declaration.name,
                        rel.display(),
                        dc.declaration.location.line
                    ),
                    detail: format!(
                        "{}\n\nkind: {}\nconfidence: {}\nrisk: {}",
                        dc.message,
                        dc.declaration.kind.display_name(),
                        dc.confidence,
                        dc.risk
                    ),
                }
            })
            .collect();
        Self { rows, selected: 0 }
    }

    /// j/down and k/up move the selection, q leaves.
    pub fn on_key(&mut self, key: char) -> Outcome {
        match key {
            'q' => return Outcome::Quit,
            'j' => {
                if self.selected + 1 < self.rows.len() {
                    self.selected += 1;
                }
            }
            'k' => {
                self.selected = self.selected.saturating_sub(1);
            }
            _ => {}
        }
        Outcome::Continue
    }

    pub fn render(&self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(frame.area());

        let items: Vec<ListItem> = self
            .rows
            .iter()
            .map(|row| ListItem::new(Line::raw(row.label.clone())))
            .collect();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!(
                " findings ({}) — j/k move, q quit ",
                self.rows.len()
            )))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        let mut state = ListState::default();
        state.select(Some(self.selected));
        frame.render_stateful_widget(list, chunks[0], &mut state);

        let detail = self
            .rows
            .get(self.selected)
            .map(|row| row.detail.clone())
            .unwrap_or_else(|| "no findings".to_string());
        let panel = Paragraph::new(detail)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" detail "));
        frame.render_widget(panel, chunks[1]);
    }
}

/// Real terminal loop — the only part a test cannot drive.
pub fn run(findings: &[DeadCode], root: &Path) -> std::io::Result<()> {
    use crossterm::event::{self, Event, KeyCode};
    let mut app = TuiApp::new(findings, root);
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| app.render(frame))?;
        if let Event::Key(key) = event::read()? {
            let ch = match key.code {
                KeyCode::Char(c) => c,
                KeyCode::Down => 'j',
                KeyCode::Up => 'k',
                KeyCode::Esc => 'q',
                _ => ' ',
            };
            if app.on_key(ch) == Outcome::Quit {
                break;
            }
        }
    }
    ratatui::restore();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::DeadCodeIssue;
    use crate::graph::{Declaration, DeclarationId, DeclarationKind, Language, Location};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;

    fn finding(name: &str, line: usize) -> DeadCode {
        let path = PathBuf::from("/repo/src/A.kt");
        let decl = Declaration::new(
            DeclarationId::new(path.clone(), line * 100, line * 100 + 10),
            name.to_string(),
            DeclarationKind::Class,
            Location::new(path, line, 1, line * 100, line * 100 + 10),
            Language::Kotlin,
        );
        DeadCode::new(decl, DeadCodeIssue::Unreferenced)
    }

    fn rendered(app: &TuiApp) -> String {
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn the_list_shows_every_finding_and_the_detail_follows_selection() {
        let findings = vec![finding("GhostA", 3), finding("GhostB", 9)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));

        let screen = rendered(&app);
        assert!(screen.contains("GhostA") && screen.contains("GhostB"));
        assert!(
            screen.contains("never used")
                || screen.contains("Unreferenced")
                || screen.contains("class")
        );

        app.on_key('j');
        assert_eq!(app.selected, 1, "j moves down");
        let screen = rendered(&app);
        assert!(screen.contains("findings (2)"));
    }

    #[test]
    fn navigation_clamps_at_both_ends() {
        let findings = vec![finding("OnlyOne", 1)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));
        app.on_key('k');
        assert_eq!(app.selected, 0, "k at the top stays");
        app.on_key('j');
        assert_eq!(app.selected, 0, "j at the bottom stays");
    }

    #[test]
    fn q_quits_and_other_keys_do_not() {
        let findings = vec![finding("GhostA", 3)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));
        assert_eq!(app.on_key('x'), Outcome::Continue);
        assert_eq!(app.on_key('q'), Outcome::Quit);
    }

    #[test]
    fn an_empty_list_renders_without_panicking() {
        let app = TuiApp::new(&[], Path::new("/repo"));
        let screen = rendered(&app);
        assert!(screen.contains("no findings"));
    }
}
