use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedReceiver;

pub enum AppEvent {
    Key(KeyEvent),
}

pub fn start_poller() -> UnboundedReceiver<AppEvent> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    std::thread::spawn(move || {
        let mut last_key: Option<(KeyEvent, Instant)> = None;

        loop {
            if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(CrosstermEvent::Key(key)) = event::read() {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    let now = Instant::now();
                    if let Some((last, last_time)) = last_key {
                        if key == last && now.duration_since(last_time) < Duration::from_millis(50)
                        {
                            continue;
                        }
                    }
                    last_key = Some((key, now));
                    let _ = tx.send(AppEvent::Key(key));
                }
            }
        }
    });

    rx
}
