use crate::tui::theme;
use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Widget};

/// State for the permission dialog
#[derive(Clone)]
pub struct PermissionPanelState {
    /// Whether the panel is visible
    pub visible: bool,
    /// Current prompt message
    pub prompt: String,
    /// Available options (e.g., ["Allow", "Deny"])
    pub options: Vec<String>,
    /// Currently selected option index
    pub selected_idx: usize,
    /// The request ID to respond to
    pub request_id: String,
}

impl Default for PermissionPanelState {
    fn default() -> Self {
        Self {
            visible: false,
            prompt: String::new(),
            options: Vec::new(),
            selected_idx: 0,
            request_id: String::new(),
        }
    }
}

impl PermissionPanelState {
    pub fn show(&mut self, request_id: String, prompt: String, options: Vec<String>) {
        self.visible = true;
        self.request_id = request_id;
        self.prompt = prompt;
        self.options = options;
        self.selected_idx = 0;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.selected_idx = 0;
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        if self.selected_idx > 0 {
            self.selected_idx -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        if self.selected_idx < self.options.len().saturating_sub(1) {
            self.selected_idx += 1;
        }
    }

    /// Get the currently selected option index
    pub fn selected(&self) -> usize {
        self.selected_idx
    }
}

/// Render the permission panel as a collapsible bottom section.
/// When visible, it takes up the bottom portion of the area.
pub fn render_permission_panel(area: Rect, buf: &mut Buffer, state: &PermissionPanelState) {
    if !state.visible {
        return;
    }

    // Bottom panel takes 4 lines:
    // - 1: top border with title
    // - 2: prompt message
    // - 3: options row
    // - 4: bottom border
    let panel_height = 4u16;

    if area.height < panel_height + 1 {
        // Not enough space, skip rendering
        return;
    }

    let panel_area = Rect {
        x: area.x,
        y: area.y + area.height - panel_height,
        width: area.width,
        height: panel_height,
    };

    // Fill panel background (use terminal background - Color::Reset = inherit)
    for y in panel_area.y..panel_area.y + panel_height {
        for x in panel_area.x..panel_area.x + panel_area.width {
            if x < area.width && y < panel_height {
                buf.get_mut(area.x + x, area.y + y).set_bg(Color::Reset);
            }
        }
    }

    // Top border - straight line with title
    let title = " Permission Request ".to_string();
    let border_style = Style::default().fg(theme::OUTLINE);
    let title_style = Style::default()
        .fg(theme::PRIMARY)
        .add_modifier(Modifier::BOLD);

    // Draw top border
    let mut top_spans = vec![Span::styled("─".repeat(2), border_style)];
    top_spans.push(Span::styled(title.as_str(), title_style));
    let remaining = panel_area.width.saturating_sub(2 + title.len() as u16 + 2);
    if remaining > 0 {
        top_spans.push(Span::styled("─".repeat(remaining as usize), border_style));
    }
    top_spans.push(Span::styled("─".repeat(2), border_style));

    let top_line = Line::from(top_spans);
    let top_para = Paragraph::new(top_line);
    top_para.render(
        Rect {
            x: panel_area.x,
            y: panel_area.y,
            width: panel_area.width,
            height: 1,
        },
        buf,
    );

    // Prompt line (indented)
    let prompt_style = Style::default().fg(theme::FG);
    let prompt_spans = vec![Span::raw("  "), Span::styled(&state.prompt, prompt_style)];
    let prompt_line = Line::from(prompt_spans);
    let prompt_para = Paragraph::new(prompt_line);
    prompt_para.render(
        Rect {
            x: panel_area.x,
            y: panel_area.y + 1,
            width: panel_area.width,
            height: 1,
        },
        buf,
    );

    // Options row with selection highlight
    let mut option_spans = vec![Span::raw("  ")];
    for (i, opt) in state.options.iter().enumerate() {
        if i > 0 {
            option_spans.push(Span::raw("   "));
        }

        let opt_style = if i == state.selected_idx {
            Style::default()
                .fg(theme::PERMISSION_HIGHLIGHT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::DIM)
        };

        option_spans.push(Span::styled(format!("{}: {}", i + 1, opt), opt_style));
    }

    // Add navigation hint
    option_spans.push(Span::raw("  |  "));
    option_spans.push(Span::styled(
        "↑↓ navigate  Enter confirm  Esc cancel",
        Style::default().fg(theme::DIM),
    ));

    let options_line = Line::from(option_spans);
    let options_para = Paragraph::new(options_line);
    options_para.render(
        Rect {
            x: panel_area.x,
            y: panel_area.y + 2,
            width: panel_area.width,
            height: 1,
        },
        buf,
    );

    // Bottom border
    let bottom_spans = vec![Span::styled(
        "─".repeat(panel_area.width as usize),
        border_style,
    )];
    let bottom_line = Line::from(bottom_spans);
    let bottom_para = Paragraph::new(bottom_line);
    bottom_para.render(
        Rect {
            x: panel_area.x,
            y: panel_area.y + 3,
            width: panel_area.width,
            height: 1,
        },
        buf,
    );
}
