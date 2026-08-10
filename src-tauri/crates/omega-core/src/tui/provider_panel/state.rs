//! Provider panel state (P5 split from provider_panel.rs).

use super::logic::{rank_model, set_step};
use crate::tui::filter::FilteredList;

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
    ApiKeyField,
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
    pub key_buffer: String,
    pub key_cursor: usize,
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
    /// Shared filter logic (kept in sync with `filtered`, `selected_model`, `model_scroll`).
    pub(super) filter_list: FilteredList<String>,
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
        let key_seed = config.api_key.clone().unwrap_or_default();
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
            key_buffer: key_seed.clone(),
            key_cursor: key_seed.len(),
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
            filter_list: FilteredList::new(),
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
            api_key: Some(self.key_buffer.clone()).filter(|s| !s.is_empty()),
            base_url: Some(self.url_buffer.clone()).filter(|s| !s.is_empty()),
            model: if self.model_buffer.is_empty() {
                original.model.clone()
            } else {
                self.model_buffer.clone()
            },
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            max_concurrent_tools: 3,
        }
    }

    /// Reset the filter state (called when models are cleared/fetched).
    pub fn reset_filter_state(&mut self) {
        self.filtered.clear();
        self.selected_model = 0;
        self.model_scroll = 0;
        self.filter_list = FilteredList::new();
    }

    pub fn recompute_filter(&mut self) {
        // Delegate to the shared FilteredList, then sync our public fields.
        let current = self.model_buffer.clone();
        self.filter_list.set_preferred(
            self.models
                .iter()
                .position(|m| m == &current),
        );
        self.filter_list.recompute(&self.models, &self.search_buffer, |name, query| {
            rank_model(name, query, &current)
        });
        self.filtered = self.filter_list.filtered.clone();
        self.selected_model = self.filter_list.selected;
        self.model_scroll = self.filter_list.scroll;
    }

    pub(super) fn provider_name(&self) -> String {
        providers::ProviderKind::all()
            .get(self.selected_provider)
            .map(|k| k.to_string())
            .unwrap_or_else(|| "unknown".into())
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> crate::tui::component::Action {
        match super::logic::handle_key(self, key) {
            PanelAction::Apply => crate::tui::component::Action::ProviderApply,
            PanelAction::Close => crate::tui::component::Action::ProviderClose,
            PanelAction::None => crate::tui::component::Action::Noop,
        }
    }

    pub fn render(&mut self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        super::ui::render(area, f.buffer_mut(), self, &self.config);
    }
}
