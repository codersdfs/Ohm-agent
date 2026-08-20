use crossterm::event::{Event, KeyCode, KeyEventState, KeyModifiers, MouseEvent};

pub type KeyState = KeyEventState;

pub struct KeyMsg {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
    pub state: KeyState,
}

pub type MouseMsg = MouseEvent;

pub enum Message {
    Key(KeyMsg),
    Mouse(MouseMsg),
    Resize(u16, u16),
    FocusGained,
    FocusLost,
    #[cfg(feature = "paste")]
    Paste(String),
    Shutdown,
    Tick,
}

impl From<Event> for Message {
    fn from(value: Event) -> Self {
        match value {
            Event::FocusGained => Message::FocusGained,
            Event::FocusLost => Message::FocusLost,
            Event::Key(key) => Message::Key(KeyMsg {
                code: key.code,
                modifiers: key.modifiers,
                state: key.state,
            }),
            Event::Mouse(mouse) => Message::Mouse(mouse),
            #[cfg(feature = "paste")]
            Event::Paste(value) => Message::Paste(value),
            Event::Resize(x, y) => Message::Resize(x, y),
        }
    }
}

#[cfg(feature = "paste")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paste_event_becomes_paste_message_with_full_payload() {
        let payload = "line one\nline two\nline three\n";
        let message = Message::from(Event::Paste(payload.to_string()));
        match message {
            Message::Paste(text) => assert_eq!(text, payload),
            _ => panic!("expected Message::Paste"),
        }
    }

    #[test]
    fn paste_newlines_are_not_enter_keys() {
        // Pasted newlines route through Event::Paste, so they never reach the
        // key/Enter path that maps to send.
        let payload = "a\nb\n";
        match Message::from(Event::Paste(payload.to_string())) {
            Message::Paste(text) => assert_eq!(text, payload),
            _ => panic!("expected Message::Paste"),
        }
    }
}
