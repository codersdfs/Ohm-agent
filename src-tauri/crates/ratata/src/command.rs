use std::any::TypeId;
use std::fmt;
use std::io;

use crate::screen::Screen;

mod macros {
    #[macro_export]
    macro_rules! __batch {
        ($($command:expr),* $(,)?) => ($crate::command::Command::Batch(vec![$($command),*]));
        () => (Vec::new());
    }

    pub use __batch as batch;
}

pub use macros::batch;

pub enum Command {
    Batch(Vec<Self>),
    Screen(TypeId),
    EnableRawMode,
    DisableRawMode,
    Crossterm(#[allow(private_interfaces)] ObjectSafeCrosstermCommand),
    Quit,
}

impl Command {
    #[inline(always)]
    pub fn screen<S: Screen + 'static>() -> Command {
        Self::Screen(TypeId::of::<S>())
    }

    #[inline(always)]
    pub fn crossterm<C>(command: C) -> Command
    where
        C: crossterm::Command + 'static,
    {
        Self::Crossterm(ObjectSafeCrosstermCommand(Box::new(command)))
    }
}

pub(crate) trait ObjectSafeCommand {
    fn object_safe_write_ansi(&self, f: &mut dyn fmt::Write) -> fmt::Result;
}

impl<T: crossterm::Command> ObjectSafeCommand for T {
    fn object_safe_write_ansi(&self, mut f: &mut dyn fmt::Write) -> fmt::Result {
        self.write_ansi(&mut f)
    }
}

pub(crate) struct ObjectSafeCrosstermCommand(Box<dyn ObjectSafeCommand>);

impl crossterm::Command for ObjectSafeCrosstermCommand {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        self.0.object_safe_write_ansi(f)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> Result<(), io::Error> {
        // Object-safe wrapper can't forward winapi — fallible only on Windows
        // where the calling code expects an ansi-capable backend.
        Ok(())
    }
}
