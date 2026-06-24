use chrono::{DateTime, Utc};
use ratatui::style::{Color, Style};

#[derive(Debug, Clone, PartialEq)]
pub enum MessageSender {
    User,
    Assistant,
    System,
    Tool,
}

impl std::fmt::Display for MessageSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "You"),
            Self::Assistant => write!(f, "Assistant"),
            Self::System => write!(f, "System"),
            Self::Tool => write!(f, "Tool"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum MessageStatus {
    Complete,
    Streaming,
    Error,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Message {
    pub sender: MessageSender,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub status: MessageStatus,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            sender: MessageSender::User,
            content: content.into(),
            timestamp: Utc::now(),
            status: MessageStatus::Complete,
        }
    }

    pub fn assistant(content: impl Into<String>, streaming: bool) -> Self {
        Self {
            sender: MessageSender::Assistant,
            content: content.into(),
            timestamp: Utc::now(),
            status: if streaming {
                MessageStatus::Streaming
            } else {
                MessageStatus::Complete
            },
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            sender: MessageSender::System,
            content: content.into(),
            timestamp: Utc::now(),
            status: MessageStatus::Complete,
        }
    }

    pub fn tool(content: impl Into<String>) -> Self {
        Self {
            sender: MessageSender::Tool,
            content: content.into(),
            timestamp: Utc::now(),
            status: MessageStatus::Complete,
        }
    }

    #[allow(dead_code)]
    pub fn sender_style(&self) -> Style {
        match self.sender {
            MessageSender::User => Style::default().fg(Color::Rgb(99, 102, 241)),
            MessageSender::Assistant => Style::default().fg(Color::Rgb(52, 211, 153)),
            MessageSender::System => Style::default().fg(Color::Rgb(115, 115, 128)),
            MessageSender::Tool => Style::default().fg(Color::Rgb(251, 191, 36)),
        }
    }

    #[allow(dead_code)]
    pub fn content_style(&self) -> Style {
        Style::default().fg(Color::Rgb(226, 232, 240))
    }

    pub fn status_cursor(&self) -> &'static str {
        match self.status {
            MessageStatus::Streaming => " █",
            _ => "",
        }
    }
}

#[derive(Debug)]
pub struct MessageHistory {
    messages: Vec<Message>,
    pub scroll_offset: usize,
    pub follow: bool,
}

impl Default for MessageHistory {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            follow: true,
        }
    }
}

impl MessageHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
        if self.follow {
            self.scroll_offset = 0;
        }
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.scroll_offset = 0;
        self.follow = true;
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Message> {
        self.messages.iter()
    }

    pub fn last(&self) -> Option<&Message> {
        self.messages.last()
    }

    pub fn update_last(&mut self, content: &str) {
        if let Some(msg) = self.messages.last_mut() {
            if msg.sender == MessageSender::Assistant && msg.status == MessageStatus::Streaming {
                msg.content = content.to_string();
            }
        }
    }

    pub fn finalize_last(&mut self) {
        if let Some(msg) = self.messages.last_mut() {
            if msg.sender == MessageSender::Assistant && msg.status == MessageStatus::Streaming {
                msg.status = MessageStatus::Complete;
            }
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
        self.follow = false;
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        if self.scroll_offset == 0 {
            self.follow = true;
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.follow = true;
    }

    #[allow(dead_code)]
    pub fn is_at_bottom(&self) -> bool {
        self.scroll_offset == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_creation() {
        let m = Message::user("hello");
        assert_eq!(m.sender, MessageSender::User);
        assert_eq!(m.content, "hello");
        assert_eq!(m.status, MessageStatus::Complete);
    }

    #[test]
    fn message_history_push_and_scroll() {
        let mut h = MessageHistory::new();
        h.push(Message::system("welcome"));
        h.push(Message::user("hi"));
        assert_eq!(h.len(), 2);
        assert!(h.is_at_bottom());

        h.scroll_up(5);
        assert_eq!(h.scroll_offset, 5);
        assert!(!h.is_at_bottom());

        h.scroll_down(3);
        assert_eq!(h.scroll_offset, 2);

        h.scroll_to_bottom();
        assert!(h.is_at_bottom());
    }

    #[test]
    fn message_history_clear() {
        let mut h = MessageHistory::new();
        h.push(Message::user("a"));
        h.push(Message::assistant("b", false));
        h.clear();
        assert!(h.is_empty());
        assert!(h.is_at_bottom());
    }

    #[test]
    fn update_last_message() {
        let mut h = MessageHistory::new();
        h.push(Message::assistant("part", true));
        h.update_last("part ial");
        assert_eq!(h.last().unwrap().content, "part ial");

        h.finalize_last();
        assert_eq!(h.last().unwrap().status, MessageStatus::Complete);
    }
}
