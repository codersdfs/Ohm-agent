use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Style, Modifier};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget};

use super::theme;

/// Editor UI state — the text being edited and input mode.
#[derive(Clone)]
pub struct EditorState {
    pub buffer: String,
    pub cursor: usize,       // Byte offset into buffer
    pub state: EditorMode,
    /// Slash-command suggestions (popup above editor)
    pub suggestions: Vec<String>,
    pub selected_suggestion: usize,
}

/// What the editor is currently doing — determines border color.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Idle,
    Thinking,
    Streaming,
    Error,
    Confirm,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            state: EditorMode::Idle,
            suggestions: Vec::new(),
            selected_suggestion: 0,
        }
    }

    pub fn border_color(&self) -> ratatui::style::Color {
        match self.state {
            EditorMode::Idle => theme::EDITOR_IDLE,
            EditorMode::Thinking => theme::EDITOR_THINKING,
            EditorMode::Streaming => theme::EDITOR_STREAMING,
            EditorMode::Error => theme::EDITOR_ERROR,
            EditorMode::Confirm => theme::EDITOR_CONFIRM,
        }
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        if self.cursor <= self.buffer.len() {
            self.buffer.insert(self.cursor, c);
            self.cursor += c.len_utf8();
        }
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = self.buffer[..self.cursor].char_indices().last();
            if let Some((idx, _)) = prev {
                self.buffer.drain(idx..self.cursor);
                self.cursor = idx;
            }
        }
    }

    /// Delete the character at the cursor.
    pub fn delete(&mut self) {
        if self.cursor < self.buffer.len() {
            let next = self.buffer[self.cursor..].char_indices().nth(1);
            let end = next.map(|(i, _)| self.cursor + i).unwrap_or(self.buffer.len());
            self.buffer.drain(self.cursor..end);
        }
    }

    /// Move cursor left by one character.
    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            let prev = self.buffer[..self.cursor].char_indices().last();
            if let Some((idx, _)) = prev {
                self.cursor = idx;
            }
        }
    }

    /// Move cursor right by one character.
    pub fn cursor_right(&mut self) {
        if self.cursor < self.buffer.len() {
            let next = self.buffer[self.cursor..].char_indices().nth(1);
            if let Some((i, _)) = next {
                self.cursor += i;
            } else {
                self.cursor = self.buffer.len();
            }
        }
    }

    /// Move cursor to start of line.
    pub fn cursor_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end of line.
    pub fn cursor_end(&mut self) {
        self.cursor = self.buffer.len();
    }

    /// Insert newline at cursor.
    pub fn newline(&mut self) {
        self.buffer.insert(self.cursor, '\n');
        self.cursor += 1;
    }

    /// Take the current buffer for sending.
    pub fn take_buffer(&mut self) -> String {
        let content = self.buffer.clone();
        self.buffer.clear();
        self.cursor = 0;
        content
    }
}

impl Widget for &EditorState {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 || area.width < 4 {
            return;
        }

        let border_color = self.border_color();
        let border_style = Style::default().fg(border_color);

        let label = match self.state {
            EditorMode::Idle => " type a message… ",
            EditorMode::Thinking => " Cooking… ",
            EditorMode::Streaming => " Writing… ",
            EditorMode::Error => " error ",
            EditorMode::Confirm => " confirm? ",
        };

        let block = Block::default()
            .borders(Borders::TOP)
            .border_type(BorderType::Plain)
            .border_style(border_style)
            .title(Line::from(Span::styled(label, Style::default().fg(border_color).add_modifier(Modifier::DIM))))
            .style(Style::default().bg(theme::RECESSED));

        let text = if self.buffer.is_empty() {
            Text::from(Line::from(Span::styled("", theme::style_dim())))
        } else {
            let lines: Vec<Line> = self
                .buffer
                .split('\n')
                .map(|l| Line::from(Span::styled(l.to_string(), Style::default().fg(theme::FG))))
                .collect();
            Text::from(lines)
        };

        let para = Paragraph::new(text)
            .block(block);

        para.render(area, buf);
    }
}

/// Render slash-command suggestion popup above the editor.
pub fn render_suggestions(
    area: Rect,
    buf: &mut Buffer,
    suggestions: &[String],
    selected: usize,
) {
    if suggestions.is_empty() || area.height < 1 {
        return;
    }

    let max_height = suggestions.len().min(5) as u16;
    let popup_height = max_height + 2; // border
    let popup_y = area.y.saturating_sub(popup_height);

    let popup_area = Rect::new(
        area.x,
        popup_y,
        area.width.min(40),
        popup_height,
    );

    let mut lines = Vec::new();
    for (i, s) in suggestions.iter().enumerate().take(max_height as usize) {
        let style = if i == selected {
            Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::DIM)
        };
        lines.push(Line::from(Span::styled(s.clone(), style)));
    }

    let popup = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(Style::default().fg(theme::DIM))
                .title(" commands ")
                .title_style(Style::default().fg(theme::DIM)),
        )
        .style(Style::default().bg(theme::BG));

    popup.render(popup_area, buf);
}

use crate::tui::component::{Action, Component};
use crossterm::event::{KeyCode, KeyEventKind};

impl Component for EditorState {
    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Action {
        if key.kind != KeyEventKind::Press {
            return Action::Noop;
        }
        match key.code {
            KeyCode::Enter => {
                if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                    self.newline();
                    Action::Noop
                } else {
                    Action::SendMessage
                }
            }
            KeyCode::Char(c) => {
                self.insert_char(c);
                Action::Noop
            }
            KeyCode::Backspace => {
                self.backspace();
                Action::Noop
            }
            KeyCode::Delete => {
                self.delete();
                Action::Noop
            }
            KeyCode::Left => {
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    self.cursor_home();
                } else {
                    self.cursor_left();
                }
                Action::Noop
            }
            KeyCode::Right => {
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    self.cursor_end();
                } else {
                    self.cursor_right();
                }
                Action::Noop
            }
            KeyCode::Home => {
                self.cursor_home();
                Action::Noop
            }
            KeyCode::End => {
                self.cursor_end();
                Action::Noop
            }
            KeyCode::Tab => {
                if !self.suggestions.is_empty() {
                    self.selected_suggestion = (self.selected_suggestion + 1) % self.suggestions.len();
                }
                Action::Noop
            }
            _ => Action::Noop,
        }
    }

    fn render(&mut self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        f.render_widget(&*self, area);
    }
}
