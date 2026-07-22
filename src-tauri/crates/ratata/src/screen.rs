use ratatui::Frame;

use crate::{command::Command, message::Message};

pub trait Screen {
    fn render(&mut self, f: &mut Frame<'_>);

    fn update(&mut self, message: Message) -> Option<Command>;
}
