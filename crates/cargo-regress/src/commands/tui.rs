use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Terminal;
use regress_core::causal::CausalEntry;
use regress_core::classify;
use regress_core::classify::BloatCategory;
use regress_core::diff::{group_by_crate, BinaryDiff, CrateGroup};
use regress_core::{binary, diff};

use crate::build;
use crate::cli::DiffArgs;
use crate::commands::diff::{build_causal, read_lock_diff};
use regress_render::terminal::fmt_bytes;

// ── State ────────────────────────────────────────────────────────────────────

#[derive(PartialEq)]
enum Focus {
    Crates,
    Symbols,
}

enum Mode {
    Normal,
    Search,
}

struct App {
    binary_diff: BinaryDiff,
    causal_map: HashMap<String, CausalEntry>,
    all_groups: Vec<CrateGroup>,
    filtered: Vec<usize>,
    crate_state: ListState,
    symbol_state: ListState,
    focus: Focus,
    mode: Mode,
    search: String,
}

impl App {
    fn new(binary_diff: BinaryDiff, causal_entries: Vec<CausalEntry>) -> Self {
        let growing: Vec<_> = binary_diff.all_growing().cloned().collect();
        let all_groups = group_by_crate(&growing);
        let filtered: Vec<usize> = (0..all_groups.len()).collect();

        let causal_map: HashMap<String, CausalEntry> =
            causal_entries.into_iter().map(|e| (e.crate_name.clone(), e)).collect();

        let mut crate_state = ListState::default();
        if !filtered.is_empty() {
            crate_state.select(Some(0));
        }

        Self {
            binary_diff,
            causal_map,
            all_groups,
            filtered,
            crate_state,
            symbol_state: ListState::default(),
            focus: Focus::Crates,
            mode: Mode::Normal,
            search: String::new(),
        }
    }

    fn selected_group_idx(&self) -> Option<usize> {
        let list_idx = self.crate_state.selected()?;
        self.filtered.get(list_idx).copied()
    }

    fn apply_search(&mut self) {
        let q = self.search.to_lowercase();
        self.filtered = (0..self.all_groups.len())
            .filter(|&i| q.is_empty() || self.all_groups[i].name.to_lowercase().contains(&q))
            .collect();
        let sel = if self.filtered.is_empty() { None } else { Some(0) };
        self.crate_state.select(sel);
        self.symbol_state.select(None);
    }

    fn move_up(&mut self) {
        match self.focus {
            Focus::Crates => {
                let n = self.filtered.len();
                if n == 0 { return; }
                let i = self.crate_state.selected().unwrap_or(0);
                self.crate_state.select(Some(if i == 0 { n - 1 } else { i - 1 }));
                self.symbol_state.select(None);
            }
            Focus::Symbols => {
                if let Some(gi) = self.selected_group_idx() {
                    let n = self.all_groups[gi].symbols.len();
                    if n == 0 { return; }
                    let i = self.symbol_state.selected().unwrap_or(0);
                    self.symbol_state.select(Some(if i == 0 { n - 1 } else { i - 1 }));
                }
            }
        }
    }

    fn move_down(&mut self) {
        match self.focus {
            Focus::Crates => {
                let n = self.filtered.len();
                if n == 0 { return; }
                let i = self.crate_state.selected().unwrap_or(0);
                self.crate_state.select(Some((i + 1) % n));
                self.symbol_state.select(None);
            }
            Focus::Symbols => {
                if let Some(gi) = self.selected_group_idx() {
                    let n = self.all_groups[gi].symbols.len();
                    if n == 0 { return; }
                    let i = self.symbol_state.selected().unwrap_or(0);
                    self.symbol_state.select(Some((i + 1) % n));
                }
            }
        }
    }

    fn toggle_focus(&mut self) {
        match self.focus {
            Focus::Crates => {
                let has_syms = self
                    .selected_group_idx()
                    .map(|gi| !self.all_groups[gi].symbols.is_empty())
                    .unwrap_or(false);
                if has_syms {
                    self.symbol_state.select(Some(0));
                    self.focus = Focus::Symbols;
                }
            }
            Focus::Symbols => {
                self.symbol_state.select(None);
                self.focus = Focus::Crates;
            }
        }
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

pub fn run(args: &DiffArgs, repo: &Path) -> Result<()> {
    let from_sha = build::resolve_commit(repo, &args.from)?;
    let to_sha = build::resolve_commit(repo, &args.to)?;

    eprintln!("▶ Building {} ({})…", args.from, &from_sha[..8]);
    let wt_from = build::Worktree::create(repo, &from_sha)?;
    let bin_from = wt_from.build_release(args.bin.as_deref(), args.lib)?;

    eprintln!("▶ Building {} ({})…", args.to, &to_sha[..8]);
    let wt_to = build::Worktree::create(repo, &to_sha)?;
    let bin_to = wt_to.build_release(args.bin.as_deref(), args.lib)?;

    eprintln!("▶ Analysing symbols…");
    let syms_from = binary::parse_symbols(&bin_from)?;
    let syms_to = binary::parse_symbols(&bin_to)?;
    let binary_diff = diff::compute_diff(&syms_from, &syms_to);
    let lock_diff = read_lock_diff(wt_from.root(), wt_to.root());
    let causal_entries = build_causal(&binary_diff, &lock_diff, wt_to.root());

    let mut app = App::new(binary_diff, causal_entries);

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &mut app, args);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

// ── Event loop ───────────────────────────────────────────────────────────────

fn event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    args: &DiffArgs,
) -> Result<()>
where
    B::Error: Send + Sync + 'static,
{
    loop {
        terminal.draw(|f| ui(f, app, args))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match app.mode {
                    Mode::Search => match key.code {
                        KeyCode::Esc => {
                            app.mode = Mode::Normal;
                            app.search.clear();
                            app.apply_search();
                        }
                        KeyCode::Enter => app.mode = Mode::Normal,
                        KeyCode::Backspace => {
                            app.search.pop();
                            app.apply_search();
                        }
                        KeyCode::Char(c) => {
                            app.search.push(c);
                            app.apply_search();
                        }
                        _ => {}
                    },
                    Mode::Normal => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('/') => {
                            app.mode = Mode::Search;
                            app.focus = Focus::Crates;
                        }
                        KeyCode::Tab => app.toggle_focus(),
                        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                        _ => {}
                    },
                }
            }
        }
    }
}

// ── Rendering ────────────────────────────────────────────────────────────────

fn ui(f: &mut ratatui::Frame, app: &mut App, args: &DiffArgs) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    draw_header(f, app, args, chunks[0]);
    draw_body(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);
}

fn draw_header(f: &mut ratatui::Frame, app: &App, args: &DiffArgs, area: Rect) {
    let delta = app.binary_diff.total_delta();
    let pct = app.binary_diff.total_delta_pct();
    let (sign, color) = if delta >= 0 { ("+", Color::Red) } else { ("", Color::Green) };

    let line = Line::from(vec![
        Span::styled(" cargo regress  ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!("{} → {}  ", args.from, args.to)),
        Span::styled(
            format!("{sign}{} ({sign}{:.1}%)", fmt_bytes(delta), pct),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {} regressions", app.all_groups.len()),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    f.render_widget(
        Paragraph::new(line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        area,
    );
}

fn draw_body(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(area);
    draw_crates(f, app, chunks[0]);
    draw_symbols(f, app, chunks[1]);
}

fn draw_crates(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Crates;
    let title = if matches!(app.mode, Mode::Search) {
        format!(" Crates ({}) /{} ", app.filtered.len(), app.search)
    } else {
        format!(" Crates ({}) ", app.filtered.len())
    };

    let items: Vec<ListItem> = if app.filtered.is_empty() {
        let msg = if app.search.is_empty() { "  No regressions" } else { "  No matches" };
        vec![ListItem::new(Span::styled(msg, Style::default().fg(Color::DarkGray)))]
    } else {
        app.filtered
            .iter()
            .map(|&i| {
                let group = &app.all_groups[i];
                let causal = app.causal_map.get(&group.name);
                let result = classify::classify_group(group, causal);
                let cat = match result.category {
                    BloatCategory::Unknown => String::new(),
                    c => format!(" [{c}]"),
                };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:>9}  ", fmt_bytes(group.delta)),
                        Style::default().fg(Color::Red),
                    ),
                    Span::raw(group.name.clone()),
                    Span::styled(cat, Style::default().fg(Color::DarkGray)),
                ]))
            })
            .collect()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style(focused)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut app.crate_state);
}

fn draw_symbols(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Symbols;

    let (title, items) = match app.selected_group_idx() {
        None => (
            " Symbols ".to_string(),
            vec![ListItem::new(Span::styled(
                "  ← select a crate",
                Style::default().fg(Color::DarkGray),
            ))],
        ),
        Some(gi) => {
            let group = &app.all_groups[gi];
            let causal = app.causal_map.get(&group.name);
            let result = classify::classify_group(group, causal);

            let conf = format!(
                " [{}] {}",
                result.category,
                result.confidence_label()
            );
            let title = format!(" Symbols — {}{}  ({}) ", group.name, conf, group.symbols.len());

            let items: Vec<ListItem> = group
                .symbols
                .iter()
                .map(|sym| {
                    let (color, sign) =
                        if sym.delta >= 0 { (Color::Red, "+") } else { (Color::Green, "") };
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{sign}{:>9}  ", fmt_bytes(sym.delta)),
                            Style::default().fg(color),
                        ),
                        Span::raw(truncate(&sym.demangled, 56).to_string()),
                    ]))
                })
                .collect();
            (title, items)
        }
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style(focused)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("  ");

    f.render_stateful_widget(list, area, &mut app.symbol_state);
}

fn draw_footer(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let text = match app.mode {
        Mode::Search => " [Enter] Confirm  [Esc] Cancel  [Backspace] Delete ",
        Mode::Normal => " [↑↓/jk] Navigate  [Tab] Switch panel  [/] Search  [q] Quit ",
    };
    f.render_widget(
        Paragraph::new(text).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut i = max;
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    &s[..i]
}
