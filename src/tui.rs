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
    /// Hand control back to the caller for a fresh analysis run.
    Refresh,
}

#[derive(PartialEq, Eq)]
enum Mode {
    Normal,
    Filter,
    Help,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortMode {
    File,
    Rule,
    Confidence,
}

impl SortMode {
    fn next(self) -> Self {
        match self {
            SortMode::File => SortMode::Rule,
            SortMode::Rule => SortMode::Confidence,
            SortMode::Confidence => SortMode::File,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SortMode::File => "file",
            SortMode::Rule => "rule",
            SortMode::Confidence => "confidence",
        }
    }
}

pub struct TuiApp {
    rows: Vec<RowData>,
    pub selected: usize,
    mode: Mode,
    query: String,
    sort: SortMode,
}

struct RowData {
    label: String,
    detail: String,
    marked: bool,
    /// position in the findings slice handed to new()
    finding_index: usize,
    sort_file: String,
    sort_rule: String,
    /// higher confidence first
    sort_confidence: std::cmp::Reverse<u32>,
}

impl TuiApp {
    pub fn new(findings: &[DeadCode], root: &Path) -> Self {
        let rows = findings
            .iter()
            .enumerate()
            .map(|(finding_index, dc)| {
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
                    marked: false,
                    finding_index,
                    sort_file: format!("{}:{:08}", rel.display(), dc.declaration.location.line),
                    sort_rule: dc.issue.code().to_string(),
                    sort_confidence: std::cmp::Reverse((dc.confidence.score() * 100.0) as u32),
                }
            })
            .collect();
        let mut app = Self {
            rows,
            selected: 0,
            mode: Mode::Normal,
            query: String::new(),
            sort: SortMode::File,
        };
        app.apply_sort();
        app
    }

    fn apply_sort(&mut self) {
        match self.sort {
            SortMode::File => self.rows.sort_by(|a, b| a.sort_file.cmp(&b.sort_file)),
            SortMode::Rule => self.rows.sort_by(|a, b| {
                a.sort_rule
                    .cmp(&b.sort_rule)
                    .then(a.sort_file.cmp(&b.sort_file))
            }),
            SortMode::Confidence => self.rows.sort_by(|a, b| {
                a.sort_confidence
                    .cmp(&b.sort_confidence)
                    .then(a.sort_file.cmp(&b.sort_file))
            }),
        }
        self.selected = 0;
    }

    /// Indices of the rows the current filter lets through.
    fn visible(&self) -> Vec<usize> {
        let needle = self.query.to_lowercase();
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| needle.is_empty() || row.label.to_lowercase().contains(&needle))
            .map(|(i, _)| i)
            .collect()
    }

    /// Input positions of every marked finding, in input order.
    pub fn marked_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self
            .rows
            .iter()
            .filter(|row| row.marked)
            .map(|row| row.finding_index)
            .collect();
        indices.sort_unstable();
        indices
    }

    pub fn on_escape(&mut self) {
        self.mode = Mode::Normal;
        self.query.clear();
        self.selected = 0;
    }

    pub fn on_backspace(&mut self) {
        if self.mode == Mode::Filter {
            self.query.pop();
            self.selected = 0;
        }
    }

    /// Normal mode: j/k move, / filters, q leaves. Filter mode: every
    /// printable character narrows the query.
    pub fn on_key(&mut self, key: char) -> Outcome {
        if self.mode == Mode::Filter {
            if !key.is_control() {
                self.query.push(key);
                self.selected = 0;
            }
            return Outcome::Continue;
        }
        if self.mode == Mode::Help {
            self.mode = Mode::Normal; // any key closes the overlay
            return Outcome::Continue;
        }
        match key {
            'q' => return Outcome::Quit,
            'r' => return Outcome::Refresh,
            '/' => {
                self.mode = Mode::Filter;
                self.query.clear();
                self.selected = 0;
            }
            'j' => {
                if self.selected + 1 < self.visible().len() {
                    self.selected += 1;
                }
            }
            'k' => {
                self.selected = self.selected.saturating_sub(1);
            }
            's' => {
                self.sort = self.sort.next();
                self.apply_sort();
            }
            '?' => {
                self.mode = Mode::Help;
            }
            'b' => {
                let visible = self.visible();
                if let Some(&row) = visible.get(self.selected.min(visible.len().saturating_sub(1)))
                {
                    self.rows[row].marked = !self.rows[row].marked;
                }
            }
            _ => {}
        }
        Outcome::Continue
    }

    pub fn render(&self, frame: &mut Frame) {
        if self.mode == Mode::Help {
            let help = Paragraph::new(
                "keys\n\nj/k  move\n/    filter (esc leaves)\ns    cycle sort\nb    mark for baseline\nr    refresh (re-run the analysis)\n?    this help\nq    quit\n\npress any key to close",
            )
            .block(Block::default().borders(Borders::ALL).title(" help "));
            frame.render_widget(help, frame.area());
            return;
        }
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(frame.area());

        let visible = self.visible();
        let items: Vec<ListItem> = visible
            .iter()
            .map(|&i| {
                let row = &self.rows[i];
                let prefix = if row.marked { "[b] " } else { "    " };
                ListItem::new(Line::raw(format!("{prefix}{}", row.label)))
            })
            .collect();
        let title = if self.mode == Mode::Filter {
            format!(" findings ({}) — filter: {} ", visible.len(), self.query)
        } else {
            format!(
                " findings ({}) — marked: {} — sort: {} — / s b r refresh q — ? help ",
                visible.len(),
                self.rows.iter().filter(|r| r.marked).count(),
                self.sort.label()
            )
        };
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        let mut state = ListState::default();
        state.select(Some(self.selected.min(visible.len().saturating_sub(1))));
        frame.render_stateful_widget(list, chunks[0], &mut state);

        let detail = visible
            .get(self.selected.min(visible.len().saturating_sub(1)))
            .and_then(|&i| self.rows.get(i))
            .map(|row| row.detail.clone())
            .unwrap_or_else(|| "no findings".to_string());
        let panel = Paragraph::new(detail)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" detail "));
        frame.render_widget(panel, chunks[1]);
    }
}

/// Add the marked findings to the baseline file, creating it when
/// absent, skipping fingerprints already recorded. Returns how many
/// entries were actually added.
pub fn append_to_baseline(
    findings: &[DeadCode],
    indices: &[usize],
    root: &Path,
    baseline_path: &Path,
) -> std::io::Result<usize> {
    use crate::baseline::{Baseline, IssueFingerprint};
    let mut baseline = if baseline_path.exists() {
        Baseline::load(baseline_path).map_err(std::io::Error::other)?
    } else {
        Baseline::from_findings(&[], root)
    };
    let mut added = 0usize;
    for &index in indices {
        let Some(dc) = findings.get(index) else {
            continue;
        };
        if baseline.is_baselined(dc, root) {
            continue;
        }
        baseline
            .issues
            .push(IssueFingerprint::from_dead_code(dc, root));
        added += 1;
    }
    if added > 0 {
        baseline
            .save(baseline_path)
            .map_err(std::io::Error::other)?;
    }
    Ok(added)
}

/// How the terminal loop ended: a normal quit, or a refresh request
/// the caller honors by re-running the whole analysis.
#[derive(PartialEq, Eq, Debug)]
pub enum Exit {
    Quit,
    Refresh,
}

/// Real terminal loop — the only part a test cannot drive.
pub fn run(findings: &[DeadCode], root: &Path, baseline: Option<&Path>) -> std::io::Result<Exit> {
    use crossterm::event::{self, Event, KeyCode};
    let mut app = TuiApp::new(findings, root);
    let mut terminal = ratatui::init();
    let mut exit = Exit::Quit;
    loop {
        terminal.draw(|frame| app.render(frame))?;
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => app.on_escape(),
                KeyCode::Backspace => app.on_backspace(),
                other => {
                    let ch = match other {
                        KeyCode::Char(c) => c,
                        KeyCode::Down => 'j',
                        KeyCode::Up => 'k',
                        KeyCode::Enter => ' ',
                        _ => ' ',
                    };
                    match app.on_key(ch) {
                        Outcome::Quit => break,
                        Outcome::Refresh => {
                            exit = Exit::Refresh;
                            break;
                        }
                        Outcome::Continue => {}
                    }
                }
            }
        }
    }
    ratatui::restore();
    if let Some(baseline_path) = baseline {
        let marked = app.marked_indices();
        if !marked.is_empty() {
            let added = append_to_baseline(findings, &marked, root, baseline_path)?;
            println!(
                "added {added} finding(s) to {} ({} marked)",
                baseline_path.display(),
                marked.len()
            );
        }
    }
    Ok(exit)
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
    fn slash_enters_filter_mode_and_typing_narrows_the_list() {
        let findings = vec![
            finding("GhostAlpha", 3),
            finding("GhostBeta", 9),
            finding("Zombie", 12),
        ];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));

        app.on_key('/');
        for c in "ghost".chars() {
            app.on_key(c);
        }
        let screen = rendered(&app);
        assert!(
            screen.contains("GhostAlpha") && screen.contains("GhostBeta"),
            "both ghosts match, screen was:\n{screen}"
        );
        assert!(
            !screen.contains("Zombie"),
            "the zombie is filtered out, screen was:\n{screen}"
        );
        assert!(
            screen.contains("ghost"),
            "the query is visible, screen was:\n{screen}"
        );
    }

    #[test]
    fn escape_leaves_filter_mode_and_restores_the_full_list() {
        let findings = vec![finding("GhostAlpha", 3), finding("Zombie", 12)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));

        app.on_key('/');
        for c in "ghost".chars() {
            app.on_key(c);
        }
        app.on_escape();
        let screen = rendered(&app);
        assert!(
            screen.contains("Zombie"),
            "escape clears the filter, screen was:\n{screen}"
        );
    }

    #[test]
    fn q_filters_instead_of_quitting_while_in_filter_mode() {
        let findings = vec![finding("Quokka", 3)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));
        app.on_key('/');
        assert_eq!(
            app.on_key('q'),
            Outcome::Continue,
            "q is a letter inside the filter"
        );
        let screen = rendered(&app);
        assert!(screen.contains("Quokka"), "quokka matches 'q':\n{screen}");
    }

    #[test]
    fn backspace_widens_the_filter_again() {
        let findings = vec![finding("GhostAlpha", 3), finding("Zombie", 12)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));
        app.on_key('/');
        for c in "ghostx".chars() {
            app.on_key(c);
        }
        let screen = rendered(&app);
        assert!(!screen.contains("GhostAlpha"), "'ghostx' matches nothing");
        app.on_backspace();
        let screen = rendered(&app);
        assert!(
            screen.contains("GhostAlpha"),
            "one backspace back to 'ghost', screen was:\n{screen}"
        );
    }

    #[test]
    fn s_cycles_the_sort_order() {
        // two files, codes chosen so file-order and code-order differ
        let mut zz = finding("ZzLate", 2);
        zz.declaration.location.file = std::path::PathBuf::from("/repo/src/Zz.kt");
        zz.declaration.id.file = std::path::PathBuf::from("/repo/src/Zz.kt");
        let aa = finding("AaEarly", 8);
        let findings = vec![zz, aa];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));

        // default: file order — Zz.kt was given first but A.kt sorts first
        let screen = rendered(&app);
        let a_pos = screen.find("AaEarly").unwrap();
        let z_pos = screen.find("ZzLate").unwrap();
        assert!(a_pos < z_pos, "file order puts A.kt first:\n{screen}");

        app.on_key('s');
        let screen = rendered(&app);
        assert!(
            screen.contains("sort: rule") || screen.contains("sort: code"),
            "the sort mode is visible after s:\n{screen}"
        );

        app.on_key('s');
        let screen = rendered(&app);
        assert!(
            screen.contains("sort: confidence"),
            "second s reaches confidence:\n{screen}"
        );

        app.on_key('s');
        let screen = rendered(&app);
        assert!(
            screen.contains("sort: file"),
            "third s cycles back to file:\n{screen}"
        );
    }

    #[test]
    fn typing_s_in_filter_mode_filters_instead_of_sorting() {
        let findings = vec![finding("Session", 3), finding("Ghost", 9)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));
        app.on_key('/');
        for c in "sess".chars() {
            app.on_key(c);
        }
        let screen = rendered(&app);
        assert!(
            screen.contains("Session") && !screen.contains("Ghost"),
            "'sess' narrows to Session inside the filter (and did not sort):\n{screen}"
        );
    }

    #[test]
    fn b_toggles_a_mark_on_the_selected_row() {
        let findings = vec![finding("GhostA", 3), finding("GhostB", 9)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));

        app.on_key('b');
        let screen = rendered(&app);
        assert!(
            screen.contains("[b] DC001 GhostA") || screen.contains("[b]"),
            "the mark shows on the selected row:\n{screen}"
        );
        assert!(
            screen.contains("marked: 1"),
            "the title counts marks:\n{screen}"
        );

        app.on_key('b');
        let screen = rendered(&app);
        assert!(!screen.contains("[b]"), "b again unmarks:\n{screen}");
    }

    #[test]
    fn marks_follow_the_finding_through_a_resort() {
        let mut zz = finding("ZzLate", 2);
        zz.declaration.location.file = std::path::PathBuf::from("/repo/src/Zz.kt");
        zz.declaration.id.file = std::path::PathBuf::from("/repo/src/Zz.kt");
        let findings = vec![zz, finding("AaEarly", 8)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));

        // file order puts AaEarly first; mark it, then resort twice
        app.on_key('b');
        app.on_key('s');
        app.on_key('s');
        let screen = rendered(&app);
        let marked_line = screen
            .lines()
            .find(|l| l.contains("[b]"))
            .expect("the mark survived the resort");
        assert!(
            marked_line.contains("AaEarly"),
            "the mark rides the finding, not the slot:\n{screen}"
        );
    }

    #[test]
    fn b_inside_filter_mode_is_just_a_letter() {
        let findings = vec![finding("Bumble", 3), finding("Ghost", 9)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));
        app.on_key('/');
        for c in "bumb".chars() {
            app.on_key(c);
        }
        let screen = rendered(&app);
        assert!(
            screen.contains("Bumble") && !screen.contains("[b]"),
            "no marking from inside the filter:\n{screen}"
        );
    }

    #[test]
    fn marked_indices_point_back_at_the_original_findings() {
        let mut zz = finding("ZzLate", 2);
        zz.declaration.location.file = std::path::PathBuf::from("/repo/src/Zz.kt");
        zz.declaration.id.file = std::path::PathBuf::from("/repo/src/Zz.kt");
        // input order: ZzLate (0), AaEarly (1); file sort shows AaEarly first
        let findings = vec![zz, finding("AaEarly", 8)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));

        app.on_key('b'); // marks AaEarly, which is input index 1
        assert_eq!(
            app.marked_indices(),
            vec![1],
            "the index is the input position, not the display slot"
        );
    }

    #[test]
    fn append_marked_writes_fingerprints_and_deduplicates() {
        let temp = tempfile::tempdir().unwrap();
        let baseline_path = temp.path().join("baseline.json");
        let findings = vec![finding("GhostA", 3), finding("GhostB", 9)];

        let added =
            append_to_baseline(&findings, &[0], Path::new("/repo"), &baseline_path).unwrap();
        assert_eq!(added, 1);
        let json = std::fs::read_to_string(&baseline_path).unwrap();
        assert!(json.contains("GhostA") && !json.contains("GhostB"));

        // appending the same finding again adds nothing
        let added =
            append_to_baseline(&findings, &[0], Path::new("/repo"), &baseline_path).unwrap();
        assert_eq!(added, 0, "already baselined, nothing to add");

        // a second finding lands next to the first
        let added =
            append_to_baseline(&findings, &[0, 1], Path::new("/repo"), &baseline_path).unwrap();
        assert_eq!(added, 1);
        let json = std::fs::read_to_string(&baseline_path).unwrap();
        assert!(json.contains("GhostA") && json.contains("GhostB"));
    }

    #[test]
    fn question_mark_opens_the_help_and_any_key_closes_it() {
        let findings = vec![finding("GhostA", 3)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));

        app.on_key('?');
        let screen = rendered(&app);
        assert!(
            screen.contains("help") || screen.contains("keys"),
            "the help overlay is visible:\n{screen}"
        );
        assert!(
            screen.contains("b ") && screen.contains("mark"),
            "the keys are explained:\n{screen}"
        );

        assert_eq!(
            app.on_key('q'),
            Outcome::Continue,
            "any key closes the help instead of acting"
        );
        let screen = rendered(&app);
        assert!(
            !screen.contains("j/k  move"),
            "the overlay is gone:\n{screen}"
        );
        // and q now quits again from normal mode
        assert_eq!(app.on_key('q'), Outcome::Quit);
    }

    #[test]
    fn question_mark_inside_filter_mode_is_just_a_character() {
        let findings = vec![finding("GhostA", 3)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));
        app.on_key('/');
        app.on_key('?');
        let screen = rendered(&app);
        assert!(
            screen.contains("filter: ?"),
            "? lands in the query, no overlay:\n{screen}"
        );
    }

    #[test]
    fn an_empty_list_renders_without_panicking() {
        let app = TuiApp::new(&[], Path::new("/repo"));
        let screen = rendered(&app);
        assert!(screen.contains("no findings"));
    }

    #[test]
    fn pressing_r_asks_for_a_fresh_analysis() {
        let findings = vec![finding("Ghost", 3)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));
        assert_eq!(
            app.on_key('r'),
            Outcome::Refresh,
            "r hands control back for a re-analysis"
        );
    }

    #[test]
    fn r_typed_into_the_filter_stays_a_filter_character() {
        let findings = vec![finding("Ghost", 3)];
        let mut app = TuiApp::new(&findings, Path::new("/repo"));
        app.on_key('/');
        assert_eq!(
            app.on_key('r'),
            Outcome::Continue,
            "while filtering, r is just a letter"
        );
    }

    #[test]
    fn the_footer_advertises_the_refresh_key() {
        let findings = vec![finding("Ghost", 3)];
        let app = TuiApp::new(&findings, Path::new("/repo"));
        let screen = rendered(&app);
        assert!(
            screen.contains("r refresh"),
            "the key is discoverable:\n{screen}"
        );
    }
}
