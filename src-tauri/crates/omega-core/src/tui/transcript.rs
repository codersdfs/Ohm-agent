use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};

use super::markdown;
use super::theme;

const ACTIVITY_SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const ACTIVITY_WORDS: &[&str] = &[
    "Cooking…",
    "Pondering…",
    "Reasoning…",
    "Planning…",
    "Considering…",
];

fn activity_text(tick: u64) -> String {
    let glyph = ACTIVITY_SPINNER[tick as usize % ACTIVITY_SPINNER.len()];
    let word = ACTIVITY_WORDS[(tick as usize / 24) % ACTIVITY_WORDS.len()];
    format!("  {glyph} {word} ")
}

/// A single entry in the conversation transcript.
#[derive(Clone)]
pub enum TranscriptEntry {
    /// User message (plain text or markdown)
    User { content: String },
    /// Assistant message (markdown rendered, with optional thinking/reasoning prefix)
    Assistant {
        content: String,
        rendered: Option<Text<'static>>,
        is_streaming: bool,
        /// Model-internal reasoning/thinking, shown dimmed before content
        thinking: String,
    },
    /// Tool call — rendered as a bordered box (Claude Code / Pi Agent style)
    ToolCallBox { state: ToolCallState },

    /// Legacy simple inline tool call (not boxed)
    ToolCall {
        tool_name: String,
        args: String,
        result: Option<String>,
    },
    /// System notice or error
    Notice { text: String, is_error: bool },
}

impl TranscriptEntry {
    /// Render (or re-render) the entry's text content into ratatui Lines.
    pub fn render_to_text(&mut self, _width: u16, activity_tick: u64) -> Text<'static> {
        match self {
            TranscriptEntry::User { content } => {
                let mut text = markdown::render_markdown(content);
                // Prepend user marker: cyan chevron
                let marker = Line::from(vec![Span::styled(
                    "> ",
                    Style::default()
                        .fg(theme::PRIMARY)
                        .add_modifier(Modifier::BOLD),
                )]);
                let mut all = vec![marker];
                all.append(&mut text.lines);
                Text::from(all)
            }
            TranscriptEntry::Assistant {
                content,
                rendered,
                is_streaming,
                thinking,
            } => {
                let mut all = Vec::new();

                // Show activity directly, without a separate assistant marker.
                if *is_streaming {
                    let activity = activity_text(activity_tick);
                    all.push(Line::from(vec![Span::styled(
                        activity.trim_start().to_owned(),
                        theme::style_dim(),
                    )]));
                }

                // Reasoning text remains below the activity line.
                if !thinking.is_empty() {
                    let mut thinking_lines = markdown::render_markdown(thinking).lines;
                    for line in thinking_lines.iter_mut() {
                        let dimmed: Vec<Span> = line
                            .spans
                            .iter()
                            .map(|s| Span::styled(s.content.clone(), theme::style_dim()))
                            .collect();
                        all.push(Line::from(dimmed));
                    }
                }

                // Actual response content. Render it both live (so streamed
                // tokens appear as they arrive) and after completion.
                if !content.is_empty() {
                    let mut text = markdown::render_markdown(content);
                    all.append(&mut text.lines);
                }

                // Live response cursor uses a conventional terminal spinner.
                if *is_streaming && !content.is_empty() {
                    let glyph = ACTIVITY_SPINNER[activity_tick as usize % ACTIVITY_SPINNER.len()];
                    all.push(Line::from(Span::styled(
                        format!(" {glyph}"),
                        Style::default().fg(theme::PRIMARY),
                    )));
                }

                let t = Text::from(all);
                *rendered = Some(t.clone());
                t
            }
            TranscriptEntry::ToolCall {
                tool_name,
                args,
                result,
            } => render_tool_call_box_simple(tool_name, args, result, _width),
            TranscriptEntry::ToolCallBox { state } => render_tool_call_compact(state, _width),
            TranscriptEntry::Notice { text, is_error } => {
                // Try to detect a typed error via flat-string prefix so notices
                // get the right neon chip / icon instead of always error-red bold.
                let typed = if *is_error {
                    Some(crate::error::AgentError::from_flat_string(text))
                } else {
                    None
                };
                let is_quiet = typed.as_ref().map(|e| e.is_quiet()).unwrap_or(false);

                let style = if is_quiet {
                    theme::style_dim()
                } else if *is_error {
                    let col = typed
                        .as_ref()
                        .map(|e| e.chip_color())
                        .unwrap_or(theme::ERROR);
                    Style::default().fg(col).add_modifier(Modifier::BOLD)
                } else {
                    theme::style_dim()
                };

                let prefix = match (&typed, *is_error) {
                    (Some(e), _) => format!("{} ", e.icon()),
                    (None, true) => "✗ ".to_string(),
                    (None, false) => "· ".to_string(),
                };

                // Render the chip label inline when it's a typed error.
                if let Some(e) = &typed {
                    let chip = format!("[ {} ] ", e.chip_label());
                    Text::from(Line::from(vec![
                        Span::styled(
                            prefix.clone(),
                            Style::default()
                                .fg(e.chip_color())
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(chip, e.style()),
                        Span::styled(e.message(), style),
                    ]))
                } else {
                    Text::from(Line::from(vec![
                        Span::styled(prefix.to_string(), style),
                        Span::styled(text.clone(), style),
                    ]))
                }
            }
        }
    }

    /// Get the rendered text, rendering if needed.
    pub fn get_rendered(&mut self, width: u16, activity_tick: u64) -> Text<'static> {
        match self {
            TranscriptEntry::Assistant {
                rendered,
                is_streaming,
                ..
            } => {
                // Streaming entries must be regenerated so the spinner advances.
                if !*is_streaming {
                    if let Some(r) = rendered.take() {
                        return r;
                    }
                }
                self.render_to_text(width, activity_tick)
            }
            _ => self.render_to_text(width, activity_tick),
        }
    }
}

// ─── Shared box-rendering helpers ────────────────────────────────────────────

/// Per-tool icon for tool call boxes: 🔧 for generic, 📖 for read, ✏️ for write/edit,
/// 💻 for bash, 🔍 for grep/glob.

/// Build one interior line of the box: `│ <content><pad>│`.

/// Push a centered `├─ Result ─┤` style divider into `lines`.

// ─── Simple tool call box (for legacy ToolCall variant) ──────────────────────

/// Render a compact box for simple tool call entries (not collapsible).
fn render_tool_call_box_simple(
    tool_name: &str,
    args: &str,
    result: &Option<String>,
    avail_width: u16,
) -> Text<'static> {
    let mut state = ToolCallState::new(tool_name.to_string(), args.to_string());
    state.expanded = false;
    if let Some(r) = result.as_ref().filter(|r| !r.trim().is_empty()) {
        state.status = if r.starts_with("ERROR") {
            ToolCallStatus::Errored
        } else {
            ToolCallStatus::Completed
        };
        state.result = Some(r.clone());
        state.result_preview = Some(r.clone());
        if state.status == ToolCallStatus::Errored {
            state.error = crate::error::AgentError::from_flat_string(r)
                .typed_tool_error()
                .or_else(|| {
                    Some(crate::error::ToolCallError::new(
                        tool_name.to_string(),
                        crate::error::ToolErrorKind::ExecutionFailed,
                        r.trim_start_matches("ERROR:").trim().to_string(),
                    ))
                });
        }
    } else if result.is_some() {
        state.status = ToolCallStatus::Completed;
    }
    render_tool_call_compact(&state, avail_width)
}
/// `ToolCallStatus` and the optional typed `ToolCallError`.
fn compute_tool_summary(tool_name: &str, args: &str) -> String {
    let parsed: Option<serde_json::Value> = serde_json::from_str(args).ok();
    let obj = parsed.as_ref().and_then(|v| v.as_object());
    match tool_name {
        "bash" | "shell" | "command" | "run" => {
            let cmd = obj
                .and_then(|o| o.get("command").or(o.get("cmd")).or(o.get("shell")))
                .and_then(|v| v.as_str())
                .unwrap_or(args.trim());
            format!("bash {}", shorten(cmd, 40))
        }
        "write" | "create" => {
            let p = obj
                .and_then(|o| o.get("filePath").or(o.get("path")).or(o.get("file")))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let content = obj
                .and_then(|o| o.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let first_line = content.lines().next().unwrap_or("");
            if first_line.is_empty() {
                format!("write {}", p)
            } else {
                format!("write {} | {}", p, shorten(first_line, 30))
            }
        }
        "edit" | "patch" | "str_replace" => {
            let p = obj
                .and_then(|o| o.get("filePath").or(o.get("path")).or(o.get("file")))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let old_lines = obj
                .and_then(|o| o.get("oldString").or(o.get("oldText")).or(o.get("find")))
                .and_then(|v| v.as_str())
                .map(|s| s.lines().count())
                .unwrap_or(0);
            let new_lines = obj
                .and_then(|o| o.get("newString").or(o.get("newText")).or(o.get("replace")))
                .and_then(|v| v.as_str())
                .map(|s| s.lines().count())
                .unwrap_or(0);
            // Never put edited source into the summary. Only path and counts.
            format!("edit {} · -{} / +{} lines", p, old_lines, new_lines)
        }
        "read" | "view" | "cat" => {
            let p = obj
                .and_then(|o| o.get("filePath").or(o.get("path")).or(o.get("file")))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("read {}", p)
        }
        "glob" | "ls" | "list" => {
            let pat = obj
                .and_then(|o| o.get("pattern").or(o.get("glob")))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("glob {}", pat)
        }
        "grep" | "search" | "rg" => {
            let q = obj
                .and_then(|o| o.get("pattern").or(o.get("query")).or(o.get("search")))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("grep {}", q)
        }
        "web" | "fetch" | "browse" => {
            let u = obj
                .and_then(|o| o.get("url"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("fetch {}", u)
        }
        _ => format!("{} {}", tool_name, shorten(args.trim(), 30)),
    }
}

const COLLAPSED_SOURCE_LINES: usize = 10;
const MAX_RETAINED_SOURCE_LINES: usize = 100;
const MAX_SOURCE_COLUMNS: usize = 240;

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

fn extract_write_preview(tool_name: &str, args: &str) -> Option<WriteCodePreview> {
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

fn extract_edit_preview(tool_name: &str, args: &str) -> Option<EditCodePreview> {
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
fn push_write_code_panel(
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

fn push_edit_diff_panel(
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

/// Render a tool call as a simple green/red box.
/// Green = OK, red = error. Error details only shown when expanded.
fn render_tool_call_compact(state: &ToolCallState, _avail_width: u16) -> Text<'static> {
    if let Some(source_shell) = render_source_tool_shell(state, _avail_width) {
        return source_shell;
    }
    let name = state.tool_name.clone();
    let status = state.status;
    let expanded = state.expanded;

    let (border_color, fill_bg, icon_str) = match status {
        ToolCallStatus::Completed => (theme::SUCCESS, Color::Rgb(6, 18, 10), "✓"),
        ToolCallStatus::Errored => (theme::ERROR, Color::Rgb(22, 8, 8), "✗"),
        ToolCallStatus::Running => (theme::PRIMARY, Color::Rgb(8, 14, 20), "▶"),
        ToolCallStatus::Pending => (theme::DIM, Color::Rgb(8, 10, 14), "⋯"),
    };

    let bstyle = Style::default().fg(border_color).bg(fill_bg);
    let content_style = Style::default().fg(theme::FG).bg(fill_bg);

    // Never force a minimum wider than the actual transcript area. A narrow
    // terminal must produce a narrow box, not overflow into adjacent layout.
    let avail = usize::from(56u16.min(_avail_width.saturating_sub(2)).max(4));

    let mut lines: Vec<Line<'static>> = Vec::new();

    // All rows are `avail + 2` chars wide: 1 left border + `avail` inner
    // chars + 1 right border. Any leading space inside a row must be
    // accounted for by subtracting it from the trailing padding so the
    // inner width — and therefore the full row width — stays constant.
    // (Off-by-one misalignment here distorted the box grid and, at small
    // widths, could underflow and panic the full-screen TUI.)

    // Top border with tool name — total width = avail + 2
    let title = fit_to_width(&format!(" {} {} ", icon_str, name), avail.saturating_sub(1));
    let title_len = title.chars().count();
    // ┌ (1) + ─ (1) + title (N) + dashes (avail − 1 − N) + ┐ (1) = avail + 2
    let right_dashes = avail.saturating_sub(title_len + 1);
    lines.push(Line::from(Span::styled(
        format!("┌─{}{}┐", title, "─".repeat(right_dashes)),
        bstyle,
    )));

    // Summary line — total width = avail + 2. `" "` prefix is one of the
    // inner chars, so trailing padding = avail − 1 − summary_len.
    let summary = fit_to_width(&state.tool_summary, avail.saturating_sub(1));
    let spad = avail
        .saturating_sub(1)
        .saturating_sub(summary.chars().count());
    lines.push(Line::from(vec![
        Span::styled("│", bstyle),
        Span::styled(format!(" {}{}", summary, " ".repeat(spad)), content_style),
        Span::styled("│", bstyle),
    ]));

    // Source-changing tools get bounded nested panels. The renderer never sees
    // their complete payloads, only previews captured at event ingestion.
    push_write_code_panel(&mut lines, state, avail, bstyle, fill_bg);
    push_edit_diff_panel(&mut lines, state, avail, bstyle, fill_bg);

    // Expanded body
    if expanded {
        match status {
            ToolCallStatus::Errored => {
                if let Some(e) = &state.error {
                    // Error label row: inner width = avail.
                    let err_label = " error ";
                    let err_len = err_label.chars().count();
                    let err_pad = avail.saturating_sub(err_len);
                    lines.push(Line::from(vec![
                        Span::styled("│", bstyle),
                        Span::styled(
                            err_label.to_string(),
                            Style::default()
                                .fg(theme::ERROR)
                                .add_modifier(Modifier::BOLD)
                                .bg(fill_bg),
                        ),
                        Span::styled(" ".repeat(err_pad), fill_bg),
                        Span::styled("│", bstyle),
                    ]));
                    for l in e.message.lines().take(4) {
                        let line = fit_to_width(l, avail.saturating_sub(1));
                        let lpad = avail.saturating_sub(1).saturating_sub(line.chars().count());
                        lines.push(Line::from(vec![
                            Span::styled("│", bstyle),
                            Span::styled(
                                format!(" {}{}", line, " ".repeat(lpad)),
                                Style::default().fg(theme::ERROR).bg(fill_bg),
                            ),
                            Span::styled("│", bstyle),
                        ]));
                    }
                }
            }
            ToolCallStatus::Completed => {
                if let Some(preview) = &state.result_preview {
                    for l in preview.lines().take(6) {
                        let line = fit_to_width(l, avail.saturating_sub(1));
                        let lpad = avail.saturating_sub(1).saturating_sub(line.chars().count());
                        lines.push(Line::from(vec![
                            Span::styled("│", bstyle),
                            Span::styled(format!(" {}{}", line, " ".repeat(lpad)), content_style),
                            Span::styled("│", bstyle),
                        ]));
                    }
                }
            }
            _ => {}
        }
    }

    // Bottom border — total width = avail + 2
    lines.push(Line::from(Span::styled(
        format!("└{}┘", "─".repeat(avail)),
        bstyle,
    )));

    Text::from(lines)
}

fn fit_to_width(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let take = max_chars.saturating_sub(1);
    let truncated: String = s.chars().take(take).collect();
    format!("{}…", truncated)
}

fn shorten(s: &str, max: usize) -> String {
    fit_to_width(s, max)
}

// ─── ToolCallState ───────────────────────────────────────────────────────────

/// Tool call execution status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolCallStatus {
    Pending,
    Running,
    Completed,
    Errored,
}

/// State for a boxed tool call entry.
#[derive(Clone)]
pub struct ToolCallState {
    pub tool_name: String,
    /// Bounded arguments preview. Full write payloads are intentionally not retained.
    pub args: String,
    /// Parsed key-value lines for display (computed from bounded args)
    pub args_kv: Vec<(String, String)>,
    /// Bounded code preview for write/create calls.
    pub write_preview: Option<WriteCodePreview>,
    /// Bounded before/after preview for edit/patch calls.
    pub edit_preview: Option<EditCodePreview>,
    /// One-line summary for box header
    pub tool_summary: String,
    /// Full result text, if available
    pub result: Option<String>,
    /// Preview snippet of the result (first N chars)
    pub result_preview: Option<String>,
    /// Whether arguments are expanded (Ctrl+E to toggle)
    pub expanded: bool,
    /// Execution status
    pub status: ToolCallStatus,
    /// Duration string like "12ms"
    pub duration: Option<String>,
    /// Optional typed error — present only when status == Errored
    pub error: Option<crate::error::ToolCallError>,
}

impl ToolCallState {
    pub fn new(tool_name: String, args: String) -> Self {
        let write_preview = extract_write_preview(&tool_name, &args);
        let edit_preview = extract_edit_preview(&tool_name, &args);
        let tool_summary = compute_tool_summary(&tool_name, &args);
        let bounded_args = if write_preview.is_some() || edit_preview.is_some() {
            // Structured previews contain everything the renderer needs. Avoid
            // retaining arbitrarily large source-changing payloads.
            String::new()
        } else {
            fit_to_width(&args, 512)
        };
        let args_kv = parse_args_kv(&bounded_args);
        Self {
            tool_name,
            args: bounded_args,
            args_kv,
            write_preview,
            edit_preview,
            tool_summary,
            result: None,
            result_preview: None,
            expanded: false, // compact-by-default
            status: ToolCallStatus::Running,
            duration: None,
            error: None,
        }
    }

    /// Compute the title string for the bar.
    pub fn title(&self) -> String {
        let icon = match self.status {
            ToolCallStatus::Pending => "⋯",
            ToolCallStatus::Running => "▶",
            ToolCallStatus::Completed => "✓",
            ToolCallStatus::Errored => self.error.as_ref().map(|e| e.kind.icon()).unwrap_or("✗"),
        };
        let dur = self.duration.as_deref().unwrap_or("");
        if self.expanded {
            format!(" {} {} {} ", icon, self.tool_name, dur)
        } else {
            let kv_count = self.args_kv.len();
            let dur_suffix = if !dur.is_empty() {
                format!(" {}", dur)
            } else {
                String::new()
            };
            if self.result_preview.is_some() {
                format!(
                    " {} {} ({} args){}",
                    icon, self.tool_name, kv_count, dur_suffix
                )
            } else {
                format!(
                    " {} {} ({} args){}",
                    icon, self.tool_name, kv_count, dur_suffix
                )
            }
        }
    }
}

/// Parse a JSON arguments string into key-value pairs for clean display.
fn parse_args_kv(args: &str) -> Vec<(String, String)> {
    if args.trim().is_empty() {
        return Vec::new();
    }
    // Try to parse as JSON object
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(args) {
        if let Some(obj) = val.as_object() {
            let mut pairs: Vec<(String, String)> = Vec::new();
            for (k, v) in obj {
                let v_str = match v {
                    serde_json::Value::String(s) => {
                        // Char-safe truncation: byte-slicing here would panic if
                        // the cut point lands inside a multibyte codepoint.
                        // `shorten` already appends the ellipsis when it truncates.
                        if s.chars().count() > 80 {
                            format!("\"{}\"", shorten(s, 79))
                        } else {
                            format!("\"{}\"", s)
                        }
                    }
                    other => other.to_string(),
                };
                pairs.push((k.clone(), v_str));
            }
            return pairs;
        }
    }
    // Fallback: show raw args as a single entry
    // `shorten` appends the ellipsis itself when it truncates.
    let preview = if args.chars().count() > 100 {
        shorten(args, 100)
    } else {
        args.to_string()
    };
    vec![("args".to_string(), preview)]
}

// ─── Scroll State ────────────────────────────────────────────────────────────

/// Scroll state for the transcript.
pub struct ScrollState {
    pub offset: usize,     // Scroll offset in lines from top
    pub auto_scroll: bool, // Whether to follow new content
}

impl Default for ScrollState {
    fn default() -> Self {
        Self {
            offset: 0,
            auto_scroll: true,
        }
    }
}

/// Render the transcript area.
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    entries: &mut [TranscriptEntry],
    scroll: &mut ScrollState,
    activity_tick: u64,
) {
    if area.height < 1 || area.width < 2 {
        return;
    }

    // Build the full rendered text from all entries
    let mut all_lines: Vec<Line<'static>> = Vec::new();
    for entry in entries.iter_mut() {
        let rendered = entry.get_rendered(area.width, activity_tick);
        all_lines.extend(rendered.lines);
    }

    let total_lines = all_lines.len();
    let view_height = area.height as usize;

    // Auto-scroll to bottom
    if scroll.auto_scroll && total_lines > view_height {
        scroll.offset = total_lines.saturating_sub(view_height);
    }

    // Clamp scroll offset
    if total_lines > view_height {
        scroll.offset = scroll.offset.min(total_lines.saturating_sub(view_height));
    } else {
        scroll.offset = 0;
    }

    // Visible slice
    let visible: Vec<Line<'static>> = if total_lines > scroll.offset {
        all_lines[scroll.offset..].to_vec()
    } else {
        all_lines.clone()
    };

    let text = Text::from(visible);

    let para = Paragraph::new(text)
        .block(Block::default().style(Style::default().bg(theme::BG)))
        .style(Style::default().bg(theme::BG))
        .wrap(Wrap { trim: false });
    para.render(area, buf);
}

/// Scroll up by `delta` lines.
pub fn scroll_up(scroll: &mut ScrollState, delta: usize) {
    scroll.auto_scroll = false;
    scroll.offset = scroll.offset.saturating_sub(delta);
}

/// Scroll down by `delta` lines.
pub fn scroll_down(scroll: &mut ScrollState, total_lines_hint: usize, delta: usize) {
    let max_offset = total_lines_hint.saturating_sub(1);
    if scroll.offset + delta >= max_offset {
        scroll.auto_scroll = true;
        scroll.offset = 0;
    } else {
        scroll.offset = scroll.offset.saturating_add(delta);
    }
}

/// Scroll to top.
pub fn scroll_top(scroll: &mut ScrollState) {
    scroll.auto_scroll = false;
    scroll.offset = 0;
}

/// Scroll to bottom.
pub fn scroll_bottom(scroll: &mut ScrollState) {
    scroll.auto_scroll = true;
    scroll.offset = 0;
}

// ─── Transcript Component ────────────────────────────────────────────────────

use crate::tui::component::{Action, Component};

/// Aggregated transcript state: entries, scroll, conversation history, streaming channel.
pub struct Transcript {
    pub entries: Vec<TranscriptEntry>,
    pub scroll: ScrollState,
    pub messages: Vec<providers::ChatMessage>,
    pub stream_event_rx:
        Option<tokio::sync::mpsc::UnboundedReceiver<super::component::UiStreamEvent>>,
    pub streaming_fragment: String,
    pub tools_expanded: bool,
    pub activity_tick: u64,
}

impl Transcript {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            scroll: ScrollState::default(),
            messages: Vec::new(),
            stream_event_rx: None,
            streaming_fragment: String::new(),
            tools_expanded: false,
            activity_tick: 0,
        }
    }

    /// Restore provider history and UI entries from a loaded session.
    /// Exact ChatMessage history is preserved for the LLM; UI entries are approximate.
    pub fn load_from_session(&mut self, messages: Vec<providers::ChatMessage>) {
        self.messages = messages;
        let ui = crate::session::messages_to_transcript_entries(&self.messages);
        self.entries.extend(ui);
        self.scroll.auto_scroll = true;
    }

    pub fn tick_activity(&mut self) {
        self.activity_tick = self.activity_tick.wrapping_add(1);
    }

    /// Globally expand or collapse all structured tool executions.
    pub fn set_tools_expanded(&mut self, expanded: bool) {
        self.tools_expanded = expanded;
        for entry in &mut self.entries {
            if let TranscriptEntry::ToolCallBox { state } = entry {
                state.expanded = expanded;
            }
        }
        self.scroll.auto_scroll = true;
    }

    /// Process one streaming event from the channel. Returns an action for the caller.
    pub fn process_stream_event(&mut self, event: &super::component::UiStreamEvent) -> Action {
        match event {
            super::component::UiStreamEvent::Token(t) => {
                self.streaming_fragment.push_str(t);
                let follows_tool = matches!(
                    self.entries.last(),
                    Some(TranscriptEntry::ToolCall { .. } | TranscriptEntry::ToolCallBox { .. })
                );
                if follows_tool {
                    let drop_idx = if self.entries.len() >= 2 {
                        let i = self.entries.len() - 2;
                        match &self.entries[i] {
                            TranscriptEntry::Assistant { content, .. } if content.is_empty() => {
                                Some(i)
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if let Some(i) = drop_idx {
                        self.entries.remove(i);
                    }
                    self.entries.push(TranscriptEntry::Assistant {
                        content: String::new(),
                        rendered: None,
                        is_streaming: true,
                        thinking: String::new(),
                    });
                }
                for entry in self.entries.iter_mut().rev() {
                    if let TranscriptEntry::Assistant {
                        content,
                        rendered,
                        is_streaming,
                        ..
                    } = entry
                    {
                        content.push_str(t);
                        *rendered = None;
                        *is_streaming = true;
                        break;
                    }
                }
                Action::Noop
            }
            super::component::UiStreamEvent::Thinking(t) => {
                for entry in self.entries.iter_mut().rev() {
                    if let TranscriptEntry::Assistant {
                        ref mut thinking,
                        ref mut rendered,
                        is_streaming,
                        ..
                    } = entry
                    {
                        thinking.push_str(t);
                        *rendered = None;
                        *is_streaming = true;
                        break;
                    }
                }
                Action::Noop
            }
            super::component::UiStreamEvent::ThinkingDone => {
                for entry in self.entries.iter_mut().rev() {
                    if let TranscriptEntry::Assistant {
                        ref mut is_streaming,
                        ..
                    } = entry
                    {
                        *is_streaming = false;
                        break;
                    }
                }
                Action::Noop
            }
            super::component::UiStreamEvent::ToolCall { name, args } => {
                // Parse once while the complete JSON is available. ToolCallState
                // retains only bounded display data; large write source is dropped.
                let mut state = ToolCallState::new(name.clone(), args.clone());
                state.expanded = self.tools_expanded;
                self.entries.push(TranscriptEntry::ToolCallBox { state });
                Action::Noop
            }
            super::component::UiStreamEvent::ToolResult {
                name,
                success,
                output,
            } => {
                // Char-safe truncation (byte slicing can panic on multibyte UTF-8).
                // `shorten` appends the ellipsis itself when it truncates.
                let preview: String = if output.chars().count() > 200 {
                    shorten(output, 200)
                } else {
                    output.clone()
                };
                for entry in self.entries.iter_mut().rev() {
                    match entry {
                        TranscriptEntry::ToolCallBox { state } => {
                            state.result = Some(preview.clone());
                            state.result_preview = Some(preview.clone());
                            if *success {
                                state.status = ToolCallStatus::Completed;
                            } else {
                                let flat = format!("ERROR: {}", preview);
                                let typed = crate::error::AgentError::from_flat_string(&flat)
                                    .typed_tool_error()
                                    .unwrap_or_else(|| {
                                        crate::error::ToolCallError::new(
                                            name.clone(),
                                            crate::error::ToolErrorKind::ExecutionFailed,
                                            preview.trim_start_matches("ERROR:").trim().to_string(),
                                        )
                                    });
                                state.status = ToolCallStatus::Errored;
                                state.error = Some(typed);
                            }
                            break;
                        }
                        TranscriptEntry::ToolCall { result, .. } => {
                            *result = Some(if *success {
                                preview.clone()
                            } else {
                                format!("ERROR: {}", preview)
                            });
                            break;
                        }
                        _ => {}
                    }
                }
                Action::Noop
            }
            super::component::UiStreamEvent::Done {
                full: _,
                tokens_in,
                tokens_out,
                messages,
            } => {
                // Flip the trailing assistant entry out of streaming mode so
                // its content renders (and the live cursor disappears).
                for entry in self.entries.iter_mut().rev() {
                    if let TranscriptEntry::Assistant {
                        is_streaming,
                        rendered,
                        ..
                    } = entry
                    {
                        *is_streaming = false;
                        *rendered = None;
                        break;
                    }
                }
                self.messages = messages.clone();
                Action::StreamDone {
                    tokens_in: *tokens_in,
                    tokens_out: *tokens_out,
                }
            }
            super::component::UiStreamEvent::Error(e) => {
                self.entries.push(TranscriptEntry::Notice {
                    text: e.clone(),
                    is_error: true,
                });
                if let Some(last) = self.entries.last() {
                    if let TranscriptEntry::Assistant { content, .. } = last {
                        if content.is_empty() {
                            self.entries.pop();
                        }
                    }
                }
                Action::StreamError
            }
        }
    }
}

impl Component for Transcript {
    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        use crossterm::event::{KeyCode, KeyEventKind};
        if key.kind != KeyEventKind::Press {
            return Action::Noop;
        }
        match key.code {
            KeyCode::Up => Action::ScrollUp(3),
            KeyCode::Down => Action::ScrollDown(3),
            KeyCode::PageUp => Action::ScrollUp(10),
            KeyCode::PageDown => Action::ScrollDown(10),
            _ => Action::Noop,
        }
    }

    fn render(&mut self, f: &mut ratatui::Frame, area: Rect) {
        render(
            area,
            f.buffer_mut(),
            &mut self.entries,
            &mut self.scroll,
            self.activity_tick,
        );
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn text_to_string(t: &Text<'static>) -> String {
        t.lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn streaming_assistant_uses_activity_spinner_not_thinking_label() {
        let mut entry = TranscriptEntry::Assistant {
            content: String::new(),
            rendered: None,
            is_streaming: true,
            thinking: String::new(),
        };
        let first = text_to_string(&entry.get_rendered(60, 0));
        let second = text_to_string(&entry.get_rendered(60, 1));
        assert!(first.contains("⠋ Cooking…"));
        assert!(second.contains("⠙ Cooking…"));
        assert!(!first.contains('◆'));
        assert!(!first.to_lowercase().contains("thinking"));
    }

    #[test]
    fn test_parse_args_kv_empty() {
        let kv = parse_args_kv("");
        assert!(kv.is_empty());
    }

    #[test]
    fn test_parse_args_kv_json_object() {
        let kv = parse_args_kv(r#"{"filePath": "src/main.rs", "limit": 2000}"#);
        assert!(kv.len() >= 2);
        assert!(kv.iter().any(|(k, _)| k == "filePath"));
        assert!(kv.iter().any(|(k, _)| k == "limit"));
    }

    #[test]
    fn test_tool_call_state_title_pending() {
        let state = ToolCallState::new("read".into(), r#"{}"#.into());
        let title = state.title();
        assert!(title.contains("read"));
        assert!(title.contains("▶"));
    }

    #[test]
    fn print_all_tool_boxes() {
        let test_data: Vec<(&str, &str, Option<&str>)> = vec![
            ("read", r#"{"filePath": "src/main.rs"}"#, Some("pub fn main() {\n    println!(\"hello\");\n}")),
            ("write", r#"{"filePath": "hello.txt", "content": "Hello world"}"#, Some("wrote 11 bytes to hello.txt")),
            ("edit", r#"{"filePath": "src/main.rs", "oldString": "foo", "newString": "bar"}"#, Some("patched src/main.rs")),
            ("bash", "cargo build", Some("Compiling omega-core v0.1.0\nerror[E0425]: cannot find value `x` in this scope\n\nerror: could not compile `omega-core` (lib) due to 1 previous error")),
            ("glob", r#"{"pattern": "**/*.rs"}"#, Some("src/main.rs\nsrc/lib.rs\nsrc/utils.rs")),
            ("grep", r#"{"pattern": "fn main", "include": "*.rs"}"#, Some("src/main.rs:42: pub fn main() {")),
            ("bash", "", None),
        ];

        println!("\n═══ Tool call box renders ═══\n");
        for (tool, args, result) in &test_data {
            let result_owned = result.map(|s| s.to_string());
            let entry = TranscriptEntry::ToolCall {
                tool_name: tool.to_string(),
                args: args.to_string(),
                result: result_owned,
            };
            let mut entry_clone = entry.clone();
            let rendered = entry_clone.get_rendered(80, 0);
            println!("→ {} {} {}", tool, args, result.unwrap_or("(running)"));
            println!("{}", text_to_string(&rendered));
            println!();
        }
    }

    #[test]
    fn write_tool_code_is_boxed_and_width_safe() {
        let content = (1..=10)
            .map(|n| format!("\tprintln!(\"line {} — Ω\");", n))
            .collect::<Vec<_>>()
            .join("\n");
        let args = serde_json::json!({
            "filePath": "src/Ωmega.rs",
            "content": content,
        })
        .to_string();
        let state = ToolCallState::new("write".into(), args);
        assert!(
            state.args.is_empty(),
            "full write payload must not be retained"
        );
        assert_eq!(state.write_preview.as_ref().unwrap().lines.len(), 10);
        let mut entry = TranscriptEntry::ToolCallBox { state };

        let rendered = entry.get_rendered(60, 0);
        let output = text_to_string(&rendered);
        let lines: Vec<&str> = output.lines().collect();

        assert!(output.contains("write  src/Ωmega.rs"));
        assert!(output.contains("RUNNING"));
        assert!(output.contains("println!"));
        assert!(output.contains("[Ctrl+E] expand"));

        let expected = lines[0].chars().count();
        for (index, line) in lines.iter().enumerate() {
            assert_eq!(
                line.chars().count(),
                expected,
                "write panel line {} has inconsistent width: '{}'",
                index,
                line
            );
        }
    }

    #[test]
    fn large_write_event_keeps_bounded_transcript_state() {
        let long_line = "Ω".repeat(10_000);
        let content = std::iter::repeat(long_line)
            .take(2_000)
            .collect::<Vec<_>>()
            .join("\n");
        let original_chars = content.chars().count();
        let args = serde_json::json!({
            "filePath": "src/huge.rs",
            "content": content,
        })
        .to_string();

        let mut transcript = Transcript::new();
        transcript.process_stream_event(&super::super::component::UiStreamEvent::ToolCall {
            name: "write".into(),
            args,
        });

        let TranscriptEntry::ToolCallBox { state } = transcript.entries.last().unwrap() else {
            panic!("write event should create bounded ToolCallBox state");
        };
        let preview = state.write_preview.as_ref().unwrap();
        assert!(state.args.is_empty());
        assert_eq!(preview.lines.len(), MAX_RETAINED_SOURCE_LINES);
        assert_eq!(preview.omitted_lines, 2_000 - MAX_RETAINED_SOURCE_LINES);
        let retained_chars: usize = preview.lines.iter().map(|line| line.chars().count()).sum();
        assert!(retained_chars <= MAX_RETAINED_SOURCE_LINES * MAX_SOURCE_COLUMNS);
        assert!(retained_chars < original_chars / 500);
    }

    #[test]
    fn large_edit_is_bounded_and_does_not_render_all_code() {
        let old = (1..=2_000)
            .map(|n| format!("old line {} {}", n, "Ω".repeat(200)))
            .collect::<Vec<_>>()
            .join("\n");
        let new = (1..=2_000)
            .map(|n| {
                if n == 2_000 {
                    "SENTINEL_MUST_NOT_RENDER".to_string()
                } else {
                    format!("new line {} {}", n, "λ".repeat(200))
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let args = serde_json::json!({
            "filePath": "src/large.rs",
            "oldString": old,
            "newString": new,
        })
        .to_string();

        let state = ToolCallState::new("edit".into(), args);
        assert!(
            state.args.is_empty(),
            "full edit payload must not be retained"
        );
        let preview = state.edit_preview.as_ref().unwrap();
        assert_eq!(preview.removed.len(), MAX_RETAINED_SOURCE_LINES);
        assert_eq!(preview.added.len(), MAX_RETAINED_SOURCE_LINES);
        assert_eq!(preview.omitted_removed, 2_000 - MAX_RETAINED_SOURCE_LINES);
        assert_eq!(preview.omitted_added, 2_000 - MAX_RETAINED_SOURCE_LINES);

        let mut entry = TranscriptEntry::ToolCallBox { state };
        let rendered = entry.get_rendered(60, 0);
        let output = text_to_string(&rendered);
        assert!(output.contains("edit  src/large.rs"));
        assert!(output.contains("RUNNING"));
        assert!(output.contains("more lines"));
        assert!(!output.contains("SENTINEL_MUST_NOT_RENDER"));

        let lines: Vec<&str> = output.lines().collect();
        let expected = lines[0].chars().count();
        for (index, line) in lines.iter().enumerate() {
            assert_eq!(
                line.chars().count(),
                expected,
                "edit panel line {} has inconsistent width: '{}'",
                index,
                line
            );
        }
    }

    #[test]
    fn global_expansion_applies_to_existing_and_new_source_tools() {
        let args = serde_json::json!({
            "filePath": "src/main.rs",
            "content": (1..=20).map(|n| format!("line {}", n)).collect::<Vec<_>>().join("\n"),
        })
        .to_string();
        let mut transcript = Transcript::new();
        transcript.process_stream_event(&super::super::component::UiStreamEvent::ToolCall {
            name: "write".into(),
            args: args.clone(),
        });
        transcript.set_tools_expanded(true);
        transcript.process_stream_event(&super::super::component::UiStreamEvent::ToolCall {
            name: "write".into(),
            args,
        });

        for entry in &transcript.entries {
            let TranscriptEntry::ToolCallBox { state } = entry else {
                continue;
            };
            assert!(state.expanded);
        }
    }

    #[test]
    fn tool_box_borders_close() {
        let mut entry = TranscriptEntry::ToolCall {
            tool_name: "bash".into(),
            args: "cargo build --release".into(),
            result: Some("Compiling...\nFinished\n".into()),
        };
        let rendered = entry.get_rendered(60, 0);
        let s = text_to_string(&rendered);
        let lines: Vec<&str> = s.lines().collect();

        assert!(
            lines[0].starts_with("┌─"),
            "top border should start with ┌─"
        );
        assert!(lines[0].ends_with("┐"), "top border should end with ┐");

        for line in &lines[1..lines.len() - 1] {
            if line.starts_with("├") || line.starts_with("└") {
                continue;
            }
            assert!(line.starts_with("│"), "content lines should start with │");
            assert!(line.ends_with("│"), "content lines should end with │");
        }

        assert!(
            lines.last().unwrap().starts_with("└"),
            "bottom border should start with └"
        );
        assert!(
            lines.last().unwrap().ends_with("┘"),
            "bottom border should end with ┘"
        );

        let widths: Vec<usize> = lines.iter().map(|l| l.chars().count()).collect();
        let expected = widths[0];
        for (i, w) in widths.iter().enumerate() {
            assert_eq!(
                *w, expected,
                "line {} is {} chars wide, expected {}: '{}'",
                i, w, expected, lines[i]
            );
        }

        println!(
            "Tool box borders OK — {} lines, {} chars wide",
            lines.len(),
            expected
        );
    }
}
