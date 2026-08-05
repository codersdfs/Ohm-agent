use crate::ChatEmitter;

const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub fn show_diff<E: ChatEmitter>(path: &str, old: &str, new: &str, emitter: &E) {
    if old == new {
        return;
    }
    if !emitter.allows_direct_terminal_output() {
        return;
    }
    eprintln!("  {} {} {}", "──", path, "──");
    let diff = similar::TextDiff::from_lines(old, new);
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            similar::ChangeTag::Delete => "-",
            similar::ChangeTag::Insert => "+",
            similar::ChangeTag::Equal => " ",
        };
        let line = change.value().trim_end_matches('\n');
        if line.is_empty() {
            continue;
        }
        match change.tag() {
            similar::ChangeTag::Equal => {
                eprintln!("  {} {}{}{}", sign, DIM, line, RESET);
            }
            _ => {
                eprintln!("  {} {}", sign, line);
            }
        }
    }
}
