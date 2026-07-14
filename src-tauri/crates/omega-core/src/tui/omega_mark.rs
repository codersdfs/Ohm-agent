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
    let speed = match phase.agent {
        AgentState::Idle => 0.025,
        AgentState::Thinking => 0.08,
        AgentState::Streaming => 0.12 + (t.sin() * 0.05).abs(),
    };
    let dot_set = if phase.agent == AgentState::Streaming {
        // Scrambled dots during streaming
        let scramble = ((t * 3.0).floor() as usize) % 6;
        match scramble {
            0 => ['⠿', '⠾', '⠽', '⠼', '⠻', '⠺'],
            1 => ['⠛', '⠟', '⠿', '⠾', '⠽', '⠼'],
            2 => ['⠉', '⠃', '⠋', '⠛', '⠟', '⠿'],
            _ => ['⠿', '⠾', '⠽', '⠼', '⠻', '⠺'],
        }
    } else {
        ['⠿', '⠾', '⠽', '⠼', '⠻', '⠺']
    };

    let dots: Vec<(i16, i16, char)> = (0..6)
        .map(|i| {
            let angle = t * speed + (i as f64 * std::f64::consts::PI / 3.0);
            let dx = (angle.cos() * 10.0).round() as i16;
            let dy = (angle.sin() * 4.0).round() as i16;
            let idx = i.min(5);
            (dx, dy, dot_set[idx])
        })
        .collect();

    // Slow pulse on idle, faster on thinking, flicker on streaming
    let modifier = match phase.agent {
        AgentState::Idle => {
            // Subtle pulse: alternate bold every 8 ticks
            if (phase.tick / 8) % 2 == 0 {
                Some(Modifier::BOLD)
            } else {
                None
            }
        }
        AgentState::Thinking => {
            // Faster pulse
            if (phase.tick / 4) % 2 == 0 {
                Some(Modifier::BOLD)
            } else {
                None
            }
        }
        AgentState::Streaming => {
            // Rapid flicker
            if (phase.tick / 2) % 2 == 0 {
                Some(Modifier::BOLD)
            } else {
                None
            }
        }
    };

    (glyph, dots, modifier)
}

// ── Compact version ────────────────────────────────────────────────────────

/// Build the compact Omega glyph (≈9×5) with a two-frame dither.
fn compact_omega(phase: &AnimationPhase) -> (Vec<&'static str>, Vec<(i16, i16, char)>, Option<Modifier>) {
    let frame = (phase.tick / 6) % 2; // slow 2-frame dither

    let glyph = if frame == 0 {
        vec![
            "  █████ ",
            " ██▒▒▒██",
            " ██ Ω ██",
            " ██▒▒▒██",
            "  █████ ",
        ]
    } else {
        vec![
            "  ░░░░░ ",
            " ░░███░░",
            " ░░ Ω ░░",
            " ░░███░░",
            "  ░░░░░ ",
        ]
    };

    let accent_mod = match phase.agent {
        AgentState::Idle => {
            if (phase.tick / 8) % 2 == 0 {
                Some(Modifier::BOLD)
            } else {
                None
            }
        }
        AgentState::Thinking => Some(Modifier::BOLD),
        AgentState::Streaming => None, // streaming uses flicker below
    };

    // Two smaller orbiting dots for compact mode
    let t = phase.tick as f64;
    let speed = match phase.agent {
        AgentState::Idle => 0.03,
        AgentState::Thinking => 0.08,
        AgentState::Streaming => 0.15,
    };
    let dots = vec![
        (
            (t.cos() * speed * 6.0).round() as i16,
            (t.sin() * speed * 3.0).round() as i16,
            if (phase.tick / 2) % 2 == 0 { '⠿' } else { '⠾' },
        ),
        (
            ((t + std::f64::consts::PI).cos() * speed * 6.0).round() as i16,
            ((t + std::f64::consts::PI).sin() * speed * 3.0).round() as i16,
            if (phase.tick / 3) % 2 == 0 { '⠛' } else { '⠟' },
        ),
    ];

    (glyph, dots, accent_mod)
}
