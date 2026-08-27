//! Advanced-step panel rendering (P5 split from provider_panel/ui.rs).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Widget};

use super::state::{PanelFocus, ProviderPanelState};
use super::ui::section_block;
use crate::tui::theme;

pub fn render_step_advanced(
    area: Rect,
    buf: &mut Buffer,
    state: &ProviderPanelState,
    _config: &providers::ProviderConfig,
) {
    if area.height < 6 {
        return;
    }

    let half = area.height / 2;
    let conn_area = Rect::new(area.x, area.y, area.width, half.max(5));
    let gen_area = Rect::new(
        area.x,
        area.y + half.max(5),
        area.width,
        area.height.saturating_sub(half.max(5)),
    );

    // Connection section
    let conn_block = section_block("Connection");
    let conn_inner = conn_block.inner(conn_area);
    conn_block.render(conn_area, buf);

    let url_foc = state.focus == PanelFocus::BaseUrlField;
    let key_foc = state.focus == PanelFocus::ApiKeyField;
    let url_label = if url_foc {
        "Γû╕ Base URL"
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
    let key_label = if key_foc {
        "Γû╕ API key"
    } else {
        "  API key"
    };
    let key_label_style = if key_foc {
        Style::default()
            .fg(theme::PRIMARY_CONTAINER)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        theme::style_dim()
    };

    let key_set = !state.key_buffer.is_empty();
    let key_display = if key_set {
        // Mask the secret; show length so users know something is set.
        format!(
            "{}  ({} chars)",
            "ΓùÅ".repeat(state.key_buffer.chars().count().min(24)),
            state.key_buffer.chars().count()
        )
    } else if key_foc {
        "type to set API key...".into()
    } else {
        "ΓÇö not set ΓÇö".into()
    };
    let key_color = if key_set {
        theme::SUCCESS
    } else if key_foc {
        theme::FG
    } else {
        theme::ERROR
    };

    let is_custom = matches!(
        providers::ProviderKind::all().get(state.selected_provider),
        Some(providers::ProviderKind::Custom)
    );
    let mut conn_lines = vec![
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
        Line::from(Span::styled(key_label, key_label_style)),
        Line::from(Span::styled(
            format!("  {}", key_display),
            if key_foc {
                Style::default()
                    .fg(key_color)
                    .add_modifier(Modifier::UNDERLINED)
            } else {
                Style::default().fg(key_color)
            },
        )),
    ];
    if is_custom {
        conn_lines.push(Line::from(Span::styled(
            "  custom = OpenAI-compatible endpoint; set base URL + key",
            theme::style_dim(),
        )));
    }
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
                    "Γû╕ Max tokens  "
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
                    "Γû╕ Temperature "
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
                "Γû╕ [ Apply ]  Enter / Ctrl+Enter"
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
                "  Will set: {} ┬╖ {}",
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
