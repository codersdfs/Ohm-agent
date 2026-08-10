//! Provider configuration wizard panel (P5 split from provider_panel.rs).

pub mod advanced;
pub mod editing;
pub mod logic;
pub mod state;
pub mod ui;

pub use state::{PanelAction, PanelFocus, ProviderPanelState, WizardStep};

// Re-export the public key handler + render used by App/other modules.
pub use logic::handle_key;
pub use ui::render;

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn config(kind: providers::ProviderKind, base_url: Option<&str>) -> providers::ProviderConfig {
        providers::ProviderConfig {
            kind,
            api_key: Some("test-key".into()),
            base_url: base_url.map(str::to_owned),
            model: "current-model".into(),
            max_tokens: 4096,
            temperature: 0.7,
            max_concurrent_tools: 3,
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
        logic::set_step(&mut state, WizardStep::Advanced);
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
        logic::set_step(&mut state, WizardStep::Provider);
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
        logic::set_step(&mut state, WizardStep::Advanced);
        assert_eq!(state.focus, PanelFocus::BaseUrlField);
        handle_key(&mut state, key(KeyCode::Tab));
        assert_eq!(state.focus, PanelFocus::ApiKeyField);
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
        logic::set_step(&mut state, WizardStep::Advanced);
        state.url_buffer.clear();
        state.url_cursor = 0;
        for c in ['h', 'j', 'k', 'l'] {
            handle_key(&mut state, key(KeyCode::Char(c)));
        }
        assert_eq!(state.url_buffer, "hjkl");
    }

    #[test]
    fn api_key_field_accepts_input_and_to_config() {
        let original = config(providers::ProviderKind::OpenAI, None);
        let mut state = ProviderPanelState::from_config(&original);
        logic::set_step(&mut state, WizardStep::Advanced);
        handle_key(&mut state, key(KeyCode::Tab)); // BaseUrl → ApiKey
        assert_eq!(state.focus, PanelFocus::ApiKeyField);
        state.key_buffer.clear();
        state.key_cursor = 0;
        for c in ['s', 'k', '-', '1', '2', '3'] {
            handle_key(&mut state, key(KeyCode::Char(c)));
        }
        assert_eq!(state.key_buffer, "sk-123");
        let cfg = state.to_config(&original);
        assert_eq!(cfg.api_key.as_deref(), Some("sk-123"));
    }

    #[test]
    fn custom_provider_is_in_list() {
        let all = providers::ProviderKind::all();
        assert!(all
            .iter()
            .any(|k| matches!(k, providers::ProviderKind::Custom)));
        let state = ProviderPanelState::from_config(&config(providers::ProviderKind::Custom, None));
        assert_eq!(
            state.selected_provider,
            all.iter()
                .position(|k| matches!(k, providers::ProviderKind::Custom))
                .unwrap()
        );
        assert_eq!(
            state.url_buffer,
            providers::ProviderKind::Custom.default_base_url()
        );
    }

    #[test]
    fn provider_list_moves_and_wraps() {
        let mut state =
            ProviderPanelState::from_config(&config(providers::ProviderKind::Anthropic, None));
        logic::set_step(&mut state, WizardStep::Provider);
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
        logic::set_step(&mut default_state, WizardStep::Provider);
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
        logic::set_step(&mut custom_state, WizardStep::Provider);
        handle_key(&mut custom_state, key(KeyCode::Down));
        assert_eq!(custom_state.url_buffer, "https://gateway.example/v1");
    }

    #[test]
    fn max_tokens_and_temperature_adjust() {
        let mut state =
            ProviderPanelState::from_config(&config(providers::ProviderKind::OpenAI, None));
        logic::set_step(&mut state, WizardStep::Advanced);
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
        state.key_buffer = "new-secret".into();
        // Keep OpenAI selected
        let cfg = state.to_config(&original);
        assert_eq!(cfg.model, "new-model");
        assert_eq!(cfg.max_tokens, 2048);
        assert!((cfg.temperature - 0.2).abs() < f32::EPSILON);
        assert_eq!(cfg.base_url.as_deref(), Some("https://custom"));
        assert_eq!(cfg.api_key.as_deref(), Some("new-secret"));
    }
}

