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
    /// Close without saving (Esc).
    Close,
    /// Apply config from state and close (Enter on Apply button).
    Apply,
    /// No action.
    None,
}

/// Which section of the panel has keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelFocus {
    ProviderGrid,
    ModelField,
    BaseUrlField,
    MaxTokens,
    Temperature,
    ApplyButton,
}

/// Interactive state for the provider configuration panel.
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
    /// Initialize panel state from an existing provider config.
    pub fn from_config(config: &providers::ProviderConfig) -> Self {
        let all = providers::ProviderKind::all();
        let selected = all.iter().position(|k| std::mem::discriminant(k) == std::mem::discriminant(&config.kind))
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

    /// Build a ProviderConfig from the current panel state, using the original config for API key.
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

/// Process a key event for the provider panel. Returns an action for the caller.
pub fn handle_key(state: &mut ProviderPanelState, key: KeyEvent) -> PanelAction {
    if key.kind != KeyEventKind::Press {
        return PanelAction::None;
    }

    match key.code {
        KeyCode::Esc => {
            return PanelAction::Close;
        }
        KeyCode::Tab => {
            state.focus = match state.focus {
                PanelFocus::ProviderGrid => PanelFocus::ModelField,
                PanelFocus::ModelField => PanelFocus::BaseUrlField,
                PanelFocus::BaseUrlField => PanelFocus::MaxTokens,
                PanelFocus::MaxTokens => PanelFocus::Temperature,
                PanelFocus::Temperature => PanelFocus::ApplyButton,
                PanelFocus::ApplyButton => PanelFocus::ProviderGrid,
            };
            return PanelAction::None;
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
            return PanelAction::None;
        }
        KeyCode::Up => match state.focus {
            PanelFocus::ProviderGrid => {
                if state.selected_provider > 0 {
                    state.selected_provider -= 1;
                }
                // Adjust scroll
                if state.selected_provider < state.provider_scroll {
                    state.provider_scroll = state.provider_scroll.saturating_sub(1);
                }
            }
            PanelFocus::MaxTokens => {
                state.max_tokens = state.max_tokens.saturating_add(512);
            }
            PanelFocus::Temperature => {
                state.temperature = (state.temperature + 0.1).min(2.0);
            }
            _ => {}
        },
        KeyCode::Down => match state.focus {
            PanelFocus::ProviderGrid => {
                let max = providers::ProviderKind::all().len() - 1;
                if state.selected_provider < max {
                    state.selected_provider += 1;
                }
                // Adjust scroll
                if state.selected_provider >= state.provider_scroll + 5 {
                    state.provider_scroll = state.provider_scroll.saturating_add(1).min(max.saturating_sub(4));
                }
            }
            PanelFocus::MaxTokens => {
                state.max_tokens = state.max_tokens.saturating_sub(512).max(1);
            }
            PanelFocus::Temperature => {
                state.temperature = (state.temperature - 0.1).max(0.0);
            }
            _ => {}
        },
        KeyCode::Left => match state.focus {
            PanelFocus::ProviderGrid => {
                if state.selected_provider > 2 {
                    state.selected_provider -= 3;
                }
            }
            PanelFocus::ModelField | PanelFocus::BaseUrlField => {
                // Move cursor left in text field
                let buf = state.current_buffer();
                if state.current_cursor() > 0 {
                    let prev = buf[..state.current_cursor()].char_indices().last();
                    if let Some((idx, _)) = prev {
                        state.set_cursor(idx);
                    }
                }
            }
            PanelFocus::MaxTokens => {
                state.max_tokens = state.max_tokens.saturating_sub(512).max(1);
            }
            PanelFocus::Temperature => {
                state.temperature = (state.temperature - 0.1).max(0.0);
            }
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
                // Move cursor right in text field
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
            PanelFocus::MaxTokens => {
                state.max_tokens = state.max_tokens.saturating_add(512);
            }
            PanelFocus::Temperature => {
                state.temperature = (state.temperature + 0.1).min(2.0);
            }
            _ => {}
        },
        KeyCode::Enter => {
            // Ctrl+Enter always applies, regardless of focus
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                return PanelAction::Apply;
            }
            match state.focus {
                PanelFocus::ApplyButton => {
                    return PanelAction::Apply;
                }
                _ => {
                    // Advance focus (Enter acts like Tab for most fields)
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
                    let new_buf = {
                        let mut s = state.current_buffer().to_string();
                        s.drain(prev..cursor);
                        s
                    };
                    state.set_buffer(new_buf);
                    state.set_cursor(prev);
                }
            }
        }
        KeyCode::Char(c) => {
            // Check for Ctrl+Enter to apply
            if c == '\n' && key.modifiers.contains(KeyModifiers::CONTROL) {
                return PanelAction::Close;
            }

            match state.focus {
                PanelFocus::ModelField | PanelFocus::BaseUrlField => {
                    let cursor = state.current_cursor();
                    let new_buf = {
                        let mut s = state.current_buffer().to_string();
                        s.insert(cursor, c);
                        s
                    };
                    state.set_buffer(new_buf);
                    state.set_cursor(cursor + c.len_utf8());
                }
                _ => {}
            }
        }
        _ => {}
    }

    PanelAction::None
}

// ── Helper methods on ProviderPanelState for text field manipulation ────────

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

/// Render the provider configuration panel as a centered overlay.
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    state: &ProviderPanelState,
    config: &providers::ProviderConfig,
) {
    // Need at least 54x20 for the compact panel to fit
    if area.width < 54 || area.height < 22 {
        return;
    }

    let popup_width = area.width.min(58);
    let popup_height = 20u16; // compact: 18 content + 2 border
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Dim the background
    for cy in area.y..area.y + area.height {
        for cx in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((cx, cy)) {
                cell.set_style(Style::default().fg(theme::DIM));
            }
        }
    }

    let inner_w = popup_width.saturating_sub(4) as usize; // 2 border + 2 indent
    let border_w = inner_w + 2; // width of the ┌───┐ box

    // ── Build content lines ──────────────────────────────────────────────
    let mut lines: Vec<Line<'static>> = Vec::new();
    let all_providers = providers::ProviderKind::all();
    let col_w = 17usize; // per-provider column width

    // Compact provider grid (no section header, just radio buttons)
    lines.push(Line::from(""));
    for chunk in all_providers.chunks(3) {
        let mut spans = Vec::new();
        spans.push(Span::raw("  "));
        for (ci, kind) in chunk.iter().enumerate() {
            let idx = all_providers.iter()
                .position(|k| std::mem::discriminant(k) == std::mem::discriminant(kind))
                .unwrap();
            let is_sel = idx == state.selected_provider;
            let is_foc = state.focus == PanelFocus::ProviderGrid && is_sel;
            let style = if is_foc {
                Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)
            } else if is_sel {
                Style::default().fg(theme::ACCENT)
            } else {
                theme::style_dim()
            };
            let label = format!("{} {}", if is_sel { "◉" } else { "○" }, kind);
            spans.push(Span::styled(label, style));
            if ci < 2 {
                let pad = col_w.saturating_sub(format!("{}", kind).len() + 2);
                spans.push(Span::raw(" ".repeat(pad)));
            }
        }
        lines.push(Line::from(spans));
    }

    // ── Inline Model field ───────────────────────────────────────────────
    lines.push(Line::from(""));
    let m_border = if state.focus == PanelFocus::ModelField {
        Style::default().fg(theme::ACCENT)
    } else {
        theme::style_dim()
    };
    let model_text = if state.model_buffer.is_empty() {
        "enter model name…".to_string()
    } else {
        state.model_buffer.clone()
    };
    let model_fill = inner_w.saturating_sub(model_text.len());
    lines.push(Line::from(vec![
        Span::styled("  Model  ", theme::style_dim()),
        Span::styled("┌", m_border),
        Span::raw("─".repeat(border_w.saturating_sub(8))),
        Span::styled("┐", m_border),
    ]));
    lines.push(Line::from(vec![
        Span::raw("          "),
        Span::styled("│ ", Style::default().fg(theme::DIM)),
        Span::styled(model_text, Style::default().fg(theme::FG)),
        Span::raw(" ".repeat(model_fill.saturating_sub(2).min(60))),
        Span::styled("  │", Style::default().fg(theme::DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::raw("          "),
        Span::styled("└", m_border),
        Span::raw("─".repeat(border_w.saturating_sub(8))),
        Span::styled("┘", m_border),
    ]));

    // ── Inline Base URL field ────────────────────────────────────────────
    lines.push(Line::from(""));
    let u_border = if state.focus == PanelFocus::BaseUrlField {
        Style::default().fg(theme::ACCENT)
    } else {
        theme::style_dim()
    };
    let url_text = if state.url_buffer.is_empty() {
        "enter base URL…".to_string()
    } else {
        let max_chars = inner_w.saturating_sub(4);
        if state.url_buffer.len() > max_chars {
            format!("…{}", &state.url_buffer[state.url_buffer.len().saturating_sub(max_chars - 1)..])
        } else {
            state.url_buffer.clone()
        }
    };
    let url_fill = inner_w.saturating_sub(url_text.len());
    lines.push(Line::from(vec![
        Span::styled("  URL  ", theme::style_dim()),
        Span::styled("┌", u_border),
        Span::raw("─".repeat(border_w.saturating_sub(6))),
        Span::styled("┐", u_border),
    ]));
    lines.push(Line::from(vec![
        Span::raw("        "),
        Span::styled("│ ", Style::default().fg(theme::DIM)),
        Span::styled(url_text, Style::default().fg(theme::FG)),
        Span::raw(" ".repeat(url_fill.saturating_sub(2).min(60))),
        Span::styled("  │", Style::default().fg(theme::DIM)),
    ]));
    lines.push(Line::from(vec![
        Span::raw("        "),
        Span::styled("└", u_border),
        Span::raw("─".repeat(border_w.saturating_sub(6))),
        Span::styled("┘", u_border),
    ]));

    // ── API key + numeric fields (one line each, no blanks) ─────────────
    lines.push(Line::from(""));
    let (key_label, key_icon, key_style) = if config.api_key.as_deref().filter(|k| !k.is_empty()).is_some() {
        (" Key", "●●●●●●●●●●", Style::default().fg(theme::SUCCESS))
    } else {
        (" Key", "— not set —", Style::default().fg(theme::ERROR))
    };
    let tkn_foc = state.focus == PanelFocus::MaxTokens;
    let tmp_foc = state.focus == PanelFocus::Temperature;
    lines.push(Line::from(vec![
        Span::styled(format!(" {}", key_label), theme::style_dim()),
        Span::raw(" "),
        Span::styled(key_icon, key_style),
        Span::raw("   "),
        Span::styled("Max tokens", if tkn_foc {
            Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)
        } else { theme::style_dim() }),
        Span::raw(":"),
        Span::styled(format!("{}", state.max_tokens), if tkn_foc {
            Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)
        } else { Style::default().fg(theme::FG) }),
        Span::raw("  "),
        Span::styled("Temp", if tmp_foc {
            Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)
        } else { theme::style_dim() }),
        Span::raw(":"),
        Span::styled(format!("{:.1}", state.temperature), if tmp_foc {
            Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)
        } else { Style::default().fg(theme::FG) }),
    ]));

    // ── Apply button + hints (compact one line) ──────────────────────────
    lines.push(Line::from(""));
    let btn_foc = state.focus == PanelFocus::ApplyButton;
    let btn_style = if btn_foc {
        Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)
    } else {
        theme::style_dim()
    };
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled("▶ Apply (Ctrl+Enter)", btn_style),
        Span::raw("   "),
        Span::styled("Tab/↑↓ · Esc cancel", theme::style_dim()),
    ]));

    // ── Render the popup ─────────────────────────────────────────────────
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
    para.render(popup_area, buf);
}
