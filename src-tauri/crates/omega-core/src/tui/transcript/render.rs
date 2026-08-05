//! Transcript rendering (P5 split).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use super::state::ScrollState;
use super::TranscriptEntry;
use crate::tui::theme;

/// A render segment: either a user message card (with background color) or
/// a group of flat lines (assistant, tool calls, notices - no card styling).
struct RenderSegment {
    lines: Vec<Line<'static>>,
    bg: Option<Color>,
    fg: Option<Color>,
}

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

    // Build render segments from all entries.
    // Track whether any preceding entries exist to identify "middle" user prompts.
    let mut segments: Vec<RenderSegment> = Vec::new();
    let mut has_previous_entries = false;
    for entry in entries.iter_mut() {
        let rendered = entry.get_rendered(area.width, activity_tick);
        match entry {
            TranscriptEntry::User { .. } => {
                let bg = if entry.has_attachments() {
                    Some(theme::USER_ATTACH_BG)
                } else if has_previous_entries {
                    // User prompts after any prior content use a distinct gray card
                    Some(theme::USER_MIDDLE_CARD_BG)
                } else {
                    // First user prompt uses the standard card background
                    Some(theme::USER_CARD_BG)
                };
                let fg = if entry.has_attachments() {
                    Some(theme::USER_ATTACH_FG)
                } else {
                    None
                };
                segments.push(RenderSegment {
                    lines: rendered.lines,
                    bg,
                    fg,
                });
                // Mark that we now have content before future entries
                has_previous_entries = true;
            }
            _ => {
                segments.push(RenderSegment {
                    lines: rendered.lines,
                    bg: None,
                    fg: None,
                });
                // Non-user entries count as preceding content for subsequent users
                has_previous_entries = true;
            }
        }
    }

    // Compute total line count (including separator rows between adjacent user cards)
    let total_lines: usize = segments.iter().map(|s| s.lines.len()).sum();
    // Add separator rows: 2 rows between each pair of adjacent user-card segments
    let mut separator_count = 0usize;
    for i in 1..segments.len() {
        if segments[i].bg.is_some() && segments[i - 1].bg.is_some() {
            separator_count += 2;
        }
    }
    let total_lines = total_lines + separator_count;
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

    // Fill entire area with base background
    fill_area_buf(buf, area, theme::BG);

    // Render visible segments at correct y positions
    let mut y = 0usize;
    let mut remaining = scroll.offset;

    for (i, seg) in segments.iter().enumerate() {
        let seg_height = seg.lines.len();

        // Skip segments entirely before the visible window
        if remaining >= seg_height {
            remaining -= seg_height;
            // Skip separator rows if they exist (2 rows per separator)
            if i + 1 < segments.len()
                && segments[i + 1].bg.is_some()
                && seg.bg.is_some()
            {
                if remaining >= 2 {
                    remaining -= 2;
                } else {
                    remaining = 0;
                }
            }
            continue;
        }

        // This segment is (partially) visible
        let start_line = remaining;
        let visible_lines = seg_height - start_line;
        let available = view_height.saturating_sub(y);

        if visible_lines > 0 && available > 0 {
            // User cards: minimum 3 lines so padding fits; non-bg segments use natural height
            let content_count = visible_lines.min(seg.lines.len());
            let render_count = if seg.bg.is_some() {
                content_count.max(3).min(available)
            } else {
                content_count.min(available)
            };
            let render_area = Rect::new(
                area.x,
                area.y + y as u16,
                area.width,
                render_count as u16,
            );

            if let Some(bg) = seg.bg {
                // User card: fill the full render area with the card background
                // so the card stays at its current position, then render text
                // with padding around the edges.
                fill_area_buf(buf, render_area, bg);

                // Horizontal padding is always 1 char. Vertical spacing varies by card type:
                // - First user card: minimal spacing (top=1, bottom=0 effectively)
                // - Middle user cards: more bottom spacing for better readability
                // - Attachment cards: normal spacing
                let pad_h = 1u16;
                let pad_v_top_desired = 2u16;
                let pad_v_bottom_desired = 1u16;

                // Clamp padding so text always gets at least 1 line
                let total_desired = pad_v_top_desired + pad_v_bottom_desired;
                let (pad_v_top, pad_v_bottom) = if render_area.height > total_desired + 1 {
                    (pad_v_top_desired, pad_v_bottom_desired)
                } else if render_area.height > 2 {
                    let pad_v_top = (render_area.height / 3).max(1);
                    let pad_v_bottom = render_area.height.saturating_sub(pad_v_top).saturating_sub(1);
                    (pad_v_top, pad_v_bottom)
                } else {
                    (0u16, 0u16)
                };

                let text_area_height = render_area.height
                    .saturating_sub(pad_v_top)
                    .saturating_sub(pad_v_bottom)
                    .max(1);

                let pad = pad_h;
                let text_area = Rect::new(
                    render_area.x.saturating_add(pad),
                    render_area.y.saturating_add(pad_v_top),
                    render_area.width.saturating_sub(pad_h * 2),
                    text_area_height,
                );
                // Set foreground color based on segment type
                let fg_color = seg.fg.unwrap_or(theme::FG);
                let para_style = Style::default().bg(bg).fg(fg_color);
                let actual_lines = content_count.min(seg.lines.len().saturating_sub(start_line));
                let visible_seg_lines: Vec<Line<'static>> =
                    seg.lines[start_line..start_line + actual_lines].to_vec();
                let text = Text::from(visible_seg_lines);
                let para = Paragraph::new(text)
                    .style(para_style)
                    .wrap(Wrap { trim: false });
                para.render(text_area, buf);
            } else {
                // Flat segment: render without background
                let actual_lines = content_count.min(seg.lines.len().saturating_sub(start_line));
                let visible_seg_lines: Vec<Line<'static>> =
                    seg.lines[start_line..start_line + actual_lines].to_vec();
                let text = Text::from(visible_seg_lines);
                let para = Paragraph::new(text)
                    .style(Style::default().bg(theme::BG))
                    .wrap(Wrap { trim: false });
                para.render(render_area, buf);
            }

            y += render_count;
        }

        // Add separator between adjacent user cards — a subtle 2-row gap
        // that uses USER_CARD_SEPARATOR for a clean visual break between cards.
        if i + 1 < segments.len()
            && segments[i + 1].bg.is_some()
            && seg.bg.is_some()
            && y < view_height
        {
            let sep_rows = 2usize;
            for row in 0..sep_rows {
                if y + row >= view_height {
                    break;
                }
                for x in area.x..area.x + area.width {
                    buf.get_mut(x, area.y + (y + row) as u16).set_bg(theme::USER_CARD_SEPARATOR);
                }
            }
            y += sep_rows;
        }

        if y >= view_height {
            break;
        }
        remaining = 0;
    }
}

/// Fill a rect in the buffer with a solid background color.
pub fn fill_area_buf(buf: &mut Buffer, area: Rect, color: Color) {
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            buf.get_mut(x, y).set_bg(color);
        }
    }
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
