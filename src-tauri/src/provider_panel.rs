use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::time::Duration;
use tokio::sync::oneshot;

// ─── Theme ───────────────────────────────────────────────────────────────────

#[allow(dead_code)]
struct Theme {
    bg: Color,
    surface: Color,
    selected_bg: Color,
    accent: Color,
    current: Color,
    text: Color,
    text_dim: Color,
    border: Color,
    border_focus: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(11, 15, 25),
            surface: Color::Rgb(17, 24, 39),
            selected_bg: Color::Rgb(55, 65, 81),
            accent: Color::Rgb(139, 92, 246),
            current: Color::Rgb(52, 211, 153),
            text: Color::Rgb(226, 232, 240),
            text_dim: Color::Rgb(115, 115, 128),
            border: Color::Rgb(45, 55, 65),
            border_focus: Color::Rgb(139, 92, 246),
        }
    }
}

// ─── Spinner ─────────────────────────────────────────────────────────────────

const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

// ─── Terminal Setup ──────────────────────────────────────────────────────────

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        orig_hook(panic);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

// ─── Layout Helper ───────────────────────────────────────────────────────────

fn centered_rect(width_pct: u16, height_pct: u16, area: Rect) -> Rect {
    let w = (area.width * width_pct / 100).max(20);
    let h = (area.height * height_pct / 100).max(8);
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    Rect::new(area.x + x, area.y + y, w, h)
}

// ─── Provider Selection Screen ───────────────────────────────────────────────

pub fn select_provider(current: &providers::ProviderKind) -> io::Result<Option<providers::ProviderKind>> {
    let mut terminal = setup_terminal()?;
    let result = run_provider_list(&mut terminal, current);
    restore_terminal(&mut terminal)?;
    result
}

fn run_provider_list(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    current: &providers::ProviderKind,
) -> io::Result<Option<providers::ProviderKind>> {
    let providers_list = providers::ProviderKind::all();
    let mut selected = providers_list
        .iter()
        .position(|p| p.to_string() == current.to_string())
        .unwrap_or(0);
    let theme = Theme::default();

    loop {
        terminal.draw(|f| {
            draw_provider_list(f, &theme, &providers_list, selected, current);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        selected = selected.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        selected = (selected + 1).min(providers_list.len() - 1);
                    }
                    KeyCode::Enter => {
                        if selected < providers_list.len() {
                            return Ok(Some(providers_list[selected].clone()));
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
                    _ => {}
                }
            }
        }
    }
}

fn draw_provider_list(
    f: &mut Frame,
    theme: &Theme,
    providers_list: &[providers::ProviderKind],
    selected: usize,
    current: &providers::ProviderKind,
) {
    let area = centered_rect(70, 75, f.area());

    let items: Vec<ListItem> = providers_list
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let is_current = p.to_string() == current.to_string();
            let marker = if is_current { "◉" } else { "○" };
            let name = p.to_string();
            let url = p.default_base_url();

            let prefix_style = if i == selected {
                Style::default()
                    .fg(theme.current)
                    .add_modifier(Modifier::BOLD)
            } else if is_current {
                Style::default()
                    .fg(theme.current)
            } else {
                Style::default()
                    .fg(theme.text_dim)
            };

            let name_style = if i == selected {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(theme.text)
            };

            let url_style = if i == selected {
                Style::default().fg(Color::Rgb(148, 163, 184))
            } else {
                Style::default().fg(theme.text_dim)
            };

            let line = Line::from(vec![
                Span::styled(format!(" {}  ", marker), prefix_style),
                Span::styled(format!("{:<12}", name), name_style),
                Span::styled(format!("  {}", url), url_style),
            ]);

            let style = if i == selected {
                Style::default().bg(theme.selected_bg)
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title(" Provider Setup ")
            .title_style(
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
    );

    let mut list_state = ListState::default();
    list_state.select(Some(selected));

    f.render_stateful_widget(list, area, &mut list_state);

    // Footer hints
    let footer_area = Rect {
        x: area.x,
        y: area.y + area.height,
        width: area.width,
        height: 1,
    };

    let hints = Line::from(vec![
        Span::styled(" ↑/j ↓/k ", Style::default().fg(theme.accent)),
        Span::styled("navigate   ", Style::default().fg(theme.text_dim)),
        Span::styled("Enter ", Style::default().fg(theme.current)),
        Span::styled("select   ", Style::default().fg(theme.text_dim)),
        Span::styled("q/ESC ", Style::default().fg(Color::Rgb(248, 113, 113))),
        Span::styled("cancel", Style::default().fg(theme.text_dim)),
    ]);

    let footer = Paragraph::new(hints);
    f.render_widget(footer, footer_area);
}

// ─── Model Selection Screen ──────────────────────────────────────────────────

pub async fn select_model(cfg: &providers::ProviderConfig) -> anyhow::Result<Option<String>> {
    let mut terminal = setup_terminal()?;
    let result = run_model_panel(&mut terminal, cfg).await;
    restore_terminal(&mut terminal)?;
    result
}

enum ModelState {
    Loading {
        tick: u8,
        rx: oneshot::Receiver<Result<Vec<providers::ModelInfo>, String>>,
    },
    Loaded {
        models: Vec<providers::ModelInfo>,
        selected: usize,
        list_state: ListState,
    },
}

async fn run_model_panel(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    cfg: &providers::ProviderConfig,
) -> anyhow::Result<Option<String>> {
    let provider_name = cfg.kind.to_string();
    let base_url = cfg
        .base_url
        .clone()
        .unwrap_or_else(|| cfg.kind.default_base_url());

    let (tx, rx) = oneshot::channel();
    let fetch_cfg = cfg.clone();

    tokio::spawn(async move {
        let result = providers::fetch_models(&fetch_cfg).await;
        let _ = tx.send(result);
    });

    let mut state = ModelState::Loading { tick: 0, rx };
    let theme = Theme::default();

    loop {
        terminal.draw(|f| {
            let area = centered_rect(65, 55, f.area());
            match &mut state {
                ModelState::Loading { tick, .. } => {
                    draw_spinner(f, &theme, area, &provider_name, &base_url, *tick);
                }
                ModelState::Loaded { models, list_state, .. } => {
                    draw_model_list(f, &theme, area, &provider_name, models, list_state);
                }
            }
        })?;

        // Advance spinner
        if let ModelState::Loading { tick, .. } = &mut state {
            *tick = (*tick + 1) % (SPINNER.len() as u8);
        }

        // Check if models arrived
        if let ModelState::Loading { rx, .. } = &mut state {
            match rx.try_recv() {
                Ok(Ok(models)) => {
                    if models.is_empty() {
                        return Err(anyhow::anyhow!("No models returned from provider"));
                    }
                    let mut list_state = ListState::default();
                    list_state.select(Some(0));
                    state = ModelState::Loaded {
                        models,
                        selected: 0,
                        list_state,
                    };
                }
                Ok(Err(e)) => {
                    return Err(anyhow::anyhow!("Failed to fetch models: {}", e));
                }
                Err(oneshot::error::TryRecvError::Empty) => {}
                Err(oneshot::error::TryRecvError::Closed) => {
                    return Err(anyhow::anyhow!("Model fetch channel closed unexpectedly"));
                }
            }
        }

        // Handle key events
        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
                    _ => {}
                }

                if let ModelState::Loaded {
                    models,
                    selected,
                    list_state,
                } = &mut state
                {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            *selected = selected.saturating_sub(1);
                            list_state.select(Some(*selected));
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            *selected = (*selected + 1).min(models.len().saturating_sub(1));
                            list_state.select(Some(*selected));
                        }
                        KeyCode::Enter => {
                            if *selected < models.len() {
                                return Ok(Some(models[*selected].id.clone()));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn draw_spinner(
    f: &mut Frame,
    theme: &Theme,
    area: Rect,
    provider_name: &str,
    base_url: &str,
    tick: u8,
) {
    let ch = SPINNER[tick as usize % SPINNER.len()];

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                format!("{}", ch),
                Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  Connecting to {}...", provider_name),
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("        ", Style::default()),
            Span::styled(base_url, Style::default().fg(theme.text_dim)),
        ]),
        Line::from(""),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(format!(" Fetching models — {} ", provider_name))
        .title_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        );

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .style(Style::default().fg(theme.text).bg(theme.bg));

    f.render_widget(paragraph, inner[0]);

    // Footer
    let hints = Line::from(vec![
        Span::styled(" q/ESC ", Style::default().fg(Color::Rgb(248, 113, 113))),
        Span::styled("cancel", Style::default().fg(theme.text_dim)),
    ]);
    f.render_widget(Paragraph::new(hints), inner[1]);
}

fn draw_model_list(
    f: &mut Frame,
    theme: &Theme,
    area: Rect,
    provider_name: &str,
    models: &[providers::ModelInfo],
    list_state: &ListState,
) {
    let items: Vec<ListItem> = models
        .iter()
        .enumerate()
        .map(|(_i, m)| {
            let display = m.display_name();
            let marker = "○";

            let line = Line::from(vec![
                Span::styled(
                    format!(" {}  ", marker),
                    Style::default().fg(theme.text_dim),
                ),
                Span::styled(
                    display,
                    Style::default().fg(theme.text),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title(format!(" Select Model — {} ", provider_name))
            .title_style(
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
    );

    let mut styled_state = list_state.clone();
    f.render_stateful_widget(list, area, &mut styled_state);

    // Footer hints
    let footer_area = Rect {
        x: area.x,
        y: area.y + area.height,
        width: area.width,
        height: 1,
    };

    let hints = Line::from(vec![
        Span::styled(" ↑/j ↓/k ", Style::default().fg(theme.accent)),
        Span::styled("navigate   ", Style::default().fg(theme.text_dim)),
        Span::styled("Enter ", Style::default().fg(theme.current)),
        Span::styled("select   ", Style::default().fg(theme.text_dim)),
        Span::styled("q/ESC ", Style::default().fg(Color::Rgb(248, 113, 113))),
        Span::styled("cancel", Style::default().fg(theme.text_dim)),
    ]);

    f.render_widget(Paragraph::new(hints), footer_area);
}
