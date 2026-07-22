use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap};

use super::theme;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

const VISIBLE_MODELS: usize = 10;

/// Actions the panel can return to the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelAction {
    Close,
    Apply,
    None,
}

/// Wizard step for the full-screen provider panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WizardStep {
    Provider,
    Model,
    Advanced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelFocus {
    ProviderGrid,
    ModelSearch,
    BaseUrlField,
    MaxTokens,
    Temperature,
    ApplyButton,
}

pub struct ProviderPanelState {
    pub visible: bool,
    pub step: WizardStep,
    pub focus: PanelFocus,
    pub selected_provider: usize,
    pub model_buffer: String,
    pub model_cursor: usize,
    pub search_buffer: String,
    pub search_cursor: usize,
    pub url_buffer: String,
    pub url_cursor: usize,
    pub max_tokens: u32,
    pub temperature: f32,
    pub needs_fetch: bool,
    pub models_loading: bool,
    pub models: Vec<String>,
    pub models_error: Option<String>,
    pub selected_model: usize,
    pub model_scroll: usize,
    /// Indices into `models` after filter/rank.
    pub filtered: Vec<usize>,
    pub models_rx: Option<tokio::sync::oneshot::Receiver<Result<Vec<String>, String>>>,
    pub config: providers::ProviderConfig,
}

impl ProviderPanelState {
    pub fn from_config(config: &providers::ProviderConfig) -> Self {
        Self::from_config_at(config, WizardStep::Provider)
    }

    /// Build panel state opened on a specific wizard step.
    /// `/provider` → Provider, `/model` → Model.
    pub fn from_config_at(config: &providers::ProviderConfig, step: WizardStep) -> Self {
        let all = providers::ProviderKind::all();
        let selected = all
            .iter()
            .position(|k| std::mem::discriminant(k) == std::mem::discriminant(&config.kind))
            .unwrap_or(0);
        let default_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| config.kind.default_base_url());
        let mut state = Self {
            visible: true,
            // Temporary; `set_step` below sets the real step + focus.
            step: WizardStep::Provider,
            focus: PanelFocus::ProviderGrid,
            selected_provider: selected,
            model_buffer: config.model.clone(),
            model_cursor: config.model.len(),
            search_buffer: String::new(),
            search_cursor: 0,
            url_buffer: default_url.clone(),
            url_cursor: default_url.len(),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            needs_fetch: true,
            models_loading: false,
            models: Vec::new(),
            models_error: None,
            selected_model: 0,
            model_scroll: 0,
            filtered: Vec::new(),
            models_rx: None,
            config: config.clone(),
        };
        set_step(&mut state, step);
        state.recompute_filter();
        state
    }

    pub fn to_config(&self, original: &providers::ProviderConfig) -> providers::ProviderConfig {
        let all = providers::ProviderKind::all();
        let kind = all
            .get(self.selected_provider)
            .cloned()
            .unwrap_or(original.kind.clone());
        providers::ProviderConfig {
            kind,
            api_key: original.api_key.clone(),
            base_url: Some(self.url_buffer.clone()).filter(|s| !s.is_empty()),
            model: if self.model_buffer.is_empty() {
                original.model.clone()
            } else {
                self.model_buffer.clone()
            },
            max_tokens: self.max_tokens,
            temperature: self.temperature,
        }
    }

    pub fn recompute_filter(&mut self) {
        let query = self.search_buffer.to_lowercase();
        let current = self.model_buffer.to_lowercase();

        let mut ranked: Vec<(usize, i32)> = self
            .models
            .iter()
            .enumerate()
            .filter_map(|(idx, name)| {
                let lower = name.to_lowercase();
                if !query.is_empty() && !lower.contains(&query) {
                    return None;
                }
                let mut score = 0i32;
                if lower == current {
                    score += 1000;
                }
                if !query.is_empty() {
                    if lower == query {
                        score += 500;
                    } else if lower.starts_with(&query) {
                        score += 200;
                    } else {
                        score += 50;
                    }
                }
                // Prefer shorter names when scores equal-ish.
                score -= (lower.len() as i32) / 50;
                Some((idx, score))
            })
            .collect();

        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        self.filtered = ranked.into_iter().map(|(i, _)| i).collect();

        if self.filtered.is_empty() {
            self.selected_model = 0;
            self.model_scroll = 0;
        } else {
            // Prefer current model if still in filtered set.
            if let Some(pos) = self.filtered.iter().position(|&i| {
                self.models.get(i).map(|m| m.as_str()) == Some(self.model_buffer.as_str())
            }) {
                self.selected_model = pos;
            } else {
                self.selected_model = self.selected_model.min(self.filtered.len() - 1);
            }
            ensure_model_visible(self);
        }
    }

    fn provider_name(&self) -> String {
        providers::ProviderKind::all()
            .get(self.selected_provider)
            .map(|k| k.to_string())
            .unwrap_or_else(|| "unknown".into())
    }
}

// ── Navigation helpers ──────────────────────────────────────────────────────

fn select_provider(state: &mut ProviderPanelState, index: usize) {
    let all = providers::ProviderKind::all();
    let Some(kind) = all.get(index).cloned() else {
        return;
    };
    if state.selected_provider == index {
        return;
    }

    let old_default = all
        .get(state.selected_provider)
        .map(|k| k.default_base_url());
    let should_update_url =
        state.url_buffer.is_empty() || old_default.as_deref() == Some(state.url_buffer.as_str());

    state.selected_provider = index;
    state.needs_fetch = true;
    state.models.clear();
    state.models_error = None;
    state.selected_model = 0;
    state.model_scroll = 0;
    state.filtered.clear();
    state.search_buffer.clear();
    state.search_cursor = 0;

    if should_update_url {
        state.url_buffer = kind.default_base_url();
        state.url_cursor = state.url_buffer.len();
    }
}

fn ensure_model_visible(state: &mut ProviderPanelState) {
    if state.filtered.is_empty() {
        state.selected_model = 0;
        state.model_scroll = 0;
        return;
    }
    state.selected_model = state.selected_model.min(state.filtered.len() - 1);
    if state.selected_model < state.model_scroll {
        state.model_scroll = state.selected_model;
    } else if state.selected_model >= state.model_scroll + VISIBLE_MODELS {
        state.model_scroll = state.selected_model + 1 - VISIBLE_MODELS;
    }
}

fn set_step(state: &mut ProviderPanelState, step: WizardStep) {
    state.step = step;
    state.focus = match step {
        WizardStep::Provider => PanelFocus::ProviderGrid,
        WizardStep::Model => PanelFocus::ModelSearch,
        WizardStep::Advanced => PanelFocus::BaseUrlField,
    };
}

fn go_next(state: &mut ProviderPanelState) -> PanelAction {
    match state.step {
        WizardStep::Provider => {
            set_step(state, WizardStep::Model);
            PanelAction::None
        }
        WizardStep::Model => {
            accept_model_selection(state);
            set_step(state, WizardStep::Advanced);
            PanelAction::None
        }
        WizardStep::Advanced => PanelAction::Apply,
    }
}

fn go_back(state: &mut ProviderPanelState) -> PanelAction {
    match state.step {
        WizardStep::Provider => PanelAction::Close,
        WizardStep::Model => {
            set_step(state, WizardStep::Provider);
            PanelAction::None
        }
        WizardStep::Advanced => {
            set_step(state, WizardStep::Model);
            PanelAction::None
        }
    }
}

fn accept_model_selection(state: &mut ProviderPanelState) {
    if !state.filtered.is_empty() {
        if let Some(&idx) = state.filtered.get(state.selected_model) {
            if let Some(name) = state.models.get(idx) {
                state.model_buffer = name.clone();
                state.model_cursor = name.len();
                return;
            }
        }
    }
    // No matches: accept search text as custom model id when non-empty.
    if !state.search_buffer.is_empty() {
        state.model_buffer = state.search_buffer.clone();
        state.model_cursor = state.model_buffer.len();
    }
}

fn move_provider(state: &mut ProviderPanelState, code: KeyCode) {
    let len = providers::ProviderKind::all().len();
    if len == 0 {
        return;
    }
    let cur = state.selected_provider.min(len - 1);
    let next = match code {
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Left | KeyCode::Char('h') => {
            if cur == 0 {
                len - 1
            } else {
                cur - 1
            }
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Right | KeyCode::Char('l') => (cur + 1) % len,
        _ => cur,
    };
    if next != cur {
        select_provider(state, next);
    }
}

fn jump_provider_number(state: &mut ProviderPanelState, num: usize) {
    let max = providers::ProviderKind::all().len();
    if num >= 1 && num <= max {
        select_provider(state, num - 1);
    }
}

fn insert_char(buf: &mut String, cursor: &mut usize, c: char) {
    let pos = (*cursor).min(buf.len());
    buf.insert(pos, c);
    *cursor = pos + c.len_utf8();
}

fn backspace(buf: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let prev = buf[..*cursor]
        .char_indices()
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0);
    buf.drain(prev..*cursor);
    *cursor = prev;
}

fn cursor_left(buf: &str, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    *cursor = buf[..*cursor]
        .char_indices()
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0);
}

fn cursor_right(buf: &str, cursor: &mut usize) {
    if *cursor >= buf.len() {
        return;
    }
    if let Some((i, _)) = buf[*cursor..].char_indices().nth(1) {
        *cursor += i;
    } else {
        *cursor = buf.len();
    }
}

// ── Key handler ─────────────────────────────────────────────────────────────

pub fn handle_key(state: &mut ProviderPanelState, key: KeyEvent) -> PanelAction {
    if key.kind != KeyEventKind::Press {
        return PanelAction::None;
    }

    // Ctrl+Enter applies from any step.
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Enter | KeyCode::Char('\n'))
    {
        if state.step == WizardStep::Model {
            accept_model_selection(state);
        }
        return PanelAction::Apply;
    }

    match key.code {
        KeyCode::Esc => return go_back(state),
        _ => {}
    }

    match state.step {
        WizardStep::Provider => handle_step_provider(state, key),
        WizardStep::Model => handle_step_model(state, key),
        WizardStep::Advanced => handle_step_advanced(state, key),
    }
}

fn handle_step_provider(state: &mut ProviderPanelState, key: KeyEvent) -> PanelAction {
    match key.code {
        KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Tab => go_next(state),
        KeyCode::Up
        | KeyCode::Down
        | KeyCode::Left
        | KeyCode::Right
        | KeyCode::Char('h')
        | KeyCode::Char('j')
        | KeyCode::Char('k')
        | KeyCode::Char('l') => {
            move_provider(state, key.code);
            PanelAction::None
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let num = if c == '0' {
                10
            } else {
                c.to_digit(10).unwrap_or(0) as usize
            };
            jump_provider_number(state, num);
            PanelAction::None
        }
        _ => PanelAction::None,
    }
}

fn handle_step_model(state: &mut ProviderPanelState, key: KeyEvent) -> PanelAction {
    match key.code {
        KeyCode::Tab | KeyCode::Enter => go_next(state),
        KeyCode::BackTab => go_back(state),
        KeyCode::Up => {
            if !state.filtered.is_empty() {
                state.selected_model = if state.selected_model == 0 {
                    state.filtered.len() - 1
                } else {
                    state.selected_model - 1
                };
                ensure_model_visible(state);
            }
            PanelAction::None
        }
        KeyCode::Down => {
            if !state.filtered.is_empty() {
                state.selected_model = (state.selected_model + 1) % state.filtered.len();
                ensure_model_visible(state);
            }
            PanelAction::None
        }
        KeyCode::Left => {
            cursor_left(&state.search_buffer, &mut state.search_cursor);
            PanelAction::None
        }
        KeyCode::Right => {
            cursor_right(&state.search_buffer, &mut state.search_cursor);
            PanelAction::None
        }
        KeyCode::Home => {
            state.search_cursor = 0;
            PanelAction::None
        }
        KeyCode::End => {
            state.search_cursor = state.search_buffer.len();
            PanelAction::None
        }
        KeyCode::Backspace => {
            backspace(&mut state.search_buffer, &mut state.search_cursor);
            state.recompute_filter();
            PanelAction::None
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            // All printable chars (including h/j/k/l) go into search.
            insert_char(&mut state.search_buffer, &mut state.search_cursor, c);
            state.recompute_filter();
            PanelAction::None
        }
        _ => PanelAction::None,
    }
}

fn handle_step_advanced(state: &mut ProviderPanelState, key: KeyEvent) -> PanelAction {
    let on_url = state.focus == PanelFocus::BaseUrlField;

    match key.code {
        KeyCode::Tab => {
            state.focus = match state.focus {
                PanelFocus::BaseUrlField => PanelFocus::MaxTokens,
                PanelFocus::MaxTokens => PanelFocus::Temperature,
                PanelFocus::Temperature => PanelFocus::ApplyButton,
                PanelFocus::ApplyButton => PanelFocus::BaseUrlField,
                _ => PanelFocus::BaseUrlField,
            };
            PanelAction::None
        }
        KeyCode::BackTab => {
            state.focus = match state.focus {
                PanelFocus::BaseUrlField => PanelFocus::ApplyButton,
                PanelFocus::MaxTokens => PanelFocus::BaseUrlField,
                PanelFocus::Temperature => PanelFocus::MaxTokens,
                PanelFocus::ApplyButton => PanelFocus::Temperature,
                _ => PanelFocus::BaseUrlField,
            };
            PanelAction::None
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            if state.focus == PanelFocus::ApplyButton
                || key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL)
            {
                return PanelAction::Apply;
            }
            if state.focus == PanelFocus::ApplyButton {
                return PanelAction::Apply;
            }
            // Advance focus on Enter.
            state.focus = match state.focus {
                PanelFocus::BaseUrlField => PanelFocus::MaxTokens,
                PanelFocus::MaxTokens => PanelFocus::Temperature,
                PanelFocus::Temperature => PanelFocus::ApplyButton,
                other => other,
            };
            PanelAction::None
        }
        // URL editing
        KeyCode::Char(c) if on_url && !key.modifiers.contains(KeyModifiers::CONTROL) => {
            insert_char(&mut state.url_buffer, &mut state.url_cursor, c);
            PanelAction::None
        }
        KeyCode::Backspace if on_url => {
            backspace(&mut state.url_buffer, &mut state.url_cursor);
            PanelAction::None
        }
        KeyCode::Left if on_url => {
            cursor_left(&state.url_buffer, &mut state.url_cursor);
            PanelAction::None
        }
        KeyCode::Right if on_url => {
            cursor_right(&state.url_buffer, &mut state.url_cursor);
            PanelAction::None
        }
        KeyCode::Home if on_url => {
            state.url_cursor = 0;
            PanelAction::None
        }
        KeyCode::End if on_url => {
            state.url_cursor = state.url_buffer.len();
            PanelAction::None
        }
        // Numeric adjustments when not on URL
        KeyCode::Up | KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('k') if !on_url => {
            match state.focus {
                PanelFocus::MaxTokens => {
                    state.max_tokens = state.max_tokens.saturating_add(512);
                }
                PanelFocus::Temperature => {
                    state.temperature = (state.temperature + 0.1).min(2.0);
                }
                _ => {}
            }
            PanelAction::None
        }
        KeyCode::Down | KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('j') if !on_url => {
            match state.focus {
                PanelFocus::MaxTokens => {
                    state.max_tokens = state.max_tokens.saturating_sub(512).max(1);
                }
                PanelFocus::Temperature => {
                    state.temperature = (state.temperature - 0.1).max(0.0);
                }
                _ => {}
            }
            PanelAction::None
        }
        _ => PanelAction::None,
    }
}

// ── Render ─────────────────────────────────────────────────────────────────

fn clear_area(area: Rect, buf: &mut Buffer, bg: ratatui::style::Color) {
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = theme::buf_cell_mut(buf, x, y) {
                // Wipe glyph + style so underlying chat/tool chrome cannot bleed through.
                cell.set_symbol(" ");
                cell.set_style(Style::default().fg(theme::FG).bg(bg));
            }
        }
    }
}

fn section_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(theme::OUTLINE))
        .title(format!(" {} ", title))
        .title_style(Style::default().fg(theme::PRIMARY))
        .style(Style::default().bg(theme::SURFACE))
}

pub fn render(
    area: Rect,
    buf: &mut Buffer,
    state: &ProviderPanelState,
    config: &providers::ProviderConfig,
) {
    if area.width < 40 || area.height < 12 {
        return;
    }

    // Full opaque wipe — modal owns the entire screen.
    clear_area(area, buf, theme::SURFACE);

    // Outer frame
    let title = match state.step {
        WizardStep::Provider => " Provider / Model  ·  Step 1/3: Provider ",
        WizardStep::Model => " Provider / Model  ·  Step 2/3: Model ",
        WizardStep::Advanced => " Provider / Model  ·  Step 3/3: Advanced ",
    };
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(theme::OUTLINE))
        .title(title)
        .title_style(
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(theme::SURFACE));
    let inner = outer.inner(area);
    outer.render(area, buf);

    if inner.height < 6 || inner.width < 20 {
        return;
    }

    // Clear inner content area again after border paint (belt + suspenders).
    clear_area(inner, buf, theme::SURFACE);

    // Header summary
    let summary = format!(
        " Current: {} · {} ",
        state.provider_name(),
        if state.model_buffer.is_empty() {
            "—"
        } else {
            &state.model_buffer
        }
    );
    Paragraph::new(Line::from(Span::styled(summary, theme::style_dim())))
        .style(Style::default().bg(theme::SURFACE))
        .render(Rect::new(inner.x, inner.y, inner.width, 1), buf);

    // Body + footer
    let body = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(3),
    );
    let footer = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(2),
        inner.width,
        1,
    );

    match state.step {
        WizardStep::Provider => render_step_provider(body, buf, state),
        WizardStep::Model => render_step_model(body, buf, state),
        WizardStep::Advanced => render_step_advanced(body, buf, state, config),
    }

    let hints = match state.step {
        WizardStep::Provider => {
            " ↑↓/jk move · 1-9/0 jump · Enter next · Esc cancel · Ctrl+Enter apply "
        }
        WizardStep::Model => {
            " type filter · ↑↓ wrap · Enter next · Esc providers · Ctrl+Enter apply "
        }
        WizardStep::Advanced => " Tab fields · Enter apply · Esc back · Ctrl+Enter apply ",
    };
    Paragraph::new(Line::from(Span::styled(hints, theme::style_dim())))
        .style(Style::default().bg(theme::SURFACE))
        .render(footer, buf);
}

fn render_step_provider(area: Rect, buf: &mut Buffer, state: &ProviderPanelState) {
    let block = section_block("Providers");
    let inner = block.inner(area);
    block.render(area, buf);

    let all = providers::ProviderKind::all();
    let mut lines: Vec<Line<'static>> = Vec::new();

    for (idx, kind) in all.iter().enumerate() {
        let sel = idx == state.selected_provider;
        let style = if sel {
            Style::default()
                .fg(theme::PRIMARY_CONTAINER)
                .add_modifier(Modifier::BOLD)
        } else {
            theme::style_dim()
        };
        let marker = if sel { "▸ " } else { "  " };
        let num = idx + 1;
        let hint = if num <= 9 {
            format!("  [{}]", num)
        } else if num == 10 {
            "  [0]".into()
        } else {
            String::new()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{}{}", marker, kind), style),
            Span::styled(hint, theme::style_dim()),
        ]));
    }

    lines.push(Line::from(""));
    if let Some(kind) = all.get(state.selected_provider) {
        lines.push(Line::from(Span::styled(
            format!(" Default URL: {} ", kind.default_base_url()),
            theme::style_dim(),
        )));
    }

    Paragraph::new(Text::from(lines))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(theme::SURFACE))
        .render(inner, buf);
}

fn render_step_model(area: Rect, buf: &mut Buffer, state: &ProviderPanelState) {
    if area.height < 4 {
        return;
    }

    // Search box (3 rows with border)
    let search_h = 3u16.min(area.height);
    let search_area = Rect::new(area.x, area.y, area.width, search_h);
    let list_area = Rect::new(
        area.x,
        area.y + search_h,
        area.width,
        area.height.saturating_sub(search_h),
    );

    let search_block = section_block("Search");
    let search_inner = search_block.inner(search_area);
    search_block.render(search_area, buf);
    let search_display = if state.search_buffer.is_empty() {
        "type to filter models…".to_string()
    } else {
        state.search_buffer.clone()
    };
    Paragraph::new(Line::from(Span::styled(
        format!("▸ {}", search_display),
        Style::default()
            .fg(theme::FG)
            .add_modifier(Modifier::UNDERLINED),
    )))
    .style(Style::default().bg(theme::SURFACE))
    .render(search_inner, buf);

    let total = state.models.len();
    let match_n = state.filtered.len();
    let list_title = if state.models_loading {
        "Models · loading…".to_string()
    } else if let Some(ref err) = state.models_error {
        let short: String = err.chars().take(40).collect();
        format!("Models · error: {}", short)
    } else {
        format!("Models ({} match / {})", match_n, total)
    };
    let list_block = section_block(&list_title);
    let list_inner = list_block.inner(list_area);
    list_block.render(list_area, buf);

    let mut lines: Vec<Line<'static>> = Vec::new();
    if state.models_loading {
        lines.push(Line::from(Span::styled(
            "  ⠋ fetching models…",
            theme::style_dim(),
        )));
    } else if state.filtered.is_empty() {
        if total == 0 {
            lines.push(Line::from(Span::styled(
                "  no models yet — type a custom model id and press Enter",
                theme::style_dim(),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "  no matches — Enter accepts search as custom model",
                theme::style_dim(),
            )));
        }
    } else {
        let start = state
            .model_scroll
            .min(state.filtered.len().saturating_sub(1));
        let end = (start + VISIBLE_MODELS).min(state.filtered.len());
        for (vis_i, &model_idx) in state.filtered[start..end].iter().enumerate() {
            let index = start + vis_i;
            let name = state
                .models
                .get(model_idx)
                .map(|s| s.as_str())
                .unwrap_or("?");
            let is_sel = index == state.selected_model;
            let is_current = name == state.model_buffer;
            let style = if is_sel {
                Style::default()
                    .fg(theme::PRIMARY_CONTAINER)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme::style_dim()
            };
            let prefix = if is_sel { "▸ " } else { "  " };
            let suffix = if is_current { "  ★ current" } else { "" };
            lines.push(Line::from(Span::styled(
                format!("{}{}{}", prefix, name, suffix),
                style,
            )));
        }
    }

    Paragraph::new(Text::from(lines))
        .alignment(Alignment::Left)
        .style(Style::default().bg(theme::SURFACE))
        .render(list_inner, buf);
}

fn render_step_advanced(
    area: Rect,
    buf: &mut Buffer,
    state: &ProviderPanelState,
    config: &providers::ProviderConfig,
) {
    if area.height < 6 {
        return;
    }

    let half = area.height / 2;
    let conn_area = Rect::new(area.x, area.y, area.width, half.max(4));
    let gen_area = Rect::new(
        area.x,
        area.y + half.max(4),
        area.width,
        area.height.saturating_sub(half.max(4)),
    );

    // Connection section
    let conn_block = section_block("Connection");
    let conn_inner = conn_block.inner(conn_area);
    conn_block.render(conn_area, buf);

    let url_foc = state.focus == PanelFocus::BaseUrlField;
    let url_label = if url_foc {
        "▸ Base URL"
    } else {
        "  Base URL"
    };
    let url_style = if url_foc {
        Style::default()
            .fg(theme::PRIMARY_CONTAINER)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        theme::style_dim()
    };
    let key_set = config
        .api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .is_some();
    let key_display = if key_set {
        "● ● ● ● ● ● ● ●  (set)"
    } else {
        "— not set —"
    };
    let key_color = if key_set {
        theme::SUCCESS
    } else {
        theme::ERROR
    };

    let conn_lines = vec![
        Line::from(Span::styled(url_label, url_style)),
        Line::from(Span::styled(
            format!("  {}", state.url_buffer),
            if url_foc {
                Style::default()
                    .fg(theme::FG)
                    .add_modifier(Modifier::UNDERLINED)
            } else {
                theme::style_dim()
            },
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  API key  ", theme::style_dim()),
            Span::styled(key_display, Style::default().fg(key_color)),
        ]),
    ];
    Paragraph::new(Text::from(conn_lines))
        .style(Style::default().bg(theme::SURFACE))
        .render(conn_inner, buf);

    // Generation + Apply
    let gen_block = section_block("Generation & Apply");
    let gen_inner = gen_block.inner(gen_area);
    gen_block.render(gen_area, buf);

    let tkn_foc = state.focus == PanelFocus::MaxTokens;
    let tmp_foc = state.focus == PanelFocus::Temperature;
    let app_foc = state.focus == PanelFocus::ApplyButton;

    let gen_lines = vec![
        Line::from(vec![
            Span::styled(
                if tkn_foc {
                    "▸ Max tokens  "
                } else {
                    "  Max tokens  "
                },
                if tkn_foc {
                    Style::default()
                        .fg(theme::PRIMARY_CONTAINER)
                        .add_modifier(Modifier::BOLD)
                } else {
                    theme::style_dim()
                },
            ),
            Span::styled(
                format!("[ {} ]", state.max_tokens),
                if tkn_foc {
                    Style::default()
                        .fg(theme::PRIMARY_CONTAINER)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme::FG)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled(
                if tmp_foc {
                    "▸ Temperature "
                } else {
                    "  Temperature "
                },
                if tmp_foc {
                    Style::default()
                        .fg(theme::PRIMARY_CONTAINER)
                        .add_modifier(Modifier::BOLD)
                } else {
                    theme::style_dim()
                },
            ),
            Span::styled(
                format!("[ {:.1} ]", state.temperature),
                if tmp_foc {
                    Style::default()
                        .fg(theme::PRIMARY_CONTAINER)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme::FG)
                },
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            if app_foc {
                "▸ [ Apply ]  Enter / Ctrl+Enter"
            } else {
                "  [ Apply ]  Enter / Ctrl+Enter"
            },
            if app_foc {
                Style::default()
                    .fg(theme::PRIMARY_CONTAINER)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme::style_dim()
            },
        )),
        Line::from(Span::styled(
            format!(
                "  Will set: {} · {}",
                state.provider_name(),
                state.model_buffer
            ),
            theme::style_dim(),
        )),
    ];
    Paragraph::new(Text::from(gen_lines))
        .style(Style::default().bg(theme::SURFACE))
        .render(gen_inner, buf);
}

// ── Component impl ──────────────────────────────────────────────────────────

use crate::tui::component::{Action, Component};

impl Component for ProviderPanelState {
    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        match handle_key(self, key) {
            PanelAction::Apply => Action::ProviderApply,
            PanelAction::Close => Action::ProviderClose,
            PanelAction::None => Action::Noop,
        }
    }

    fn render(&mut self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        render(area, f.buffer_mut(), self, &self.config);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn config(kind: providers::ProviderKind, base_url: Option<&str>) -> providers::ProviderConfig {
        providers::ProviderConfig {
            kind,
            api_key: Some("test-key".into()),
            base_url: base_url.map(str::to_owned),
            model: "current-model".into(),
            max_tokens: 4096,
            temperature: 0.7,
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_enter() -> KeyEvent {
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL)
    }

    #[test]
    fn from_config_opens_on_provider_step() {
        let state = ProviderPanelState::from_config(&config(providers::ProviderKind::OpenAI, None));
        assert_eq!(state.step, WizardStep::Provider);
        assert_eq!(state.focus, PanelFocus::ProviderGrid);
        assert_eq!(
            state.selected_provider,
            providers::ProviderKind::all()
                .iter()
                .position(|k| matches!(k, providers::ProviderKind::OpenAI))
                .unwrap()
        );
    }

    #[test]
    fn from_config_at_opens_on_model_step() {
        let state = ProviderPanelState::from_config_at(
            &config(providers::ProviderKind::OpenAI, None),
            WizardStep::Model,
        );
        assert_eq!(state.step, WizardStep::Model);
        assert_eq!(state.focus, PanelFocus::ModelSearch);
    }

    #[test]
    fn wizard_esc_back_then_close() {
        let mut state =
            ProviderPanelState::from_config(&config(providers::ProviderKind::OpenAI, None));
        set_step(&mut state, WizardStep::Advanced);
        assert_eq!(handle_key(&mut state, key(KeyCode::Esc)), PanelAction::None);
        assert_eq!(state.step, WizardStep::Model);
        assert_eq!(handle_key(&mut state, key(KeyCode::Esc)), PanelAction::None);
        assert_eq!(state.step, WizardStep::Provider);
        assert_eq!(
            handle_key(&mut state, key(KeyCode::Esc)),
            PanelAction::Close
        );
    }

    #[test]
    fn enter_on_provider_advances_to_model() {
        let mut state =
            ProviderPanelState::from_config(&config(providers::ProviderKind::OpenAI, None));
        set_step(&mut state, WizardStep::Provider);
        assert_eq!(
            handle_key(&mut state, key(KeyCode::Enter)),
            PanelAction::None
        );
        assert_eq!(state.step, WizardStep::Model);
        assert_eq!(state.focus, PanelFocus::ModelSearch);
    }

    #[test]
    fn enter_on_model_advances_to_advanced_and_selects() {
        let mut state = ProviderPanelState::from_config_at(
            &config(providers::ProviderKind::OpenAI, None),
            WizardStep::Model,
        );
        state.models = vec!["a".into(), "b".into(), "c".into()];
        state.recompute_filter();
        state.selected_model = 1;
        assert_eq!(
            handle_key(&mut state, key(KeyCode::Enter)),
            PanelAction::None
        );
        assert_eq!(state.step, WizardStep::Advanced);
        assert_eq!(state.model_buffer, "b");
    }

    #[test]
    fn enter_accepts_custom_model_when_no_matches() {
        let mut state = ProviderPanelState::from_config_at(
            &config(providers::ProviderKind::OpenAI, None),
            WizardStep::Model,
        );
        state.models = vec!["alpha".into(), "beta".into()];
        state.recompute_filter();
        state.search_buffer = "custom-id".into();
        state.search_cursor = state.search_buffer.len();
        state.recompute_filter();
        assert!(state.filtered.is_empty());
        handle_key(&mut state, key(KeyCode::Enter));
        assert_eq!(state.model_buffer, "custom-id");
        assert_eq!(state.step, WizardStep::Advanced);
    }

    #[test]
    fn filter_narrows_and_resets_selection() {
        let mut state = ProviderPanelState::from_config_at(
            &config(providers::ProviderKind::OpenAI, None),
            WizardStep::Model,
        );
        state.models = vec![
            "claude-opus".into(),
            "claude-sonnet".into(),
            "gpt-4o".into(),
        ];
        state.recompute_filter();
        assert_eq!(state.filtered.len(), 3);
        handle_key(&mut state, key(KeyCode::Char('g')));
        handle_key(&mut state, key(KeyCode::Char('p')));
        handle_key(&mut state, key(KeyCode::Char('t')));
        assert_eq!(state.filtered.len(), 1);
        assert_eq!(state.models[state.filtered[0]], "gpt-4o");
    }

    #[test]
    fn current_model_ranked_first() {
        let mut state =
            ProviderPanelState::from_config(&config(providers::ProviderKind::OpenAI, None));
        state.model_buffer = "gpt-4o".into();
        state.models = vec!["alpha".into(), "gpt-4o".into(), "beta".into()];
        state.recompute_filter();
        assert_eq!(state.models[state.filtered[0]], "gpt-4o");
    }

    #[test]
    fn model_list_wraps_scrolls_and_selects() {
        let mut state = ProviderPanelState::from_config_at(
            &config(providers::ProviderKind::OpenAI, None),
            WizardStep::Model,
        );
        state.models = (0..15).map(|i| format!("model-{i}")).collect();
        state.recompute_filter();
        handle_key(&mut state, key(KeyCode::Up));
        assert_eq!(state.selected_model, 14);
        assert!(state.model_scroll >= 5);
        handle_key(&mut state, key(KeyCode::Down));
        assert_eq!(state.selected_model, 0);
        assert_eq!(state.model_scroll, 0);
    }

    #[test]
    fn ctrl_enter_applies_from_model_step() {
        let mut state = ProviderPanelState::from_config_at(
            &config(providers::ProviderKind::OpenAI, None),
            WizardStep::Model,
        );
        assert_eq!(state.step, WizardStep::Model);
        assert_eq!(handle_key(&mut state, ctrl_enter()), PanelAction::Apply);
    }

    #[test]
    fn tab_cycles_advanced_fields() {
        let mut state =
            ProviderPanelState::from_config(&config(providers::ProviderKind::OpenAI, None));
        set_step(&mut state, WizardStep::Advanced);
        assert_eq!(state.focus, PanelFocus::BaseUrlField);
        handle_key(&mut state, key(KeyCode::Tab));
        assert_eq!(state.focus, PanelFocus::MaxTokens);
        handle_key(&mut state, key(KeyCode::Tab));
        assert_eq!(state.focus, PanelFocus::Temperature);
        handle_key(&mut state, key(KeyCode::Tab));
        assert_eq!(state.focus, PanelFocus::ApplyButton);
        handle_key(&mut state, key(KeyCode::Tab));
        assert_eq!(state.focus, PanelFocus::BaseUrlField);
    }

    #[test]
    fn hjkl_not_nav_in_search() {
        let mut state = ProviderPanelState::from_config_at(
            &config(providers::ProviderKind::OpenAI, None),
            WizardStep::Model,
        );
        state.models = vec!["a".into(), "b".into(), "c".into()];
        state.recompute_filter();
        let before = state.selected_model;
        handle_key(&mut state, key(KeyCode::Char('j')));
        handle_key(&mut state, key(KeyCode::Char('k')));
        assert_eq!(state.search_buffer, "jk");
        assert_eq!(state.selected_model, before);
    }

    #[test]
    fn vim_letters_are_inserted_in_url_field() {
        let mut state =
            ProviderPanelState::from_config(&config(providers::ProviderKind::OpenAI, None));
        set_step(&mut state, WizardStep::Advanced);
        state.url_buffer.clear();
        state.url_cursor = 0;
        for c in ['h', 'j', 'k', 'l'] {
            handle_key(&mut state, key(KeyCode::Char(c)));
        }
        assert_eq!(state.url_buffer, "hjkl");
    }

    #[test]
    fn provider_list_moves_and_wraps() {
        let mut state =
            ProviderPanelState::from_config(&config(providers::ProviderKind::Anthropic, None));
        set_step(&mut state, WizardStep::Provider);
        state.selected_provider = 0;
        handle_key(&mut state, key(KeyCode::Down));
        assert_eq!(state.selected_provider, 1);
        handle_key(&mut state, key(KeyCode::Down));
        assert_eq!(state.selected_provider, 2);
        handle_key(&mut state, key(KeyCode::Up));
        assert_eq!(state.selected_provider, 1);

        // Wrap from first → last
        state.selected_provider = 0;
        handle_key(&mut state, key(KeyCode::Up));
        assert_eq!(
            state.selected_provider,
            providers::ProviderKind::all().len() - 1
        );
        // Wrap from last → first
        handle_key(&mut state, key(KeyCode::Down));
        assert_eq!(state.selected_provider, 0);
    }

    #[test]
    fn provider_change_updates_default_url_but_preserves_custom_url() {
        let mut default_state =
            ProviderPanelState::from_config(&config(providers::ProviderKind::OpenAI, None));
        set_step(&mut default_state, WizardStep::Provider);
        // OpenAI is index 1; Down moves +1 → index 2 (Google)
        handle_key(&mut default_state, key(KeyCode::Down));
        assert_eq!(
            default_state.url_buffer,
            providers::ProviderKind::Google.default_base_url()
        );

        let mut custom_state = ProviderPanelState::from_config(&config(
            providers::ProviderKind::OpenAI,
            Some("https://gateway.example/v1"),
        ));
        set_step(&mut custom_state, WizardStep::Provider);
        handle_key(&mut custom_state, key(KeyCode::Down));
        assert_eq!(custom_state.url_buffer, "https://gateway.example/v1");
    }

    #[test]
    fn max_tokens_and_temperature_adjust() {
        let mut state =
            ProviderPanelState::from_config(&config(providers::ProviderKind::OpenAI, None));
        set_step(&mut state, WizardStep::Advanced);
        state.focus = PanelFocus::MaxTokens;
        let before = state.max_tokens;
        handle_key(&mut state, key(KeyCode::Up));
        assert_eq!(state.max_tokens, before + 512);
        state.focus = PanelFocus::Temperature;
        state.temperature = 0.7;
        handle_key(&mut state, key(KeyCode::Down));
        assert!((state.temperature - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn to_config_roundtrip_fields() {
        let original = config(providers::ProviderKind::OpenAI, Some("https://x"));
        let mut state = ProviderPanelState::from_config(&original);
        state.model_buffer = "new-model".into();
        state.max_tokens = 2048;
        state.temperature = 0.2;
        state.url_buffer = "https://custom".into();
        // Keep OpenAI selected
        let cfg = state.to_config(&original);
        assert_eq!(cfg.model, "new-model");
        assert_eq!(cfg.max_tokens, 2048);
        assert!((cfg.temperature - 0.2).abs() < f32::EPSILON);
        assert_eq!(cfg.base_url.as_deref(), Some("https://custom"));
        assert_eq!(cfg.api_key.as_deref(), Some("test-key"));
    }
}
