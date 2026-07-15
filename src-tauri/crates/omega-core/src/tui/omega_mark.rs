use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

use super::theme;

/// Agent state passed to the Omega mark for reactive animation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Idle,
    Thinking,
    Streaming,
}

/// Phase key used by the tick-driven animation.
#[derive(Clone, Copy)]
pub struct AnimationPhase {
    pub tick: u64,
    pub agent: AgentState,
}

impl Widget for &AnimationPhase {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let w = area.width;

        if w < 50 {
            return;
        }

        let (glyph, dots, accent_mod) = if w >= 82 {
            full_omega(self)
        } else {
            compact_omega(self)
        };

        let glyph_height = glyph.len() as u16;
        let glyph_width = glyph.iter().map(|l| l.chars().count() as u16).max().unwrap_or(0);
        let start_y = area.top().saturating_add(
            area.height.saturating_sub(glyph_height).saturating_sub(2) / 2,
        );
        let start_x = area.left().saturating_add(
            area.width.saturating_sub(glyph_width) / 2,
        );

        let padded: Vec<String> = glyph
            .iter()
            .map(|line| format!("{:^width$}", line, width = glyph_width as usize))
            .collect();

        let accent = if let Some(modif) = accent_mod {
            Style::default().fg(theme::ACCENT).add_modifier(modif)
        } else {
            Style::default().fg(theme::ACCENT)
        };
        let dim_style = theme::style_dim();

        for (row, line_str) in padded.iter().enumerate() {
            let y = start_y + row as u16;
            if y >= area.bottom() {
                break;
            }
            for (col, ch) in line_str.chars().enumerate() {
                let x = start_x + col as u16;
                if x >= area.right() {
                    break;
                }
                if let Some(cell) = theme::buf_cell_mut(buf, x, y) {
                    let style = if ch == 'Ω' || ch == 'ω' {
                        accent
                    } else if ch == '▓' || ch == '▒' || ch == '█' {
                        dim_style
                    } else {
                        Style::default()
                    };
                    cell.set_char(ch);
                    cell.set_style(style);
                }
            }
        }

        for (dx, dy, dot_char) in dots {
            let x = (start_x as i32 + dx as i32 + glyph_width as i32 / 2).max(0) as u16;
            let y = (start_y as i32 + dy as i32 + glyph_height as i32 / 2).max(0) as u16;
            if y < area.bottom() && x < area.right() && x >= area.left() {
                if let Some(cell) = theme::buf_cell_mut(buf, x, y) {
                    cell.set_char(dot_char);
                    cell.set_style(accent);
                }
            }
        }
    }
}

// ── Large version ──────────────────────────────────────────────────────────

/// Build the full-size Omega glyph (≈21×9) and dot positions.
fn full_omega(phase: &AnimationPhase) -> (Vec<&'static str>, Vec<(i16, i16, char)>, Option<Modifier>) {
    let glyph = vec![
        "▓▓▓▓▓▓▓▓▓▓▓▓▓",
        "▓▓▒▒▒▒▒▒▒▒▒▒▒▒▒▒▓▓",
        "▓▓▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▓▓",
        "▓▓▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▓▓",
        "▓▓▒▒▒▒▒▒▒▒▒Ω▒▒▒▒▒▒▒▒▓▓",
        "▓▓▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▓▓",
        "▓▓▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▓▓",
        "▓▓▒▒▒▒▒▒▒▒▒▒▒▒▒▒▓▓",
        "▓▓▓▓▓▓▓▓▓▓▓▓▓",
    ];

    // Braille dots orbiting at six positions, rotating with tick
    let t = phase.tick as f64;

    if w >= 82 {
        // Large version: geometric orbital rings
        let (glyph_width, glyph_height) = (29u16, 11u16);
        let start_y = area.top().saturating_add(
            area.height.saturating_sub(glyph_height).saturating_sub(2) / 2,
        );
        let start_x = area.left().saturating_add(
            area.width.saturating_sub(glyph_width) / 2,
        );
        render_large(start_x, start_y, buf, phase, t, accent, dim_style);
    } else {
        // Compact version: simplified orbital
        let (glyph_width, glyph_height) = (21u16, 7u16);
        let start_y = area.top().saturating_add(
            area.height.saturating_sub(glyph_height).saturating_sub(2) / 2,
        );
        let start_x = area.left().saturating_add(
            area.width.saturating_sub(glyph_width) / 2,
        );
        render_compact(start_x, start_y, buf, phase, t, accent, dim_style);
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Point {
    x: i16,
    y: i16,
}

/// Plot points along an ellipse of the given radii, sampled at `count` positions.
/// Returns rounded integer offsets relative to the center.
fn ellipse_points(rx: f64, ry: f64, count: usize, rotation: f64) -> Vec<Point> {
    (0..count)
        .map(|i| {
            let angle = rotation + (i as f64) * 2.0 * std::f64::consts::PI / (count as f64);
            Point {
                x: (angle.cos() * rx).round() as i16,
                y: (angle.sin() * ry).round() as i16,
            }
        })
        .collect()
}

/// Compute the pulse modifier (bold ⇄ normal) for the given state and tick.
fn pulse_modifier(phase: &AnimationPhase) -> Option<Modifier> {
    match phase.agent {
        AgentState::Idle => {
            // Subtle breathe: bold every 10 ticks
            if (phase.tick / 10) % 2 == 0 {
                Some(Modifier::BOLD)
            } else {
                None
            }
        }
        AgentState::Thinking => Some(Modifier::BOLD),
        AgentState::Streaming => {
            if (phase.tick / 3) % 2 == 0 {
                Some(Modifier::BOLD)
            } else {
                None
            }
        }
    }
}

/// Pick the dot character describing a point on an orbiting ring.
/// During streaming, dots scramble into braille glyphs.
fn ring_dot_char(phase: &AnimationPhase, ring_idx: usize, i: usize, t: f64) -> char {
    if phase.agent == AgentState::Streaming {
        let sets = [
            ['⠿', '⠾', '⠽', '⠼', '⠻', '⠺'],
            ['⠛', '⠟', '⠿', '⠾', '⠽', '⠼'],
            ['⠉', '⠃', '⠋', '⠛', '⠟', '⠿'],
        ];
        let set = sets[ring_idx % sets.len()];
        let scramble = ((t * 3.0 + ring_idx as f64).floor() as usize) % set.len();
        set[(i + scramble) % set.len()]
    } else {
        '·'
    }
}

/// Stroke a horizontal arc segment (e.g. `╭─────╮`) centered at the given
/// buffer-absolute column on row `y`. The half-width controls the inner span.
fn stroke_top_arc(
    cx: u16,
    y: u16,
    half_w: u16,
    style: Style,
    buf: &mut Buffer,
    area_left: u16,
    area_right: u16,
) {
    set_cell(area_left, area_right, y, cx.saturating_sub(half_w), '╭', style, buf);
    set_cell(area_left, area_right, y, cx.saturating_add(half_w), '╮', style, buf);
    for i in 1..half_w {
        set_cell(area_left, area_right, y, cx.saturating_sub(i), '─', style, buf);
        set_cell(area_left, area_right, y, cx.saturating_add(i), '─', style, buf);
    }
}

/// Stroke a bottom arc segment (e.g. `╰─────╯`).
fn stroke_bottom_arc(
    cx: u16,
    y: u16,
    half_w: u16,
    style: Style,
    buf: &mut Buffer,
    area_left: u16,
    area_right: u16,
) {
    set_cell(area_left, area_right, y, cx.saturating_sub(half_w), '╰', style, buf);
    set_cell(area_left, area_right, y, cx.saturating_add(half_w), '╯', style, buf);
    for i in 1..half_w {
        set_cell(area_left, area_right, y, cx.saturating_sub(i), '─', style, buf);
        set_cell(area_left, area_right, y, cx.saturating_add(i), '─', style, buf);
    }
}

/// Stroke a vertical side segment connecting two y rows at a fixed x column.
fn stroke_side(
    x: u16,
    top_y: u16,
    bottom_y: u16,
    style: Style,
    buf: &mut Buffer,
    area_left: u16,
    area_right: u16,
    area_top: u16,
    area_bottom: u16,
) {
    for y in top_y..=bottom_y {
        set_cell_bounded(
            area_left, area_right, area_top, area_bottom, x, y, '│', style, buf,
        );
    }
}

fn set_cell(
    left: u16,
    right: u16,
    y: u16,
    x: u16,
    ch: char,
    style: Style,
    buf: &mut Buffer,
) {
    set_cell_bounded(left, right, 0, u16::MAX, x, y, ch, style, buf);
}

fn set_cell_bounded(
    left: u16,
    right: u16,
    top: u16,
    bottom: u16,
    x: u16,
    y: u16,
    ch: char,
    style: Style,
    buf: &mut Buffer,
) {
    if x < left || x >= right || y < top || y >= bottom {
        return;
    }
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_char(ch);
        cell.set_style(style);
    }
}

// ── Large version ──────────────────────────────────────────────────────────

fn render_large(
    start_x: u16,
    start_y: u16,
    buf: &mut Buffer,
    phase: &AnimationPhase,
    t: f64,
    accent: Style,
    dim: Style,
) {
    let cx = start_x + 14;
    let cy = start_y + 5;
    let right = start_x + 29;
    let bottom = start_y + 11;

    // ── Static base shape: two concentric rounded rings ─────────────────
    // Outer ring: 26 wide × 9 tall (half-width 13, rows cy-4 .. cy+4)
    let outer_style = dim;
    stroke_top_arc(cx, cy - 4, 13, outer_style, buf, start_x, right);
    stroke_bottom_arc(cx, cy + 4, 13, outer_style, buf, start_x, right);
    stroke_side(cx - 13, cy - 3, cy + 3, outer_style, buf, start_x, right, start_y, bottom);
    stroke_side(cx + 13, cy - 3, cy + 3, outer_style, buf, start_x, right, start_y, bottom);

    // Inner ring: 18 wide × 5 tall (half-width 9, rows cy-2 .. cy+2)
    let inner_half = 9u16;
    stroke_top_arc(cx, cy - 2, inner_half, outer_style, buf, start_x, right);
    stroke_bottom_arc(cx, cy + 2, inner_half, outer_style, buf, start_x, right);
    stroke_side(cx - 9, cy - 1, cy + 1, outer_style, buf, start_x, right, start_y, bottom);
    stroke_side(cx + 9, cy - 1, cy + 1, outer_style, buf, start_x, right, start_y, bottom);

    // ── Central Ω glyph ────────────────────────────────────────────────
    let center_mod = pulse_modifier(phase);
    let center_style = if let Some(m) = center_mod {
        accent.add_modifier(m)
    } else {
        accent
    };
    set_cell_bounded(start_x, right, start_y, bottom, cx, cy, 'Ω', center_style, buf);

    // ── Orbital dots: cyan accent traveling along the outer ring ────────
    // Speed and direction depend on agent state.
    let (speed, dot_count, radius_scale) = match phase.agent {
        AgentState::Idle => (0.06, 6, 1.0),
        AgentState::Thinking => (0.16, 8, 0.62), // converge inward
        AgentState::Streaming => (0.28, 10, 1.0),
    };

    // Orbit on a rounded-rectangle path approximated by an ellipse.
    let ox = 13.0 * radius_scale;
    let oy = 3.5 * radius_scale;

    let orbital_style = accent;
    let pts = ellipse_points(ox, oy, dot_count, t * speed);
    for (i, p) in pts.iter().enumerate() {
        let px = (cx as i32 + p.x as i32).max(0) as u16;
        let py = (cy as i32 + p.y as i32).max(0) as u16;
        let ch = ring_dot_char(phase, 0, i, t);
        set_cell_bounded(start_x, right, start_y, bottom, px, py, ch, orbital_style, buf);
    }

    // A second, slower counter-rotating ring of dim ticks on the inner ring.
    let (inner_speed, inner_count) = match phase.agent {
        AgentState::Idle => (-0.04, 4),
        AgentState::Thinking => (-0.10, 6),
        AgentState::Streaming => (-0.20, 8),
    };
    let ix = 9.0 * radius_scale;
    let iy = 1.8 * radius_scale;
    let inner_pts = ellipse_points(ix, iy, inner_count, t * inner_speed);
    for (i, p) in inner_pts.iter().enumerate() {
        let px = (cx as i32 + p.x as i32).max(0) as u16;
        let py = (cy as i32 + p.y as i32).max(0) as u16;
        let ch = ring_dot_char(phase, 1, i, t);
        set_cell_bounded(start_x, right, start_y, bottom, px, py, ch, dim, buf);
    }
}

// ── Compact version ────────────────────────────────────────────────────────

fn render_compact(
    start_x: u16,
    start_y: u16,
    buf: &mut Buffer,
    phase: &AnimationPhase,
    t: f64,
    accent: Style,
    dim: Style,
) {
    let cx = start_x + 10;
    let cy = start_y + 3;
    let right = start_x + 21;
    let bottom = start_y + 7;

    // ── Static base shape: single rounded ring ─────────────────────────
    // Ring: 18 wide × 5 tall (half-width 9, rows cy-2 .. cy+2)
    stroke_top_arc(cx, cy - 2, 9, dim, buf, start_x, right);
    stroke_bottom_arc(cx, cy + 2, 9, dim, buf, start_x, right);
    stroke_side(cx - 9, cy - 1, cy + 1, dim, buf, start_x, right, start_y, bottom);
    stroke_side(cx + 9, cy - 1, cy + 1, dim, buf, start_x, right, start_y, bottom);

    // ── Central Ω glyph ────────────────────────────────────────────────
    let center_mod = pulse_modifier(phase);
    let center_style = if let Some(m) = center_mod {
        accent.add_modifier(m)
    } else {
        accent
    };
    set_cell_bounded(start_x, right, start_y, bottom, cx, cy, 'Ω', center_style, buf);

    // ── Orbital dots traveling along the ring ──────────────────────────
    let (speed, dot_count, radius_scale) = match phase.agent {
        AgentState::Idle => (0.07, 4, 1.0),
        AgentState::Thinking => (0.18, 6, 0.6),
        AgentState::Streaming => (0.30, 8, 1.0),
    };

    let ox = 9.0 * radius_scale;
    let oy = 2.0 * radius_scale;

    let pts = ellipse_points(ox, oy, dot_count, t * speed);
    for (i, p) in pts.iter().enumerate() {
        let px = (cx as i32 + p.x as i32).max(0) as u16;
        let py = (cy as i32 + p.y as i32).max(0) as u16;
        let ch = ring_dot_char(phase, 0, i, t);
        let style = if phase.agent == AgentState::Thinking {
            accent
        } else {
            dim
        };
        set_cell_bounded(start_x, right, start_y, bottom, px, py, ch, style, buf);
    }
}
