use crate::ChatEmitter;

pub struct NoopEmitter;

impl ChatEmitter for NoopEmitter {
    fn emit_token(&self, _token: &str) -> Result<(), String> {
        Ok(())
    }
    fn emit_done(&self, _full: &str) -> Result<(), String> {
        Ok(())
    }
    fn emit_error(&self, _error: &str) -> Result<(), String> {
        Ok(())
    }
}

pub enum Permission {
    Allow,
    Deny,
    Abort,
}

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub async fn check_permission<E: ChatEmitter>(
    mode: &str,
    tool: &str,
    _args: &str,
    emitter: &E,
) -> Permission {
    match mode {
        "strict" => {
            if emitter.allows_direct_terminal_output() {
                eprintln!("  {}{} denied (strict mode){}", DIM, tool, RESET);
            } else {
                log::info!("{} denied (strict mode)", tool);
            }
            Permission::Deny
        }
        "on" => {
            if !emitter.allows_direct_terminal_output() {
                log::info!("{} auto-approved (TUI permission prompt unavailable)", tool);
                return Permission::Allow;
            }
            use std::io::Write;
            use tokio::io::AsyncBufReadExt;
            let mut input = String::new();
            let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
            loop {
                eprint!("  Allow {}? (y/N/q): ", tool);
                std::io::stderr().flush().ok();
                input.clear();
                if reader.read_line(&mut input).await.is_err() {
                    return Permission::Deny;
                }
                match input.trim().to_lowercase().as_str() {
                    "y" | "yes" => return Permission::Allow,
                    "" | "n" | "no" => return Permission::Deny,
                    "q" | "quit" => return Permission::Abort,
                    _ => continue,
                }
            }
        }
        _ => Permission::Allow,
    }
}
