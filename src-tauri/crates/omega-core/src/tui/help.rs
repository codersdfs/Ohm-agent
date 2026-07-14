use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

use super::theme;

/// Render a help overlay centered in the given area.
pub fn render(area: Rect, buf: &mut Buffer) {
    if area.width < 50 || area.height < 14 {
        return;
    }

    // Calculate centered popup area
    let popup_width = area.width.min(56);
    let popup_height = 14u16;
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Dim the background behind the popup
    for cy in area.y..area.y + area.height {
        for cx in area.x..area.x + area.width {
            if let Some(cell) = theme::buf_cell_mut(buf, cx, cy) {
                cell.set_style(Style::default().fg(theme::DIM));
            }
        }
    }

    let lines = vec![
        Line::from(vec![
            Span::styled(" Omega Agent ", Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled("v".to_string() + env!("CARGO_PKG_VERSION"), Style::default().fg(theme::DIM)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Keybindings", Style::default().fg(theme::FG).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Enter          ", theme::style_dim()),
            Span::styled("Send message", Style::default().fg(theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("  Shift+Enter    ", theme::style_dim()),
            Span::styled("New line in editor", Style::default().fg(theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("  Tab            ", theme::style_dim()),
            Span::styled("Cycle command suggestions", Style::default().fg(theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("  ↑/↓            ", theme::style_dim()),
            Span::styled("Scroll transcript / recall history", Style::default().fg(theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("  PgUp/PgDn      ", theme::style_dim()),
            Span::styled("Scroll faster", Style::default().fg(theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+C         ", theme::style_dim()),
            Span::styled("Cancel streaming / quit", Style::default().fg(theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+Q         ", theme::style_dim()),
            Span::styled("Quit Omega", Style::default().fg(theme::FG)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Commands", Style::default().fg(theme::FG).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  /help, /?, /h  ", theme::style_dim()),
            Span::styled("Show this help", Style::default().fg(theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("  /clear, /cls   ", theme::style_dim()),
            Span::styled("Clear conversation", Style::default().fg(theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("  /tools         ", theme::style_dim()),
            Span::styled("List available tools", Style::default().fg(theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("  /cost           ", theme::style_dim()),
            Span::styled("Show session token usage", Style::default().fg(theme::FG)),
        ]),
        Line::from(vec![
            Span::styled("  /exit, /quit   ", theme::style_dim()),
            Span::styled("Exit Omega", Style::default().fg(theme::FG)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Press any key to close ", Style::default().fg(theme::DIM).add_modifier(Modifier::BOLD)),
        ]),
    ];

    let text = Text::from(lines);
    let para = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme::ACCENT))
                .style(Style::default().bg(theme::BG)),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false });

    para.render(popup_area, buf);
}
