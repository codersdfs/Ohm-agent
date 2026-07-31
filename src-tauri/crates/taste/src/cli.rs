#![allow(dead_code)]

// Simple CLI command enum without clap derive
#[derive(Debug, Clone)]
pub enum TasteCommand {
    Enable,
    Disable,
    Sync,
    List,
    Reset,
}

/// Taste CLI main command
#[derive(Debug, Clone)]
pub struct TasteCli {
    pub command: TasteCommand,
}

/// Parse command from args
impl TasteCli {
    pub fn parse(args: &[&str]) -> Result<Self, String> {
        if args.is_empty() {
            return Ok(TasteCli { command: TasteCommand::List });
        }
        
        let cmd = match args[0] {
            "enable" => TasteCommand::Enable,
            "disable" => TasteCommand::Disable,
            "sync" => TasteCommand::Sync,
            "list" => TasteCommand::List,
            "reset" => TasteCommand::Reset,
            _ => return Err(format!("Unknown command: {}", args[0])),
        };
        
        Ok(TasteCli { command: cmd })
    }
}

/// Run the taste CLI with given arguments
pub fn run(args: &[&str]) -> Result<(), String> {
    let cli = TasteCli::parse(args)?;
    match cli.command {
        TasteCommand::Enable => println!("Taste agent enabled"),
        TasteCommand::Disable => println!("Taste agent disabled"),
        TasteCommand::Sync => println!("Sync not yet implemented"),
        TasteCommand::List => println!("Preferences (stub)"),
        TasteCommand::Reset => println!("Reset (stub)"),
    }
    Ok(())
}
