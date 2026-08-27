//! OMEGA AGENT ASCII splash banner.
//!
//! Rendered inside the transcript area while the session is empty. The art is
//! kept verbatim from the brand figlet; styling fades vertically so the block
//! stays quiet against the dark canvas (no mid-line color splits — the figlet
//! glyphs abut across word boundaries).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::theme;

/// Banner art, one entry per row. All rows are the same display width.
pub const LINES: [&str; 6] = [
    " ██████╗ ███╗   ███╗███████╗ ██████╗  █████╗        █████╗  ██████╗ ███████╗███╗   ██╗████████╗",
    "██╔═══██╗████╗ ████║██╔════╝██╔════╝ ██╔══██╗      ██╔══██╗██╔════╝ ██╔════╝████╗  ██║╚══██╔══╝",
    "██║   ██║██╔████╔██║█████╗  ██║  ███╗███████║█████╗███████║██║  ███╗█████╗  ██╔██╗ ██║   ██║   ",
    "██║   ██║██║╚██╔╝██║██╔══╝  ██║   ██║██╔══██║╚════╝██╔══██║██║   ██║██╔══╝  ██║╚██╗██║   ██║   ",
    "╚██████╔╝██║ ╚═╝ ██║███████╗╚██████╔╝██║  ██║      ██║  ██║╚██████╔╝███████╗██║ ╚████║   ██║   ",
    " ╚═════╝ ╚═╝     ╚═╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝      ╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═══╝   ╚═╝   ",
];

/// Height of the art block itself.
pub const BANNER_H: u16 = 6;
/// Total splash block height: art + gap + subtitle + gap + hint.
pub const SPLASH_H: u16 = BANNER_H + 4;

/// Widest banner row in terminal columns (all rows share the same width).
pub fn banner_width() -> u16 {
    LINES
        .iter()
        .map(|l| l.chars().count() as u16)
        .max()
        .unwrap_or(0)
}

/// Vertical fade styles, one per banner row: bright crown → quiet base.
fn row_style(row: usize) -> Style {
    match row {
        0 | 1 => Style::default()
            .fg(theme::PRIMARY_CONTAINER)
            .add_modifier(Modifier::BOLD),
        2 | 3 => Style::default().fg(theme::PRIMARY),
        _ => Style::default().fg(theme::OUTLINE_VARIANT),
    }
}

/// Draw one string starting at `(x, y)` with the given style, clipping to the
/// buffer area. Column-preserving — never realigns individual rows.
fn draw_line(buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style) {
    for (i, ch) in text.chars().enumerate() {
        if ch == ' ' {
            continue; // transparent — the background shows through
        }
        if let Some(cell) = theme::buf_cell_mut(buf, x.saturating_add(i as u16), y) {
            cell.set_char(ch);
            cell.set_style(style);
        }
    }
}

/// Horizontally center `text` within `area` on row `y`.
fn draw_centered(buf: &mut Buffer, area: Rect, y: u16, text: &str, style: Style) {
    let tw = text.chars().count() as u16;
    if tw > area.width {
        return;
    }
    let x = area.x + (area.width - tw) / 2;
    draw_line(buf, x, y, text, style);
}

/// Render the startup splash: the OMEGA AGENT banner with a provider/model
/// subtitle beneath it and an input hint, all centered as one block.
///
/// Falls back to silence (renders nothing) when the terminal cannot fit the
/// art — mirroring the responsive tiers of the flat header.
pub fn render_splash(area: Rect, buf: &mut Buffer, subtitle: &str, hint: &str) {
    let art_w = banner_width();
    if area.width < art_w || area.height < SPLASH_H + 1 {
        return;
    }

    // Block anchored: art centered, meta lines centered under it.
    let top_y = area.y + (area.height - SPLASH_H) / 2;

    for (r, line) in LINES.iter().enumerate() {
        draw_line(buf, area.x, top_y + r as u16, line, row_style(r));
    }

    let sub_y = top_y + BANNER_H + 1;
    draw_centered(buf, area, sub_y, subtitle, theme::style_dim());

    let hint_y = sub_y + 2;
    draw_centered(buf, area, hint_y, hint, theme::style_dim());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_banner_rows_share_width() {
        let widths: Vec<u16> = LINES.iter().map(|l| l.chars().count() as u16).collect();
        assert!(
            widths.windows(2).all(|w| w[0] == w[1]),
            "widths: {:?}",
            widths
        );
        assert_eq!(banner_width(), widths[0]);
    }

    #[test]
    fn test_banner_respects_max_line_length() {
        // Gate structural limit for any source line.
        for line in LINES.iter() {
            assert!(line.chars().count() <= 120);
        }
    }

    #[test]
    fn test_banner_render_places_art_in_large_area() {
        let area = Rect::new(0, 0, 140, 24);
        let mut buf = Buffer::empty(area);
        render_splash(area, &mut buf, "openai / gpt-test", "type a message");
        // Top-left of the first visible glyph lands inside the buffer.
        assert!(theme::buf_cell_mut(&mut buf, 1, 8).is_some());
        // Art occupies its first row somewhere: expect at least one non-space
        // glyph written at the computed top row band.
        let band_has_glyph = (0..area.width).any(|x| buf.get(x, 8).symbol() != " ");
        assert!(band_has_glyph);
    }

    #[test]
    fn test_banner_skips_narrow_terminal() {
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        render_splash(area, &mut buf, "sub", "hint");
        // Below the art width nothing should be drawn.
        let any_glyph = buf.content.iter().any(|cell| cell.symbol() != " ");
        assert!(!any_glyph);
    }
}
