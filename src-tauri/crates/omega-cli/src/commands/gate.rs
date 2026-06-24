use crate::app::App;
use crate::message::Message;
use harness::engine::GateEngine;
use harness::Language;

pub fn cmd_gate(app: &mut App, args: &str) {
    if args.is_empty() {
        app.history
            .push(Message::system("Usage: /gate <file-path>"));
        return;
    }

    let path = args.trim();
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            app.history
                .push(Message::system(format!("Error reading {path}: {e}")));
            return;
        }
    };

    let project_root = app
        .project_root
        .to_string_lossy()
        .to_string();

    let file_paths = vec![path.to_string()];
    let language = Language::detect(&file_paths);

    let mut engine = GateEngine::new(project_root, language);
    let result = engine.check_file(path, &content);

    let score_color = if result.score >= 80 {
        "PASS"
    } else if result.score >= 60 {
        "WARN"
    } else {
        "FAIL"
    };

    let mut output = format!("Gate: {score_color} — score {}/100", result.score);

    if !result.violations.is_empty() {
        output.push_str(&format!("\n\n{} violation(s):", result.violations.len()));
        for v in &result.violations {
            let line_info = v.line.map_or(String::new(), |l| format!(" line {l}"));
            let category = format!("{:?}", v.category);
            output.push_str(&format!(
                "\n  [{category}]{}: {}",
                line_info,
                v.message
            ));
            if let Some(hint) = &v.tool_hint {
                output.push_str(&format!("\n    Hint: {hint}"));
            }
        }
    }

    app.history.push(Message::system(output));
}

pub fn cmd_gate_score(app: &mut App, args: &str) {
    if args.is_empty() {
        app.history
            .push(Message::system("Usage: /gate-score <file-path>"));
        return;
    }

    let path = args.trim();
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            app.history
                .push(Message::system(format!("Error reading {path}: {e}")));
            return;
        }
    };

    let project_root = app
        .project_root
        .to_string_lossy()
        .to_string();

    let file_paths = vec![path.to_string()];
    let language = Language::detect(&file_paths);

    let mut engine = GateEngine::new(project_root, language);
    let result = engine.check_file(path, &content);

    let label = if result.passed { "PASS" } else { "FAIL" };
    app.history
        .push(Message::system(format!("{}: {}/100", label, result.score)));
}
