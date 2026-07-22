use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap};

use super::theme;

/// Render a help overlay centered in the given area.
pub fn render(area: Rect, buf: &mut Buffer) {
    if area.width < 40 || area.height < 10 {
        return;
    }

    let popup_width = area.width.min(40);
    let popup_height = 10u16;
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Dim background
    for cy in area.y..area.y + area.height {
        for cx in area.x..area.x + area.width {
            if let Some(cell) = theme::buf_cell_mut(buf, cx, cy) {
                cell.set_style(Style::default().fg(theme::DIM));
            }
        }
    }

    let lines = vec![
        Line::from(vec![
            Span::styled(" OMEGA_AGENT ", Style::default().fg(theme::PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled("v".to_string() + env!("CARGO_PKG_VERSION"), theme::style_dim()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [F1] HELP  [F2] LOGS  [F3] NET  [F10] EXIT", theme::style_dim()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Enter       ", theme::style_dim()),
            Span::styled("Send message", Style::default().fg(theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("  Shift+Enter ", theme::style_dim()),
            Span::styled("New line", Style::default().fg(theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+C      ", theme::style_dim()),
            Span::styled("Cancel / quit", Style::default().fg(theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+Q      ", theme::style_dim()),
            Span::styled("Quit Omega", Style::default().fg(theme::FG)),
        ]),
    ];

    let text = Text::from(lines);
    let para = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(theme::PRIMARY))
                .style(Style::default().bg(theme::SURFACE_HIGH)),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    para.render(popup_area, buf);
}