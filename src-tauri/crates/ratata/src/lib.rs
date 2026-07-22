pub mod application;
pub mod command;
pub mod events;
pub mod message;
pub mod screen;

pub use ratatui;

pub mod prelude {
    pub use crossterm::event::{KeyCode, KeyModifiers};

    pub use ratatui::backend::CrosstermBackend;
    pub use ratatui::Frame;

    pub use crate::application::Builder as Application;
    pub use crate::command::{self, Command};
    pub use crate::message::{KeyMsg, KeyState, Message, MouseMsg};
    pub use crate::screen::Screen;
}
