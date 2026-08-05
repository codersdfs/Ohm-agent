//! Write/edit code preview extraction + panel rendering (P5 split).

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::state::ToolCallState;
use super::shell::fit_to_width;
use super::toolbox::{MAX_RETAINED_SOURCE_LINES, MAX_SOURCE_COLUMNS};
use crate::tui::theme;

#[derive(Clone)]
pub struct WriteCodePreview {
    pub path: String,
    pub lines: Vec<String>,
    pub omitted_lines: usize,
}

#[derive(Clone)]
pub struct EditCodePreview {
    pub path: String,
    pub removed: Vec<String>,
    pub added: Vec<String>,
    pub omitted_removed: usize,
    pub omitted_added: usize,
}

fn collect_bounded_lines(
    source: &str,
    max_lines: usize,
    max_columns: usize,
) -> (Vec<String>, usize) {
    let mut preview = Vec::with_capacity(max_lines);
    let mut total = 0usize;
    for line in source.lines() {
        total += 1;
        if preview.len() < max_lines {
            let normalized = line.replace('\t', "    ");
            preview.push(fit_to_width(&normalized, max_columns));
        }
    }
    (preview, total.saturating_sub(max_lines))
}

pub fn extract_write_preview(tool_name: &str, args: &str) -> Option<WriteCodePreview> {
    if !matches!(tool_name, "write" | "create") {
        return None;
    }

    let parsed = serde_json::from_str::<serde_json::Value>(args).ok()?;
    let obj = parsed.as_object()?;
    let content = obj.get("content")?.as_str()?;
    if content.is_empty() {
        return None;
    }

    let path = obj
        .get("filePath")
        .or_else(|| obj.get("path"))
        .or_else(|| obj.get("file"))
        .and_then(|v| v.as_str())
        .unwrap_or("untitled");

    let (lines, omitted_lines) =
        collect_bounded_lines(content, MAX_RETAINED_SOURCE_LINES, MAX_SOURCE_COLUMNS);

    Some(WriteCodePreview {
        path: fit_to_width(path, 160),
        lines,
        omitted_lines,
    })
}

pub fn extract_edit_preview(tool_name: &str, args: &str) -> Option<EditCodePreview> {
    if !matches!(tool_name, "edit" | "patch" | "str_replace") {
        return None;
    }

    let parsed = serde_json::from_str::<serde_json::Value>(args).ok()?;
    let obj = parsed.as_object()?;
    let path = obj
        .get("filePath")
        .or_else(|| obj.get("path"))
        .or_else(|| obj.get("file"))
        .and_then(|v| v.as_str())
        .unwrap_or("untitled");
    let old = obj
        .get("oldString")
        .or_else(|| obj.get("oldText"))
        .or_else(|| obj.get("find"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let new = obj
        .get("newString")
        .or_else(|| obj.get("newText"))
        .or_else(|| obj.get("replace"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let (removed, omitted_removed) =
        collect_bounded_lines(old, MAX_RETAINED_SOURCE_LINES, MAX_SOURCE_COLUMNS);
    let (added, omitted_added) =
        collect_bounded_lines(new, MAX_RETAINED_SOURCE_LINES, MAX_SOURCE_COLUMNS);

    Some(EditCodePreview {
        path: fit_to_width(path, 160),
        removed,
        added,
        omitted_removed,
        omitted_added,
    })
}

/// Add a nested, colored code panel for write/create calls.
///
/// The preview is extracted once when the tool event arrives. The full write
/// payload is not retained by transcript state or reparsed on every frame.
pub fn push_write_code_panel(
    lines: &mut Vec<Line<'static>>,
    state: &ToolCallState,
    avail: usize,
    outer_border_style: Style,
    outer_bg: Color,
) {
    let Some(preview) = state.write_preview.as_ref() else {
        return;
    };

    // Outer row: │ + space + nested panel + space + │
    let panel_width = avail.saturating_sub(2);
    let code_width = panel_width.saturating_sub(2);
    if code_width < 4 {
        return;
    }

    let code_bg = Color::Rgb(5, 15, 10);
    let code_border = Style::default().fg(theme::TOOL_WRITE).bg(code_bg);
    let code_text = Style::default().fg(theme::FG).bg(code_bg);
    let code_dim = Style::default().fg(theme::DIM).bg(code_bg);
    let outer_pad = Style::default().bg(outer_bg);

    let label = fit_to_width(
        &format!(" CODE · {} ", preview.path),
        code_width.saturating_sub(1),
    );
    let header_fill = code_width
        .saturating_sub(1)
        .saturating_sub(label.chars().count());
    let top = format!("┌─{}{}┐", label, "─".repeat(header_fill));
    lines.push(Line::from(vec![
        Span::styled("│", outer_border_style),
        Span::styled(" ", outer_pad),
        Span::styled(top, code_border.add_modifier(Modifier::BOLD)),
        Span::styled(" ", outer_pad),
        Span::styled("│", outer_border_style),
    ]));

    for source_line in &preview.lines {
        let code = fit_to_width(source_line, code_width);
        let padding = code_width.saturating_sub(code.chars().count());
        lines.push(Line::from(vec![
            Span::styled("│", outer_border_style),
            Span::styled(" ", outer_pad),
            Span::styled("│", code_border),
            Span::styled(code, code_text),
            Span::styled(" ".repeat(padding), code_text),
            Span::styled("│", code_border),
            Span::styled(" ", outer_pad),
            Span::styled("│", outer_border_style),
        ]));
    }

    if preview.omitted_lines > 0 {
        let omitted = format!("… {} more lines", preview.omitted_lines);
        let omitted = fit_to_width(&omitted, code_width);
        let padding = code_width.saturating_sub(omitted.chars().count());
        lines.push(Line::from(vec![
            Span::styled("│", outer_border_style),
            Span::styled(" ", outer_pad),
            Span::styled("│", code_border),
            Span::styled(omitted, code_dim.add_modifier(Modifier::ITALIC)),
            Span::styled(" ".repeat(padding), code_dim),
            Span::styled("│", code_border),
            Span::styled(" ", outer_pad),
            Span::styled("│", outer_border_style),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("│", outer_border_style),
        Span::styled(" ", outer_pad),
        Span::styled(format!("└{}┘", "─".repeat(code_width)), code_border),
        Span::styled(" ", outer_pad),
        Span::styled("│", outer_border_style),
    ]));
}

pub fn push_edit_diff_panel(
    lines: &mut Vec<Line<'static>>,
    state: &ToolCallState,
    avail: usize,
    outer_border_style: Style,
    outer_bg: Color,
) {
    let Some(preview) = state.edit_preview.as_ref() else {
        return;
    };

    let panel_width = avail.saturating_sub(2);
    let code_width = panel_width.saturating_sub(2);
    if code_width < 4 {
        return;
    }

    let diff_bg = Color::Rgb(12, 10, 10);
    let diff_border = Style::default().fg(theme::TOOL_EDIT).bg(diff_bg);
    let removed_style = Style::default().fg(theme::DIFF_REMOVE).bg(diff_bg);
    let added_style = Style::default().fg(theme::DIFF_ADD).bg(diff_bg);
    let dim_style = Style::default().fg(theme::DIM).bg(diff_bg);
    let outer_pad = Style::default().bg(outer_bg);

    let label = fit_to_width(
        &format!(" DIFF · {} ", preview.path),
        code_width.saturating_sub(1),
    );
    let header_fill = code_width
        .saturating_sub(1)
        .saturating_sub(label.chars().count());
    lines.push(Line::from(vec![
        Span::styled("│", outer_border_style),
        Span::styled(" ", outer_pad),
        Span::styled(
            format!("┌─{}{}┐", label, "─".repeat(header_fill)),
            diff_border.add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", outer_pad),
        Span::styled("│", outer_border_style),
    ]));

    let mut push_diff_line = |prefix: &str, source: &str, style: Style| {
        let body_width = code_width.saturating_sub(2);
        let body = fit_to_width(source, body_width);
        let padding = body_width.saturating_sub(body.chars().count());
        lines.push(Line::from(vec![
            Span::styled("│", outer_border_style),
            Span::styled(" ", outer_pad),
            Span::styled("│", diff_border),
            Span::styled(prefix.to_string(), style),
            Span::styled(body, style),
            Span::styled(" ".repeat(padding), style),
            Span::styled("│", diff_border),
            Span::styled(" ", outer_pad),
            Span::styled("│", outer_border_style),
        ]));
    };

    for line in &preview.removed {
        push_diff_line("- ", line, removed_style);
    }
    if preview.omitted_removed > 0 {
        push_diff_line(
            "  ",
            &format!("… {} removed lines hidden", preview.omitted_removed),
            dim_style.add_modifier(Modifier::ITALIC),
        );
    }
    for line in &preview.added {
        push_diff_line("+ ", line, added_style);
    }
    if preview.omitted_added > 0 {
        push_diff_line(
            "  ",
            &format!("… {} added lines hidden", preview.omitted_added),
            dim_style.add_modifier(Modifier::ITALIC),
        );
    }

    drop(push_diff_line);
    lines.push(Line::from(vec![
        Span::styled("│", outer_border_style),
        Span::styled(" ", outer_pad),
        Span::styled(format!("└{}┘", "─".repeat(code_width)), diff_border),
        Span::styled(" ", outer_pad),
        Span::styled("│", outer_border_style),
    ]));
}