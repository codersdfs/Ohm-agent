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
    pub needs_fetch: bool,
    pub models_loading: bool,
    pub models: Vec<String>,
    pub models_error: Option<String>,
    pub show_dropdown: bool,
    pub selected_model: usize,
    pub models_rx: Option<tokio::sync::oneshot::Receiver<Result<Vec<String>, String>>>,
    /// Stashed config for Component rendering (API key display etc.).
    pub config: providers::ProviderConfig,
}

impl ProviderPanelState {
    pub fn from_config(config: &providers::ProviderConfig) -> Self {
        let all = providers::ProviderKind::all();
        let selected = all
            .iter()
            .position(|k| std::mem::discriminant(k) == std::mem::discriminant(&config.kind))
            .unwrap_or(0);
        let default_url = config.base_url.clone().unwrap_or_else(|| config.kind.default_base_url());
        Self {
            visible: true,
            focus: PanelFocus::ProviderGrid,
            selected_provider: selected,
            model_buffer: config.model.clone(),
            model_cursor: config.model.len(),
            url_buffer: default_url.clone(),
            url_cursor: default_url.len(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            provider_scroll: 0,
            needs_fetch: true,
            models_loading: false,
            models: Vec::new(),
            models_error: None,
            show_dropdown: false,
            selected_model: 0,
            models_rx: None,
            config: config.clone(),
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

// ── Navigation helpers ──────────────────────────────────────────────────────

/// Move focus up within the current field or navigate the provider grid up.
fn move_focus_up(state: &mut ProviderPanelState) {
    match state.focus {
        PanelFocus::ProviderGrid if state.selected_provider > 0 => {
            state.selected_provider -= 1;
            state.needs_fetch = true;
            if state.selected_provider < state.provider_scroll {
                state.provider_scroll = state.provider_scroll.saturating_sub(1);
            }
        }
        PanelFocus::MaxTokens => state.max_tokens = state.max_tokens.saturating_add(512),
        PanelFocus::Temperature => state.temperature = (state.temperature + 0.1).min(2.0),
        _ => {}
    }
}

/// Move focus down within the current field or navigate the provider grid down.
fn move_focus_down(state: &mut ProviderPanelState) {
    match state.focus {
        PanelFocus::ProviderGrid => {
            let max = providers::ProviderKind::all().len() - 1;
            if state.selected_provider < max {
                state.selected_provider += 1;
                state.needs_fetch = true;
            }
            if state.selected_provider >= state.provider_scroll + 5 {
                state.provider_scroll = state.provider_scroll.saturating_add(1).min(max.saturating_sub(4));
            }
        }
        PanelFocus::MaxTokens => state.max_tokens = state.max_tokens.saturating_sub(512).max(1),
        PanelFocus::Temperature => state.temperature = (state.temperature - 0.1).max(0.0),
        _ => {}
    }
}

/// Move focus left: navigate provider grid columns or cursor left in text fields.
fn move_focus_left(state: &mut ProviderPanelState) {
    match state.focus {
        PanelFocus::ProviderGrid if state.selected_provider > 2 => {
            state.selected_provider -= 3;
            state.needs_fetch = true;
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
    }
}

/// Move focus right: navigate provider grid columns or cursor right in text fields.
fn move_focus_right(state: &mut ProviderPanelState) {
    match state.focus {
        PanelFocus::ProviderGrid => {
            let max = providers::ProviderKind::all().len() - 1;
            if state.selected_provider + 3 <= max {
                state.selected_provider += 3;
                state.needs_fetch = true;
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
    }
}

/// Jump directly to a provider by number (1-indexed).
fn jump_to_provider_by_number(state: &mut ProviderPanelState, num: usize) {
    let max = providers::ProviderKind::all().len();
    if num >= 1 && num <= max {
        let idx = num - 1;
        state.selected_provider = idx;
        state.needs_fetch = true;
        // Adjust scroll so the selected provider is visible
        let items_per_col = 5usize;
        if idx < state.provider_scroll {
            state.provider_scroll = idx;
        } else if idx >= state.provider_scroll + items_per_col {
            state.provider_scroll = idx.saturating_sub(items_per_col).saturating_add(1);
        }
    }
}

/// Toggle the model dropdown open/closed.
fn toggle_dropdown(state: &mut ProviderPanelState) {
    if !state.models.is_empty() {
        state.show_dropdown = !state.show_dropdown;
        // Close: apply the currently selected model
        if !state.show_dropdown {
            if let Some(m) = state.models.get(state.selected_model) {
                state.model_buffer = m.clone();
                state.model_cursor = m.len();
            }
        }
    }
}

// ── Key handler ─────────────────────────────────────────────────────────────

pub fn handle_key(state: &mut ProviderPanelState, key: KeyEvent) -> PanelAction {
    if key.kind != KeyEventKind::Press {
        return PanelAction::None;
    }

    // Dropdown mode: keys navigate the model list
    if state.show_dropdown && state.focus == PanelFocus::ModelField {
        match key.code {
            KeyCode::Esc | KeyCode::Tab => {
                state.show_dropdown = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if state.selected_model > 0 {
                    state.selected_model -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = state.models.len().saturating_sub(1);
                if state.selected_model < max {
                    state.selected_model += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(model) = state.models.get(state.selected_model) {
                    state.model_buffer = model.clone();
                    state.model_cursor = model.len();
                    state.show_dropdown = false;
                }
            }
            _ => {}
        }
        return PanelAction::None;
    }

    match key.code {
        KeyCode::Esc => return PanelAction::Close,

        // Tab forward
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

        // Tab backward (Shift+Tab)
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

        // Vim-style navigation: j = down, k = up
        KeyCode::Char('j') => move_focus_down(state),
        KeyCode::Char('k') => move_focus_up(state),

        // Vim-style navigation: h = left, l = right
        KeyCode::Char('h') => move_focus_left(state),
        KeyCode::Char('l') => move_focus_right(state),

        // Arrow key navigation in all directions
        KeyCode::Up => move_focus_up(state),
        KeyCode::Down => move_focus_down(state),
        KeyCode::Left => move_focus_left(state),
        KeyCode::Right => move_focus_right(state),

        // Number keys for direct provider selection (1-9, 0 for 10)
        KeyCode::Char(c) if c.is_ascii_digit() && state.focus == PanelFocus::ProviderGrid => {
            let num = if c == '0' { 10 } else { c.to_digit(10).unwrap_or(0) as usize };
            jump_to_provider_by_number(state, num);
        }

        // Enter/Space to toggle dropdown or apply
        KeyCode::Enter | KeyCode::Char(' ') => {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && (key.code == KeyCode::Enter || key.code == KeyCode::Char(' '))
            {
                return PanelAction::Apply;
            }
            match state.focus {
                PanelFocus::ApplyButton => return PanelAction::Apply,
                PanelFocus::ModelField if !state.models.is_empty() => {
                    toggle_dropdown(state);
                }
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

        // Regular character input for text fields
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

    let pw = area.width.min(58);
    let ph = 20u16;
    let x = area.x + (area.width.saturating_sub(pw)) / 2;
    let y = area.y + (area.height.saturating_sub(ph)) / 2;

    // Dim background
    for cy in area.y..area.y + area.height {
        for cx in area.x..area.x + area.width {
            if let Some(cell) = theme::buf_cell_mut(buf, cx, cy) {
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
                Style::default()
                    .fg(theme::PRIMARY_CONTAINER)
                    .add_modifier(Modifier::BOLD)
            } else if sel {
                Style::default()
                    .fg(theme::ACCENT)
            } else {
                theme::style_dim()
            };
            // Use ◉ (filled circle) for selected, ○ (open circle) otherwise.
            // Focused + selected gets a filled circle with bright color;
            // selected but not focused gets a filled circle with accent;
            // unselected gets open circle.
            let marker = if sel { "◉" } else { "○" };
            let label = format!("{}{}", marker, kind);
            spans.push(Span::styled(label, style));
            // Dim the provider index hint when not in provider grid focus
            let number_hint_style = if state.focus == PanelFocus::ProviderGrid {
                Style::default().fg(theme::DIM).add_modifier(Modifier::DIM)
            } else {
                Style::default().fg(theme::DIM)
            };
            let num = idx + 1;
            let hint = if num <= 9 {
                format!("[{}]", num)
            } else if num == 10 {
                String::from("[0]")
            } else {
                String::new()
            };
            if !hint.is_empty() {
                spans.push(Span::styled(hint, number_hint_style));
            }
            let kind_str = format!("{}", kind);
            // Account for marker (1) + kind + hint (3) + 1 = kind_str.len() + 5
            let pad = 20usize.saturating_sub(kind_str.len() + 5);
            if ci < 2 {
                spans.push(Span::raw(" ".repeat(pad)));
            }
        }
        lines.push(Line::from(spans));
    }

    // Model field — single line with enhanced focus indicator
    lines.push(Line::from(""));
    let m_foc = state.focus == PanelFocus::ModelField;
    let m_label = if m_foc { "▸ Model " } else { "  Model " };
    let model_display = if state.model_buffer.is_empty() {
        String::from("enter model name…")
    } else {
        state.model_buffer.clone()
    };
    let m_avail = iw.saturating_sub(m_label.chars().count() + 3); // space for ":" and cursor
    let m_trunc: String = if model_display.chars().count() > m_avail {
        // Char-safe tail truncation with leading ellipsis.
        let keep = m_avail.saturating_sub(1).max(1) as usize;
        let tail: String = model_display.chars().rev().take(keep).collect::<Vec<_>>().into_iter().rev().collect();
        format!("…{}", tail)
    } else {
        model_display
    };
    let m_label_style = if m_foc {
        Style::default().fg(theme::PRIMARY_CONTAINER).add_modifier(Modifier::BOLD)
    } else {
        theme::style_dim()
    };
    let m_span: Span<'static> = if m_foc {
        Span::styled(m_trunc.clone(), theme::style_focused_field())
    } else {
        Span::styled(m_trunc, theme::style_dim())
    };
    // Model count indicator
    let count_span = if state.models_loading {
        Some(Span::styled(" ⟳", theme::style_dim()))
    } else if !state.models.is_empty() {
        Some(Span::styled(
            format!(" ({})", state.models.len()),
            theme::style_dim(),
        ))
    } else if let Some(ref err) = state.models_error {
        // Char-safe truncation: byte slicing here would panic if the 20th
        // byte lands inside a multibyte codepoint (e.g. localized errors).
        let short: String = err.chars().take(20).collect();
        Some(Span::styled(format!(" ⚠{}", short), Style::default().fg(theme::ERROR)))
    } else {
        None
    };
    let mut ml = vec![Span::styled(m_label, m_label_style), m_span];
    if let Some(c) = count_span {
        ml.push(c);
    }
    lines.push(Line::from(ml));

    // URL field
    let u_foc = state.focus == PanelFocus::BaseUrlField;
    let u_label = if u_foc { "▸ URL " } else { "  URL " };
    let url_text = if state.url_buffer.is_empty() {
        "enter base URL…".to_string()
    } else {
        let u_avail = iw.saturating_sub(u_label.chars().count() + 3);
        if state.url_buffer.chars().count() > u_avail {
            // Char-safe tail truncation with leading ellipsis.
            let keep = u_avail.saturating_sub(1).max(1) as usize;
            let tail: String = state.url_buffer.chars().rev().take(keep).collect::<Vec<_>>().into_iter().rev().collect();
            format!("…{}", tail)
        } else {
            state.url_buffer.clone()
        }
    };
    let u_label_style = if u_foc {
        Style::default().fg(theme::PRIMARY_CONTAINER).add_modifier(Modifier::BOLD)
    } else {
        theme::style_dim()
    };
    let u_span: Span<'static> = if u_foc {
        Span::styled(url_text, theme::style_focused_field())
    } else {
        Span::styled(url_text, theme::style_dim())
    };
    lines.push(Line::from(vec![
        Span::styled(u_label, u_label_style),
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

    // Max tokens value style
    let tkn_val_style = if tkn_foc {
        Style::default()
            .fg(theme::PRIMARY_CONTAINER)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::FG)
    };
    // Temperature value style
    let tmp_val_style = if tmp_foc {
        Style::default()
            .fg(theme::PRIMARY_CONTAINER)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::FG)
    };

    lines.push(Line::from(vec![
        Span::styled("Key ", theme::style_dim()),
        Span::styled(key_display, Style::default().fg(key_color)),
        Span::raw("  "),
        Span::styled(format!("{}Max:", tkn_lbl), if tkn_foc { Style::default().fg(theme::PRIMARY_CONTAINER).add_modifier(Modifier::BOLD) } else { theme::style_dim() }),
        Span::styled(format!("{}", state.max_tokens), tkn_val_style),
        Span::raw("  "),
        Span::styled(format!("{}Temp:", tmp_lbl), if tmp_foc { Style::default().fg(theme::PRIMARY_CONTAINER).add_modifier(Modifier::BOLD) } else { theme::style_dim() }),
        Span::styled(format!("{:.1}", state.temperature), tmp_val_style),
    ]));

    // Apply button + hints
    lines.push(Line::from(""));
    let b_foc = state.focus == PanelFocus::ApplyButton;
    let (apply_label, apply_style) = if b_foc {
        ("▸ [ Apply (Ctrl+Enter) ]", theme::style_focused_button())
    } else {
        ("  Apply (Ctrl+Enter)", theme::style_dim())
    };
    lines.push(Line::from(vec![
        Span::styled(apply_label, apply_style),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Tab/↑↓ cycle · h/j/k/l navigate · #1-9 select · Esc cancel", theme::style_dim()),
    ]));

    // ── Render main popup ──────────────────────────────────────────────
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

    // ── Model dropdown overlay ──────────────────────────────────────────
    if state.show_dropdown && state.focus == PanelFocus::ModelField && !state.models.is_empty() {
        let dd_top = y + 4 + 5 + 1 + 1; // top border + blank + 5 providers + blank + model label
        let dd_height = state.models.len().min(6) as u16 + 2; // +2 for border
        let dd_area = Rect::new(
            x + 2,
            dd_top,
            pw.saturating_sub(4).min(40),
            dd_height,
        );
        if dd_area.bottom() <= area.bottom() {
            let mut dd_lines: Vec<Line<'static>> = Vec::new();
            let visible = state.models.iter().take(6);
            for (i, model) in visible.enumerate() {
                let style = if i == state.selected_model {
                    Style::default().fg(theme::PRIMARY_CONTAINER).add_modifier(Modifier::BOLD)
                } else {
                    theme::style_dim()
                };
                let prefix = if i == state.selected_model { "▸ " } else { "  " };
                dd_lines.push(Line::from(Span::styled(
                    format!("{}{}", prefix, model),
                    style,
                )));
            }
            if state.models.len() > 6 {
                dd_lines.push(Line::from(Span::styled(
                    format!("  … {} more", state.models.len() - 6),
                    theme::style_dim(),
                )));
            }
            let dd = Paragraph::new(Text::from(dd_lines))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme::FOCUS_BORDER))
                        .style(Style::default().bg(theme::SURFACE_HIGH)),
                );
            dd.render(dd_area, buf);
        }
    }
}

use crate::tui::component::{Action, Component};

impl Component for ProviderPanelState {
    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        use crate::tui::provider_panel::{handle_key, PanelAction};
        match handle_key(self, key) {
            PanelAction::Apply => Action::ProviderApply,
            PanelAction::Close => Action::ProviderClose,
            PanelAction::None => Action::Noop,
        }
    }

    fn render(&mut self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        crate::tui::provider_panel::render(area, f.buffer_mut(), self, &self.config);
    }
}