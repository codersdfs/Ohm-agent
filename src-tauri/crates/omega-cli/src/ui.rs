use crate::app::{App, InputMode};
use crate::message::MessageSender;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

pub struct Theme {
    pub bg: Color,
    pub user: Color,
    pub assistant: Color,
    pub system: Color,
    pub tool: Color,
    pub accent: Color,
    pub input_bg: Color,
    pub input_border: Color,
    pub status_bg: Color,
    pub text: Color,
    pub text_dim: Color,
    pub border: Color,
    pub score_pass: Color,
    pub score_warn: Color,
    pub score_fail: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(18, 18, 22),
            user: Color::Rgb(99, 102, 241),
            assistant: Color::Rgb(52, 211, 153),
            system: Color::Rgb(115, 115, 128),
            tool: Color::Rgb(251, 191, 36),
            accent: Color::Rgb(139, 92, 246),
            input_bg: Color::Rgb(30, 30, 38),
            input_border: Color::Rgb(55, 55, 68),
            status_bg: Color::Rgb(30, 30, 38),
            text: Color::Rgb(226, 232, 240),
            text_dim: Color::Rgb(115, 115, 128),
            border: Color::Rgb(45, 45, 55),
            score_pass: Color::Rgb(52, 211, 153),
            score_warn: Color::Rgb(251, 191, 36),
            score_fail: Color::Rgb(248, 113, 113),
        }
    }
}

const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub fn render_spinner(tick: u8) -> char {
    SPINNER[tick as usize % SPINNER.len()]
}

pub fn draw(frame: &mut Frame, app: &App) {
    let theme = Theme::default();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_chat(frame, app, &theme, chunks[0]);
    render_input(frame, app, &theme, chunks[1]);
    render_status(frame, app, &theme, chunks[2]);
}

fn render_chat(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for msg in app.history.iter() {
        let sender_color = match msg.sender {
            MessageSender::User => theme.user,
            MessageSender::Assistant => theme.assistant,
            MessageSender::System => theme.system,
            MessageSender::Tool => theme.tool,
        };

        let label = format!("[{}] ", msg.sender);
        let cursor = msg.status_cursor();

        let content_lines: Vec<&str> = msg.content.split('\n').collect();
        for (i, content_line) in content_lines.iter().enumerate() {
            if i == 0 {
                let mut spans = vec![
                    Span::styled(
                        label.clone(),
                        Style::default()
                            .fg(sender_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                ];

                if msg.sender == MessageSender::System && content_line.contains("score") {
                    let score_color = if content_line.contains("PASS") {
                        theme.score_pass
                    } else if content_line.contains("WARN") {
                        theme.score_warn
                    } else if content_line.contains("FAIL") {
                        theme.score_fail
                    } else {
                        theme.text
                    };
                    spans.push(Span::styled(
                        format!("{content_line}{cursor}"),
                        Style::default().fg(score_color),
                    ));
                } else {
                    spans.push(Span::styled(
                        format!("{content_line}{cursor}"),
                        Style::default().fg(theme.text),
                    ));
                }

                lines.push(Line::from(spans));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("  {content_line}"),
                    Style::default().fg(theme.text),
                )));
            }
        }
        lines.push(Line::from(""));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Start a conversation. Press i to type.",
            Style::default().fg(theme.text_dim),
        )));
    }

    let line_count = lines.len();
    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .title(" omega-cli ")
                .title_style(
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .wrap(Wrap { trim: false });

    let scroll_offset = app.history.scroll_offset;
    let max_scroll = (line_count as u16).saturating_sub(area.height.saturating_sub(2));
    let clamped_scroll = (scroll_offset as u16).min(max_scroll);

    frame.render_widget(paragraph.scroll((clamped_scroll, 0)), area);
}

fn render_input(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.input_border))
        .title(" Input ")
        .title_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        );

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let input_text = app.input_lines.join("\n");
    let cursor_line = app.cursor_line;
    let cursor_col = app.cursor_col;

    let paragraph = Paragraph::new(Text::raw(&input_text))
        .style(Style::default().fg(theme.text).bg(theme.input_bg));

    frame.render_widget(paragraph, inner);

    let cursor_x = inner.x + (cursor_col as u16).min(inner.width.saturating_sub(1));
    let cursor_y = inner.y + (cursor_line as u16).min(inner.height.saturating_sub(1));

    if app.mode == InputMode::Insert {
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

fn render_status(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let mode_str = match app.mode {
        InputMode::Normal => " NORMAL ",
        InputMode::Insert => " INSERT ",
    };

    let mode_style = match app.mode {
        InputMode::Normal => Style::default()
            .fg(theme.bg)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD),
        InputMode::Insert => Style::default()
            .fg(theme.bg)
            .bg(theme.user)
            .add_modifier(Modifier::BOLD),
    };

    let mut spans = vec![Span::styled(mode_str, mode_style)];

    if app.is_loading {
        let spinner = render_spinner(app.loading_tick);
        spans.push(Span::styled(
            format!(" {spinner} Thinking..."),
            Style::default().fg(theme.assistant),
        ));
    } else {
        spans.push(Span::styled(
            "  ",
            Style::default().fg(theme.text_dim),
        ));
    }

    let right_hint = Span::styled(
        " i:insert  j/k:scroll  Enter:send  Ctrl+C:quit ",
        Style::default().fg(theme.text_dim),
    );

    let width = area.width as usize;
    let left_len: usize = spans.iter().map(|s| s.content.len()).sum();
    let hint_len = right_hint.content.len();
    let padding = width.saturating_sub(left_len + hint_len);
    let padded = format!("{:padding$}", "");

    spans.push(Span::styled(padded, Style::default()));
    spans.push(right_hint);

    let paragraph = Paragraph::new(Line::from(spans))
        .style(Style::default().fg(theme.text).bg(theme.status_bg));

    frame.render_widget(paragraph, area);
}
