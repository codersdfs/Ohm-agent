//! Provider panel navigation + key handling (P5 split from provider_panel.rs).

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::editing::{backspace, cursor_left, cursor_right, insert_char};
use super::state::{PanelAction, PanelFocus, ProviderPanelState, WizardStep};
use crate::tui::filter::FilteredList;

pub(super) const VISIBLE_MODELS: usize = 10;

pub(super) fn select_provider(state: &mut ProviderPanelState, index: usize) {
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
    state.filter_list = FilteredList::new();
    state.search_buffer.clear();
    state.search_cursor = 0;

    if should_update_url {
        state.url_buffer = kind.default_base_url();
        state.url_cursor = state.url_buffer.len();
    }
}

pub(super) fn ensure_model_visible(state: &mut ProviderPanelState) {
    // Delegate to the shared FilteredList, then sync our public fields.
    state.filter_list.selected = state.selected_model;
    state.filter_list.scroll = state.model_scroll;
    state.filter_list.ensure_visible(VISIBLE_MODELS);
    state.selected_model = state.filter_list.selected;
    state.model_scroll = state.filter_list.scroll;
}

/// Rank a model name against a query. Returns `Some(score)` if it matches.
/// Uses ranked scoring: exact match (+500), prefix (+200), contains (+50),
/// with a length penalty. The current model gets a large bonus (+1000).
pub(super) fn rank_model(name: &String, query: &str, current: &str) -> Option<i32> {
    let lower = name.to_lowercase();
    let q = query.to_lowercase();

    if !q.is_empty() && !lower.contains(&q) {
        return None;
    }

    let mut score = 0i32;

    // Prefer the current model.
    if lower == current.to_lowercase() {
        score += 1000;
    }

    if !q.is_empty() {
        if lower == q {
            score += 500;
        } else if lower.starts_with(&q) {
            score += 200;
        } else {
            score += 50;
        }
    }

    // Prefer shorter names when scores are equal.
    score -= (lower.len() as i32) / 50;

    Some(score)
}

pub(super) fn set_step(state: &mut ProviderPanelState, step: WizardStep) {
    state.step = step;
    state.focus = match step {
        WizardStep::Provider => PanelFocus::ProviderGrid,
        WizardStep::Model => PanelFocus::ModelSearch,
        WizardStep::Advanced => PanelFocus::BaseUrlField,
    };
}

pub(super) fn go_next(state: &mut ProviderPanelState) -> PanelAction {
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

pub(super) fn go_back(state: &mut ProviderPanelState) -> PanelAction {
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

pub(super) fn accept_model_selection(state: &mut ProviderPanelState) {
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
    let on_key = state.focus == PanelFocus::ApiKeyField;
    let on_text = on_url || on_key;

    match key.code {
        KeyCode::Tab => {
            state.focus = match state.focus {
                PanelFocus::BaseUrlField => PanelFocus::ApiKeyField,
                PanelFocus::ApiKeyField => PanelFocus::MaxTokens,
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
                PanelFocus::ApiKeyField => PanelFocus::BaseUrlField,
                PanelFocus::MaxTokens => PanelFocus::ApiKeyField,
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
                PanelFocus::BaseUrlField => PanelFocus::ApiKeyField,
                PanelFocus::ApiKeyField => PanelFocus::MaxTokens,
                PanelFocus::MaxTokens => PanelFocus::Temperature,
                PanelFocus::Temperature => PanelFocus::ApplyButton,
                other => other,
            };
            PanelAction::None
        }
        // Text field editing (URL / API key)
        KeyCode::Char(c) if on_text && !key.modifiers.contains(KeyModifiers::CONTROL) => {
            if on_url {
                insert_char(&mut state.url_buffer, &mut state.url_cursor, c);
            } else {
                insert_char(&mut state.key_buffer, &mut state.key_cursor, c);
            }
            PanelAction::None
        }
        KeyCode::Backspace if on_text => {
            if on_url {
                backspace(&mut state.url_buffer, &mut state.url_cursor);
            } else {
                backspace(&mut state.key_buffer, &mut state.key_cursor);
            }
            PanelAction::None
        }
        KeyCode::Left if on_text => {
            if on_url {
                cursor_left(&state.url_buffer, &mut state.url_cursor);
            } else {
                cursor_left(&state.key_buffer, &mut state.key_cursor);
            }
            PanelAction::None
        }
        KeyCode::Right if on_text => {
            if on_url {
                cursor_right(&state.url_buffer, &mut state.url_cursor);
            } else {
                cursor_right(&state.key_buffer, &mut state.key_cursor);
            }
            PanelAction::None
        }
        KeyCode::Home if on_text => {
            if on_url {
                state.url_cursor = 0;
            } else {
                state.key_cursor = 0;
            }
            PanelAction::None
        }
        KeyCode::End if on_text => {
            if on_url {
                state.url_cursor = state.url_buffer.len();
            } else {
                state.key_cursor = state.key_buffer.len();
            }
            PanelAction::None
        }
        // Numeric adjustments when not on a text field
        KeyCode::Up | KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('k') if !on_text => {
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
        KeyCode::Down | KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('j') if !on_text => {
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