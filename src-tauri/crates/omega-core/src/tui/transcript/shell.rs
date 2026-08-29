//! Source-tool lifecycle shell rendering (P5 split).

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use super::state::{ToolCallState, ToolCallStatus};
use super::toolbox::COLLAPSED_SOURCE_LINES;
use crate::tui::theme;

fn source_tool_status(status: ToolCallStatus) -> (&'static str, Color) {
    match status {
        ToolCallStatus::Pending => ("QUEUED", theme::DIM),
        ToolCallStatus::Running => ("RUNNING", theme::WARN),
        ToolCallStatus::Completed => ("COMPLETE", theme::SUCCESS),
        ToolCallStatus::Errored => ("ERROR", theme::ERROR),
    }
}

fn source_shell_row(
    content: Vec<Span<'static>>,
    content_width: usize,
    border_style: Style,
    bg: Color,
) -> Line<'static> {
    let used: usize = content.iter().map(|span| span.width()).sum();
    let padding = content_width.saturating_sub(used);
    let mut spans = Vec::with_capacity(content.len() + 3);
    spans.push(Span::styled("│", border_style));
    spans.extend(content);
    spans.push(Span::styled(" ".repeat(padding), Style::default().bg(bg)));
    spans.push(Span::styled("│", border_style));
    Line::from(spans)
}

/// Pi-style lifecycle shell for source-changing tools. Only bounded preview
/// state reaches this renderer; complete write/edit payloads are discarded at
/// ingestion. Collapsed mode shows 10 lines, expanded mode shows the retained
/// preview (capped at 100 lines per section).
fn render_source_tool_shell(state: &ToolCallState, width: u16) -> Option<Text<'static>> {
    let is_write = state.write_preview.is_some();
    let is_edit = state.edit_preview.is_some();
    if !is_write && !is_edit {
        return None;
    }

    let inner_width = usize::from(width.saturating_sub(2).max(8));
    let (status_label, status_color) = source_tool_status(state.status);
    let tool_color = if is_write {
        theme::TOOL_WRITE
    } else {
        theme::TOOL_EDIT
    };
    let bg = match state.status {
        ToolCallStatus::Errored => Color::Rgb(22, 8, 8),
        ToolCallStatus::Completed => Color::Rgb(6, 18, 10),
        ToolCallStatus::Pending | ToolCallStatus::Running => theme::SURFACE_LOW,
    };
    let border_style = Style::default().fg(tool_color).bg(bg);
    let body_style = Style::default().fg(theme::FG).bg(bg);
    let dim_style = Style::default().fg(theme::DIM).bg(bg);
    let mut lines = vec![Line::from(Span::styled(
        format!("┌{}┐", "─".repeat(inner_width)),
        border_style,
    ))];

    let (tool_label, path) = if let Some(preview) = &state.write_preview {
        ("write", preview.path.as_str())
    } else if let Some(preview) = &state.edit_preview {
        ("edit", preview.path.as_str())
    } else {
        unreachable!()
    };
    let status_width = status_label.chars().count();
    let left_width = inner_width.saturating_sub(status_width + 3);
    let left = fit_to_width(&format!(" {}  {}", tool_label, path), left_width);
    let gap = inner_width
        .saturating_sub(left.chars().count())
        .saturating_sub(status_width + 1);
    lines.push(source_shell_row(
        vec![
            Span::styled(
                left,
                Style::default()
                    .fg(tool_color)
                    .bg(bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ".repeat(gap), Style::default().bg(bg)),
            Span::styled(
                format!("{} ", status_label),
                Style::default()
                    .fg(status_color)
                    .bg(bg)
                    .add_modifier(Modifier::BOLD),
            ),
        ],
        inner_width,
        border_style,
        bg,
    ));
    lines.push(source_shell_row(vec![], inner_width, border_style, bg));

    let preview_limit = if state.expanded {
        usize::MAX
    } else {
        COLLAPSED_SOURCE_LINES
    };
    let mut shown = 0usize;
    let mut hidden = 0usize;

    if let Some(preview) = &state.write_preview {
        for line in preview.lines.iter().take(preview_limit) {
            let body = fit_to_width(line, inner_width.saturating_sub(2));
            lines.push(source_shell_row(
                vec![Span::styled(format!(" {}", body), body_style)],
                inner_width,
                border_style,
                bg,
            ));
            shown += 1;
        }
        hidden = preview.lines.len().saturating_sub(shown) + preview.omitted_lines;
    }

    if let Some(preview) = &state.edit_preview {
        let mut remaining = preview_limit;
        for line in preview.removed.iter().take(remaining) {
            let body = fit_to_width(line, inner_width.saturating_sub(3));
            lines.push(source_shell_row(
                vec![Span::styled(
                    format!(" - {}", body),
                    Style::default().fg(theme::DIFF_REMOVE).bg(bg),
                )],
                inner_width,
                border_style,
                bg,
            ));
            shown += 1;
        }
        remaining = remaining.saturating_sub(shown);
        for line in preview.added.iter().take(remaining) {
            let body = fit_to_width(line, inner_width.saturating_sub(3));
            lines.push(source_shell_row(
                vec![Span::styled(
                    format!(" + {}", body),
                    Style::default().fg(theme::DIFF_ADD).bg(bg),
                )],
                inner_width,
                border_style,
                bg,
            ));
            shown += 1;
        }
        hidden = preview.removed.len()
            + preview.added.len()
            + preview.omitted_removed
            + preview.omitted_added
            - shown;
    }

    if shown == 0 {
        lines.push(source_shell_row(
            vec![Span::styled(" (no source preview)".to_string(), dim_style)],
            inner_width,
            border_style,
            bg,
        ));
    }

    lines.push(source_shell_row(vec![], inner_width, border_style, bg));
    let footer = if hidden > 0 {
        if state.expanded {
            format!(
                " ... {} lines outside retained preview  [Ctrl+E] collapse",
                hidden
            )
        } else {
            format!(" ... {} more lines  [Ctrl+E] expand", hidden)
        }
    } else if state.expanded {
        " [Ctrl+E] collapse".to_string()
    } else {
        " [Ctrl+E] expand".to_string()
    };
    lines.push(source_shell_row(
        vec![Span::styled(fit_to_width(&footer, inner_width), dim_style)],
        inner_width,
        border_style,
        bg,
    ));

    if state.status == ToolCallStatus::Errored {
        if let Some(error) = &state.error {
            for message_line in error.message.lines().take(4) {
                lines.push(source_shell_row(
                    vec![Span::styled(
                        format!(
                            " {}",
                            fit_to_width(message_line, inner_width.saturating_sub(1))
                        ),
                        Style::default().fg(theme::ERROR).bg(bg),
                    )],
                    inner_width,
                    border_style,
                    bg,
                ));
            }
        }
    }

    lines.push(Line::from(Span::styled(
        format!("└{}┘", "─".repeat(inner_width)),
        border_style,
    )));
    Some(Text::from(lines))
}

/// Minimal left-accent-bar rendering for compact tool calls.
/// One line per call; expanded state adds result/error lines.
pub fn render_tool_call_compact(state: &ToolCallState, _avail_width: u16) -> Text<'static> {
    if let Some(source_shell) = render_source_tool_shell(state, _avail_width) {
        return source_shell;
    }
    let name = state.tool_name.clone();
    let status = state.status;
    let expanded = state.expanded;

    let accent = match status {
        ToolCallStatus::Completed => theme::SUCCESS,
        ToolCallStatus::Errored => theme::ERROR,
        ToolCallStatus::Running => theme::WARN,
        ToolCallStatus::Pending => theme::DIM,
    };

    let accent_style = Style::default().fg(accent);
    let max_name = 12usize;
    let name_short = fit_to_width(&name, max_name);
    let name_w = name_short.chars().count() as u16;

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Summary or tool name line
    let summary = fit_to_width(
        &state.tool_summary,
        _avail_width.saturating_sub(name_w + 2).max(1) as usize,
    );

    if !summary.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("│", accent_style),
            Span::styled(format!(" {} ", name_short), accent_style),
            Span::styled(summary, Style::default().fg(theme::FG)),
        ]));
    } else {
        // Just the tool name, no summary
        lines.push(Line::from(vec![
            Span::styled("│", accent_style),
            Span::styled(format!(" {}", name_short), accent_style),
        ]));
    }

    // Expanded body
    if expanded {
        match status {
            ToolCallStatus::Errored => {
                if let Some(e) = &state.error {
                    for l in e.message.lines().take(3) {
                        let line = fit_to_width(l, _avail_width.saturating_sub(2).max(1) as usize);
                        lines.push(Line::from(vec![
                            Span::styled("│", accent_style),
                            Span::styled(format!(" {}", line), Style::default().fg(theme::ERROR)),
                        ]));
                    }
                }
            }
            ToolCallStatus::Completed => {
                if let Some(preview) = &state.result_preview {
                    for l in preview.lines().take(3) {
                        let line = fit_to_width(l, _avail_width.saturating_sub(2).max(1) as usize);
                        lines.push(Line::from(vec![
                            Span::styled("│", accent_style),
                            Span::styled(format!(" {}", line), Style::default().fg(theme::FG)),
                        ]));
                    }
                }
            }
            _ => {}
        }
    }

    Text::from(lines)
}

pub fn fit_to_width(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let take = max_chars.saturating_sub(1);
    let truncated: String = s.chars().take(take).collect();
    format!("{}…", truncated)
}

pub fn shorten(s: &str, max: usize) -> String {
    fit_to_width(s, max)
}
