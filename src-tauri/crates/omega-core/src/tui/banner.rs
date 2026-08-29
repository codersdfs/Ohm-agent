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

/// Compact fallback art: OMEGA alone in the same FIGlet style (~51 cols).
/// Shown when the terminal cannot fit the full OMEGA AGENT block; the word
/// AGENT moves into the subtitle line beneath.
pub const COMPACT_LINES: [&str; 6] = [
    " ██████╗  ███╗   ███╗ ███████╗  ██████╗   █████╗ ",
    "██╔═══██╗ ████╗ ████║ ██╔════╝ ██╔════╝  ██╔══██╗",
    "██║   ██║ ██╔████╔██║ █████╗   ██║  ███╗ ███████║",
    "██║   ██║ ██║╚██╔╝██║ ██╔══╝   ██║   ██║ ██╔══██║",
    "╚██████╔╝ ██║ ╚═╝ ██║ ███████╗ ╚██████╔╝ ██║  ██║",
    " ╚═════╝  ╚═╝     ╚═╝ ╚══════╝  ╚═════╝  ╚═╝  ╚═╝",
];

/// Height of the art block itself.
pub const BANNER_H: u16 = 6;
/// Total splash block height: art + gap + subtitle + gap + hint.
pub const SPLASH_H: u16 = BANNER_H + 4;

/// Display width of an art set: max row length in terminal columns.
pub fn art_width(lines: &[&str]) -> u16 {
    lines
        .iter()
        .map(|l| l.chars().count() as u16)
        .max()
        .unwrap_or(0)
}

/// Full OMEGA AGENT block needs this many columns.
pub const FULL_MIN_W: u16 = 98;
/// Compact OMEGA-only block needs this many columns.
pub const COMPACT_MIN_W: u16 = 55;

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
/// `error` (e.g. a missing-API-key warning) is drawn dim-red beneath the hint
/// so startup warnings remain visible instead of blocking the banner.
///
/// Falls back to silence (renders nothing) when the terminal cannot fit the
/// art — mirroring the responsive tiers of the flat header.
pub fn render_splash(
    area: Rect,
    buf: &mut Buffer,
    subtitle: &str,
    hint: &str,
    error: Option<&str>,
) {
    // Tiered art selection: full block on wide terminals, compact OMEGA
    // fallback on medium ones, silence below that.
    let (art, art_w) = if area.width >= FULL_MIN_W {
        (&LINES[..], art_width(&LINES))
    } else if area.width >= COMPACT_MIN_W {
        (&COMPACT_LINES[..], art_width(&COMPACT_LINES))
    } else {
        return;
    };
    let needed = SPLASH_H + if error.is_some() { 2 } else { 0 };
    if area.height < needed + 1 {
        return;
    }

    // Block anchored: art centered, meta lines centered under it.
    let top_y = area.y + (area.height - needed) / 2;

    let x = area.x + (area.width - art_w) / 2;
    for (r, line) in art.iter().enumerate() {
        draw_line(buf, x, top_y + r as u16, line, row_style(r));
    }

    let sub_y = top_y + BANNER_H + 1;
    draw_centered(buf, area, sub_y, subtitle, theme::style_dim());

    let hint_y = sub_y + 2;
    draw_centered(buf, area, hint_y, hint, theme::style_dim());

    if let Some(err) = error {
        let err_style = Style::default().fg(theme::WARN);
        let err_y = hint_y + 2;
        draw_centered(buf, area, err_y, err, err_style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_banner_rows_share_width() {
        for set in [&LINES[..], &COMPACT_LINES[..]] {
            let widths: Vec<u16> = set.iter().map(|l| l.chars().count() as u16).collect();
            assert!(
                widths.windows(2).all(|w| w[0] == w[1]),
                "widths: {:?}",
                widths
            );
            assert_eq!(art_width(set), widths[0]);
        }
    }

    #[test]
    fn test_compact_fits_declared_minimum() {
        assert!(art_width(&COMPACT_LINES) + 2 <= COMPACT_MIN_W);
        assert!(art_width(&LINES) + 2 <= FULL_MIN_W);
    }

    #[test]
    fn test_banner_respects_max_line_length() {
        // Gate structural limit for any source line.
        for line in LINES.iter().chain(COMPACT_LINES.iter()) {
            assert!(line.chars().count() <= 120);
        }
    }

    #[test]
    fn test_full_art_in_wide_area() {
        let area = Rect::new(0, 0, 140, 24);
        let mut buf = Buffer::empty(area);
        render_splash(area, &mut buf, "openai / gpt-test", "type a message", None);
        // Art occupies its first row band: expect at least one glyph.
        let band_has_glyph = (0..area.width).any(|x| buf.get(x, 8).symbol() != " ");
        assert!(band_has_glyph);
    }

    #[test]
    fn test_medium_area_gets_compact_art() {
        let area = Rect::new(0, 0, 70, 24);
        let mut buf = Buffer::empty(area);
        render_splash(area, &mut buf, "sub", "hint", None);
        let any_glyph = buf.content.iter().any(|cell| cell.symbol() != " ");
        assert!(any_glyph, "compact art should draw at 70 cols");
    }

    #[test]
    fn test_tiny_terminal_skips_splash() {
        let area = Rect::new(0, 0, 40, 20);
        let mut buf = Buffer::empty(area);
        render_splash(area, &mut buf, "sub", "hint", None);
        let any_glyph = buf.content.iter().any(|cell| cell.symbol() != " ");
        assert!(!any_glyph);
    }

    #[test]
    fn test_short_terminal_skips_splash() {
        let area = Rect::new(0, 0, 140, 8);
        let mut buf = Buffer::empty(area);
        render_splash(area, &mut buf, "sub", "hint", None);
        let any_glyph = buf.content.iter().any(|cell| cell.symbol() != " ");
        assert!(!any_glyph, "needs {} rows", SPLASH_H + 1);
    }

    #[test]
    fn test_error_notice_drawn_under_hint() {
        let area = Rect::new(0, 0, 140, 26);
        let mut buf = Buffer::empty(area);
        render_splash(area, &mut buf, "sub", "hint", Some("no api key"));
        // With error: needed=12 -> top_y=(26-12)/2=7, sub_y=14, hint_y=16, err_y=18.
        let err_band_has_glyph = (0..area.width).any(|x| buf.get(x, 18).symbol() != " ");
        assert!(err_band_has_glyph);
    }
}
