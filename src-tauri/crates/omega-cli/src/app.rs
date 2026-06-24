use crate::backend::{ChatBackend, MockBackend, ProviderBackend};
use crate::commands;
use crate::event::{start_poller, AppEvent};
use crate::message::{Message, MessageHistory};
use crate::ui::draw;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mcp::skills::SkillsRegistry;
use mcp::transport::JsonRpcTransport;
use providers::ProviderConfig;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::Stdout;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Insert,
}

pub struct App {
    pub input_lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub mode: InputMode,
    pub history: MessageHistory,
    pub is_loading: bool,
    pub loading_tick: u8,
    pub should_quit: bool,
    pub cycle_idx: usize,
    pub provider_config: Option<ProviderConfig>,
    pub mcp_registry: SkillsRegistry,
    pub mcp_transport: Option<Arc<JsonRpcTransport>>,
    pub project_root: PathBuf,
    backend: Arc<dyn ChatBackend>,
    backend_rx: UnboundedReceiver<String>,
    backend_tx: tokio::sync::mpsc::UnboundedSender<String>,
}

impl App {
    pub fn new() -> Self {
        let (backend_tx, backend_rx) = tokio::sync::mpsc::unbounded_channel();

        let mut history = MessageHistory::new();
        history.push(Message::system(
            "Welcome to omega-cli. Press i to start typing. /help for commands.",
        ));

        let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        Self {
            input_lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            mode: InputMode::Normal,
            history,
            is_loading: false,
            loading_tick: 0,
            should_quit: false,
            cycle_idx: 0,
            provider_config: None,
            mcp_registry: SkillsRegistry::new(),
            mcp_transport: None,
            project_root,
            backend: Arc::new(MockBackend::default()),
            backend_rx,
            backend_tx,
        }
    }

    pub fn with_backend(backend: Arc<dyn ChatBackend>, project_root: PathBuf) -> Self {
        let (backend_tx, backend_rx) = tokio::sync::mpsc::unbounded_channel();

        let mut history = MessageHistory::new();
        history.push(Message::system(
            "Welcome to omega-cli. Press i to start typing. /help for commands.",
        ));

        Self {
            input_lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            mode: InputMode::Normal,
            history,
            is_loading: false,
            loading_tick: 0,
            should_quit: false,
            cycle_idx: 0,
            provider_config: None,
            mcp_registry: SkillsRegistry::new(),
            mcp_transport: None,
            project_root,
            backend,
            backend_rx,
            backend_tx,
        }
    }

    pub fn switch_provider(&mut self, config: ProviderConfig) {
        let provider = providers::create_provider(&config);
        match provider {
            Ok(p) => {
                let backend = ProviderBackend::new(Arc::from(p), config.clone());
                self.backend = Arc::new(backend);
                self.provider_config = Some(config);
            }
            Err(e) => {
                self.history
                    .push(Message::system(format!("Provider error: {e}")));
            }
        }
    }

    pub fn switch_to_mock(&mut self) {
        self.backend = Arc::new(MockBackend::default());
        self.provider_config = None;
    }

    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) {
        let mut poller = start_poller();

        loop {
            terminal
                .draw(|f| draw(f, self))
                .expect("failed to draw");

            if self.should_quit {
                break;
            }

            tokio::select! {
                Some(AppEvent::Key(key)) = poller.recv() => {
                    self.handle_key(key);
                }
                chunk = self.backend_rx.recv() => {
                    match chunk {
                        Some(c) if c.is_empty() => self.handle_done(Ok(())),
                        Some(c) if c.starts_with("ERROR: ") => {
                            self.handle_done(Err(c[7..].to_string()));
                        }
                        Some(c) => self.handle_chunk(c),
                        None => {}
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                    self.tick();
                }
            }
        }
    }

    fn tick(&mut self) {
        if self.is_loading {
            self.loading_tick = (self.loading_tick + 1) % 10;
        }
    }

    fn handle_chunk(&mut self, chunk: String) {
        if let Some(msg) = self.history.last() {
            if msg.sender == crate::message::MessageSender::Assistant
                && msg.status == crate::message::MessageStatus::Streaming
            {
                let new_content = format!("{}{chunk}", msg.content);
                self.history.update_last(&new_content);
            }
        }
    }

    fn handle_done(&mut self, result: Result<(), String>) {
        self.is_loading = false;
        self.history.finalize_last();
        if let Err(e) = result {
            self.history.push(Message::system(format!("Error: {e}")));
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') | KeyCode::Char('q') => {
                    self.should_quit = true;
                    return;
                }
                KeyCode::Char('l') => {
                    self.history.clear();
                    self.history.push(Message::system("Conversation cleared."));
                    return;
                }
                _ => {}
            }
        }

        match self.mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::Insert => self.handle_insert_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('i') | KeyCode::Char('a') => {
                self.mode = InputMode::Insert;
                if key.code == KeyCode::Char('a') {
                    self.cursor_col = self.current_line_len();
                }
            }
            KeyCode::Char('o') => {
                self.mode = InputMode::Insert;
                self.input_lines.insert(self.cursor_line + 1, String::new());
                self.cursor_line += 1;
                self.cursor_col = 0;
            }
            KeyCode::Char('O') => {
                self.mode = InputMode::Insert;
                self.input_lines.insert(self.cursor_line, String::new());
                self.cursor_col = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => self.history.scroll_up(1),
            KeyCode::Down | KeyCode::Char('j') => self.history.scroll_down(1),
            KeyCode::PageUp => self.history.scroll_up(10),
            KeyCode::PageDown => self.history.scroll_down(10),
            KeyCode::Char('/') => {
                self.mode = InputMode::Insert;
                self.input_lines = vec![String::from("/")];
                self.cursor_line = 0;
                self.cursor_col = 1;
            }
            KeyCode::Char('G') => {
                self.history.scroll_to_bottom();
            }
            KeyCode::Esc => {
                self.should_quit = true;
            }
            _ => {}
        }
    }

    fn handle_insert_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                let input = self.input_text().trim().to_string();
                if input.is_empty() {
                    return;
                }
                if input.starts_with('/') {
                    commands::handle_command(self, &input);
                    return;
                }
                if self.is_loading {
                    return;
                }
                self.submit(&input);
            }
            KeyCode::Tab => {
                self.insert_str("    ");
            }
            KeyCode::Char(c) => {
                self.insert_char(c);
            }
            KeyCode::Backspace => {
                self.backspace();
            }
            KeyCode::Delete => {
                self.delete_char();
            }
            KeyCode::Left => {
                self.cursor_left();
            }
            KeyCode::Right => {
                self.cursor_right();
            }
            KeyCode::Up => {
                self.cursor_up();
            }
            KeyCode::Down => {
                self.cursor_down();
            }
            KeyCode::Home => {
                self.cursor_col = 0;
            }
            KeyCode::End => {
                self.cursor_col = self.current_line_len();
            }
            _ => {}
        }
    }

    pub fn clear_input(&mut self) {
        self.input_lines = vec![String::new()];
        self.cursor_line = 0;
        self.cursor_col = 0;
    }

    fn submit(&mut self, text: &str) {
        let input = text.to_string();
        self.clear_input();
        self.is_loading = true;

        self.history.push(Message::user(&input));
        self.history.push(Message::assistant("", true));
        self.history.scroll_to_bottom();

        let history: Vec<Message> = self.history.iter().cloned().collect();
        let tx = self.backend_tx.clone();
        let backend = Arc::clone(&self.backend);
        let cycle = self.cycle_idx;

        tokio::spawn(async move {
            let result = backend.stream_chat(&history, tx.clone(), cycle).await;
            if let Err(e) = result {
                let _ = tx.send(format!("ERROR: {e}"));
            }
            let _ = tx.send(String::new());
        });

        self.cycle_idx += 1;
    }

    fn input_text(&self) -> String {
        self.input_lines.join("\n")
    }

    fn current_line_len(&self) -> usize {
        self.input_lines.get(self.cursor_line).map_or(0, |l| l.len())
    }

    fn insert_char(&mut self, c: char) {
        self.ensure_line_exists();
        if let Some(line) = self.input_lines.get_mut(self.cursor_line) {
            let byte_pos = line
                .char_indices()
                .nth(self.cursor_col)
                .map(|(i, _)| i)
                .unwrap_or(line.len());
            line.insert(byte_pos, c);
            self.cursor_col += 1;
        }
    }

    fn insert_str(&mut self, s: &str) {
        self.ensure_line_exists();
        if let Some(line) = self.input_lines.get_mut(self.cursor_line) {
            let byte_pos = line
                .char_indices()
                .nth(self.cursor_col)
                .map(|(i, _)| i)
                .unwrap_or(line.len());
            line.insert_str(byte_pos, s);
            self.cursor_col += s.chars().count();
        }
    }

    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            if let Some(line) = self.input_lines.get_mut(self.cursor_line) {
                let char_idx = self.cursor_col - 1;
                if let Some((byte_pos, _)) = line.char_indices().nth(char_idx) {
                    line.remove(byte_pos);
                    self.cursor_col -= 1;
                }
            }
        } else if self.cursor_line > 0 {
            let removed_line = self.input_lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_len = self.current_line_len();
            self.input_lines[self.cursor_line].push_str(&removed_line);
            self.cursor_col = prev_len;
        }
    }

    fn delete_char(&mut self) {
        if let Some(line) = self.input_lines.get(self.cursor_line) {
            let char_count = line.chars().count();
            if self.cursor_col < char_count {
                let line = self.input_lines.get_mut(self.cursor_line).unwrap();
                if let Some((byte_pos, _)) = line.char_indices().nth(self.cursor_col) {
                    line.remove(byte_pos);
                }
            } else if self.cursor_line + 1 < self.input_lines.len() {
                let next_line = self.input_lines.remove(self.cursor_line + 1);
                self.input_lines[self.cursor_line].push_str(&next_line);
            }
        }
    }

    fn cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.current_line_len();
        }
    }

    fn cursor_right(&mut self) {
        let line_len = self.current_line_len();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.input_lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    fn cursor_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            let len = self.current_line_len();
            if self.cursor_col > len {
                self.cursor_col = len;
            }
        }
    }

    fn cursor_down(&mut self) {
        if self.cursor_line + 1 < self.input_lines.len() {
            self.cursor_line += 1;
            let len = self.current_line_len();
            if self.cursor_col > len {
                self.cursor_col = len;
            }
        }
    }

    fn ensure_line_exists(&mut self) {
        while self.cursor_line >= self.input_lines.len() {
            self.input_lines.push(String::new());
        }
    }
}
