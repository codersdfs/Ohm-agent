//! Provider panel rendering (P5 split from provider_panel.rs).

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap};

use super::advanced::render_step_advanced;
use super::logic::VISIBLE_MODELS;
use super::state::{ProviderPanelState, WizardStep};
use crate::tui::theme;

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

pub(super) fn section_block(title: &str) -> Block<'static> {
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