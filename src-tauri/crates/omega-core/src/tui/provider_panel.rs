use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

use super::theme;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// Actions the panel can return to the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelAction {
    Close,
    Apply,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelFocus {
    ProviderGrid,
    ModelField,
    BaseUrlField,
    MaxTokens,
    Temperature,
    ApplyButton,
}

pub struct ProviderPanelState {
    pub visible: bool,
    pub focus: PanelFocus,
    pub selected_provider: usize,
    pub model_buffer: String,
    pub model_cursor: usize,
    pub url_buffer: String,
    pub url_cursor: usize,
    pub max_tokens: u32,
    pub temperature: f32,
    pub provider_scroll: usize,
}

impl ProviderPanelState {
    pub fn from_config(config: &providers::ProviderConfig) -> Self {
        let all = providers::ProviderKind::all();
        let selected = all
            .iter()
            .position(|k| std::mem::discriminant(k) == std::mem::discriminant(&config.kind))
            .unwrap_or(0);
        Self {
            visible: true,
            focus: PanelFocus::ProviderGrid,
            selected_provider: selected,
            model_buffer: config.model.clone(),
            model_cursor: config.model.len(),
            url_buffer: config.base_url.clone().unwrap_or_else(|| config.kind.default_base_url()),
            url_cursor: config.base_url.as_deref().unwrap_or("").len(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            provider_scroll: 0,
        }
    }

    pub fn to_config(&self, original: &providers::ProviderConfig) -> providers::ProviderConfig {
        let all = providers::ProviderKind::all();
        let kind = all.get(self.selected_provider).cloned().unwrap_or(original.kind.clone());
        providers::ProviderConfig {
            kind,
            api_key: original.api_key.clone(),
            base_url: Some(self.url_buffer.clone()).filter(|s| !s.is_empty()),
            model: if self.model_buffer.is_empty() { original.model.clone() } else { self.model_buffer.clone() },
            max_tokens: self.max_tokens,
            temperature: self.temperature,
        }
    }
}

// ── Key handler ─────────────────────────────────────────────────────────────

pub fn handle_key(state: &mut ProviderPanelState, key: KeyEvent) -> PanelAction {
    if key.kind != KeyEventKind::Press {
        return PanelAction::None;
    }

    match key.code {
        KeyCode::Esc => return PanelAction::Close,
        KeyCode::Tab => {
            state.focus = match state.focus {
                PanelFocus::ProviderGrid => PanelFocus::ModelField,
                PanelFocus::ModelField => PanelFocus::BaseUrlField,
                PanelFocus::BaseUrlField => PanelFocus::MaxTokens,
                PanelFocus::MaxTokens => PanelFocus::Temperature,
                PanelFocus::Temperature => PanelFocus::ApplyButton,
                PanelFocus::ApplyButton => PanelFocus::ProviderGrid,
            };
        }
        KeyCode::BackTab => {
            state.focus = match state.focus {
                PanelFocus::ProviderGrid => PanelFocus::ApplyButton,
                PanelFocus::ModelField => PanelFocus::ProviderGrid,
                PanelFocus::BaseUrlField => PanelFocus::ModelField,
                PanelFocus::MaxTokens => PanelFocus::BaseUrlField,
                PanelFocus::Temperature => PanelFocus::MaxTokens,
                PanelFocus::ApplyButton => PanelFocus::Temperature,
            };
        }
        KeyCode::Up => match state.focus {
            PanelFocus::ProviderGrid if state.selected_provider > 0 => {
                state.selected_provider -= 1;
                if state.selected_provider < state.provider_scroll {
                    state.provider_scroll = state.provider_scroll.saturating_sub(1);
                }
            }
            PanelFocus::MaxTokens => state.max_tokens = state.max_tokens.saturating_add(512),
            PanelFocus::Temperature => state.temperature = (state.temperature + 0.1).min(2.0),
            _ => {}
        },
        KeyCode::Down => match state.focus {
            PanelFocus::ProviderGrid => {
                let max = providers::ProviderKind::all().len() - 1;
                if state.selected_provider < max {
                    state.selected_provider += 1;
                }
                if state.selected_provider >= state.provider_scroll + 5 {
                    state.provider_scroll = state.provider_scroll.saturating_add(1).min(max.saturating_sub(4));
                }
            }
            PanelFocus::MaxTokens => state.max_tokens = state.max_tokens.saturating_sub(512).max(1),
            PanelFocus::Temperature => state.temperature = (state.temperature - 0.1).max(0.0),
            _ => {}
        },
        KeyCode::Left => match state.focus {
            PanelFocus::ProviderGrid if state.selected_provider > 2 => {
                state.selected_provider -= 3;
            }
            PanelFocus::ModelField | PanelFocus::BaseUrlField => {
                let cursor = state.current_cursor();
                if cursor > 0 {
                    let prev = state.current_buffer()[..cursor].char_indices().last().map(|(i, _)| i).unwrap_or(0);
                    state.set_cursor(prev);
                }
            }
            PanelFocus::MaxTokens => state.max_tokens = state.max_tokens.saturating_sub(512).max(1),
            PanelFocus::Temperature => state.temperature = (state.temperature - 0.1).max(0.0),
            _ => {}
        },
        KeyCode::Right => match state.focus {
            PanelFocus::ProviderGrid => {
                let max = providers::ProviderKind::all().len() - 1;
                if state.selected_provider + 3 <= max {
                    state.selected_provider += 3;
                }
            }
            PanelFocus::ModelField | PanelFocus::BaseUrlField => {
                let buf = state.current_buffer().to_string();
                let cursor = state.current_cursor();
                if cursor < buf.len() {
                    let next = buf[cursor..].char_indices().nth(1);
                    if let Some((i, _)) = next {
                        state.set_cursor(cursor + i);
                    } else {
                        state.set_cursor(buf.len());
                    }
                }
            }
            PanelFocus::MaxTokens => state.max_tokens = state.max_tokens.saturating_add(512),
            PanelFocus::Temperature => state.temperature = (state.temperature + 0.1).min(2.0),
            _ => {}
        },
        KeyCode::Enter => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                return PanelAction::Apply;
            }
            match state.focus {
                PanelFocus::ApplyButton => return PanelAction::Apply,
                _ => {
                    state.focus = match state.focus {
                        PanelFocus::ProviderGrid => PanelFocus::ModelField,
                        PanelFocus::ModelField => PanelFocus::BaseUrlField,
                        PanelFocus::BaseUrlField => PanelFocus::MaxTokens,
                        PanelFocus::MaxTokens => PanelFocus::Temperature,
                        PanelFocus::Temperature => PanelFocus::ApplyButton,
                        PanelFocus::ApplyButton => PanelFocus::ProviderGrid,
                    };
                }
            }
        }
        KeyCode::Home => {
            if matches!(state.focus, PanelFocus::ModelField | PanelFocus::BaseUrlField) {
                state.set_cursor(0);
            }
        }
        KeyCode::End => {
            if matches!(state.focus, PanelFocus::ModelField | PanelFocus::BaseUrlField) {
                state.set_cursor(state.current_buffer().len());
            }
        }
        KeyCode::Backspace => {
            if matches!(state.focus, PanelFocus::ModelField | PanelFocus::BaseUrlField) {
                let cursor = state.current_cursor();
                if cursor > 0 {
                    let prev = state.current_buffer()[..cursor].char_indices().last().map(|(i, _)| i).unwrap_or(0);
                    let mut s = state.current_buffer().to_string();
                    s.drain(prev..cursor);
                    state.set_buffer(s);
                    state.set_cursor(prev);
                }
            }
        }
        KeyCode::Char(c) => {
            match state.focus {
                PanelFocus::ModelField | PanelFocus::BaseUrlField => {
                    let cursor = state.current_cursor();
                    let mut s = state.current_buffer().to_string();
                    s.insert(cursor, c);
                    state.set_buffer(s);
                    state.set_cursor(cursor + c.len_utf8());
                }
                _ => {}
            }
        }
        _ => {}
    }

    PanelAction::None
}

// ── Helper methods ─────────────────────────────────────────────────────────

impl ProviderPanelState {
    fn current_buffer(&self) -> &str {
        match self.focus {
            PanelFocus::ModelField | PanelFocus::ProviderGrid => &self.model_buffer,
            PanelFocus::BaseUrlField => &self.url_buffer,
            _ => "",
        }
    }

    fn current_cursor(&self) -> usize {
        match self.focus {
            PanelFocus::ModelField => self.model_cursor,
            PanelFocus::BaseUrlField => self.url_cursor,
            _ => 0,
        }
    }

    fn set_cursor(&mut self, pos: usize) {
        match self.focus {
            PanelFocus::ModelField => self.model_cursor = pos.min(self.model_buffer.len()),
            PanelFocus::BaseUrlField => self.url_cursor = pos.min(self.url_buffer.len()),
            _ => {}
        }
    }

    fn set_buffer(&mut self, buf: String) {
        match self.focus {
            PanelFocus::ModelField => {
                let cursor = self.model_cursor.min(buf.len());
                self.model_buffer = buf;
                self.model_cursor = cursor;
            }
            PanelFocus::BaseUrlField => {
                let cursor = self.url_cursor.min(buf.len());
                self.url_buffer = buf;
                self.url_cursor = cursor;
            }
            _ => {}
        }
    }
}

// ── Render ─────────────────────────────────────────────────────────────────

pub fn render(
    area: Rect,
    buf: &mut Buffer,
    state: &ProviderPanelState,
    config: &providers::ProviderConfig,
) {
    if area.width < 54 || area.height < 22 {
        return;
    }

    let pw = area.width.min(56);  // popup width
    let ph = 19u16;               // popup height (17 content + 2 border)
    let x = area.x + (area.width.saturating_sub(pw)) / 2;
    let y = area.y + (area.height.saturating_sub(ph)) / 2;

    // Dim background
    for cy in area.y..area.y + area.height {
        for cx in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((cx, cy)) {
                cell.set_style(Style::default().fg(theme::DIM));
            }
        }
    }

    // Available width inside block borders
    let iw = (pw - 2) as usize; // inner width

    // ── Build lines ─────────────────────────────────────────────────────
    let mut lines: Vec<Line<'static>> = Vec::new();
    let all = providers::ProviderKind::all();

    // Blank + provider grid
    lines.push(Line::from(""));
    for chunk in all.chunks(3) {
        let mut spans = vec![Span::raw(" ")];
        for (ci, kind) in chunk.iter().enumerate() {
            let idx = all.iter()
                .position(|k| std::mem::discriminant(k) == std::mem::discriminant(kind))
                .unwrap_or(0);
            let sel = idx == state.selected_provider;
            let foc = state.focus == PanelFocus::ProviderGrid && sel;
            let style = if foc {
                Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)
            } else if sel {
                Style::default().fg(theme::ACCENT)
            } else {
                theme::style_dim()
            };
            let label = format!("{}{}", if sel { "◉" } else { "○" }, kind);
            spans.push(Span::styled(label, style));
            let kind_str = format!("{}", kind);
            let pad = 17usize.saturating_sub(kind_str.len() + 1);
            if ci < 2 {
                spans.push(Span::raw(" ".repeat(pad)));
            }
        }
        lines.push(Line::from(spans));
    }

    // Model field — single line with focus indicator
    lines.push(Line::from(""));
    let m_foc = state.focus == PanelFocus::ModelField;
    let m_label = if m_foc { "▸ Model " } else { "  Model " };
    let model_display = if state.model_buffer.is_empty() {
        String::from("enter model name…")
    } else {
        state.model_buffer.clone()
    };
    let m_avail = iw.saturating_sub(m_label.len() + 3); // space for ":" and cursor
    let m_trunc: String = if model_display.len() > m_avail {
        format!("…{}", &model_display[model_display.len().saturating_sub(m_avail - 1)..])
    } else {
        model_display
    };
    let m_style = if m_foc {
        Style::default().fg(theme::ACCENT)
    } else {
        theme::style_dim()
    };
    let m_span: Span<'static> = if m_foc {
        Span::styled(m_trunc, Style::default().fg(theme::FG))
    } else {
        Span::styled(m_trunc, theme::style_dim())
    };
    lines.push(Line::from(vec![
        Span::styled(m_label, m_style),
        m_span,
    ]));

    // URL field
    let u_foc = state.focus == PanelFocus::BaseUrlField;
    let u_label = if u_foc { "▸ URL " } else { "  URL " };
    let url_text = if state.url_buffer.is_empty() {
        "enter base URL…".to_string()
    } else {
        let u_avail = iw.saturating_sub(u_label.len() + 3);
        if state.url_buffer.len() > u_avail {
            format!("…{}", &state.url_buffer[state.url_buffer.len().saturating_sub(u_avail - 1)..])
        } else {
            state.url_buffer.clone()
        }
    };
    let u_style = if u_foc {
        Style::default().fg(theme::ACCENT)
    } else {
        theme::style_dim()
    };
    let u_span: Span<'static> = if u_foc {
        Span::styled(url_text, Style::default().fg(theme::FG))
    } else {
        Span::styled(url_text, theme::style_dim())
    };
    lines.push(Line::from(vec![
        Span::styled(u_label, u_style),
        u_span,
    ]));

    // API key status + numeric fields (one line)
    lines.push(Line::from(""));
    let key_display = if config.api_key.as_deref().filter(|k| !k.is_empty()).is_some() {
        "● ● ● ● ● ● ● ● ● ●"
    } else {
        "— not set —"
    };
    let key_color = if config.api_key.as_deref().filter(|k| !k.is_empty()).is_some() {
        theme::SUCCESS
    } else {
        theme::ERROR
    };
    let tkn_foc = state.focus == PanelFocus::MaxTokens;
    let tmp_foc = state.focus == PanelFocus::Temperature;
    let tkn_lbl = if tkn_foc { "▸" } else { " " };
    let tmp_lbl = if tmp_foc { "▸" } else { " " };
    lines.push(Line::from(vec![
        Span::styled("Key ", theme::style_dim()),
        Span::styled(key_display, Style::default().fg(key_color)),
        Span::raw("  "),
        Span::styled(format!("{}Max:", tkn_lbl), if tkn_foc { Style::default().fg(theme::ACCENT) } else { theme::style_dim() }),
        Span::styled(format!("{}", state.max_tokens), if tkn_foc { Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD) } else { Style::default().fg(theme::FG) }),
        Span::raw("  "),
        Span::styled(format!("{}Temp:", tmp_lbl), if tmp_foc { Style::default().fg(theme::ACCENT) } else { theme::style_dim() }),
        Span::styled(format!("{:.1}", state.temperature), if tmp_foc { Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD) } else { Style::default().fg(theme::FG) }),
    ]));

    // Apply button + hints
    lines.push(Line::from(""));
    let b_foc = state.focus == PanelFocus::ApplyButton;
    lines.push(Line::from(vec![
        Span::styled(if b_foc { "▸ Apply (Ctrl+Enter)" } else { "  Apply (Ctrl+Enter)" },
            if b_foc { Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD) } else { theme::style_dim() }
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Tab/↑↓ cycle · Esc cancel", theme::style_dim()),
    ]));

    // ── Render ──────────────────────────────────────────────────────────
    let text = Text::from(lines);
    let para = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::ACCENT))
                .title(" Provider ")
                .title_style(Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(theme::BG)),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });
    para.render(Rect::new(x, y, pw, ph), buf);
}
