use super::Violation;
use super::ViolationCategory;
use serde::Deserialize;
use std::process::Command;

/// A violation from an external linter.
#[derive(Debug, Clone)]
pub struct ExternalViolation {
    pub message: String,
    pub file: String,
    pub line: Option<u32>,
    pub severity: String,
}

impl ExternalViolation {
    /// Convert to a harness Violation for unified scoring.
    pub fn to_violation(&self) -> Violation {
        Violation {
            category: ViolationCategory::External,
            message: format!(
                "[{}] {}:{} {}",
                self.severity,
                self.file,
                self.line.unwrap_or(0),
                self.message
            ),
            tool_hint: Some("Fix the lint error reported by the external linter".into()),
            line: self.line,
        }
    }
}

/// Run clippy with JSON output and parse violations.
pub fn run_clippy(project_root: &str) -> Vec<ExternalViolation> {
    let result = Command::new("cargo")
        .args(["clippy", "-q", "--message-format=json"])
        .current_dir(project_root)
        .output();

    match result {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            parse_clippy_output(&stdout)
        }
        Ok(output) => {
            // clippy returns non-zero on warnings/errors
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stdout.is_empty() {
                log::warn!("clippy produced no output: {}", stderr);
                vec![]
            } else {
                parse_clippy_output(&stdout)
            }
        }
        Err(e) => {
            log::warn!("Failed to run clippy: {}", e);
            vec![]
        }
    }
}

/// Run eslint with JSON output and parse violations.
pub fn run_eslint(project_root: &str) -> Vec<ExternalViolation> {
    let result = Command::new("npx")
        .args(["eslint", ".", "-f", "json"])
        .current_dir(project_root)
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            parse_eslint_output(&stdout)
        }
        Err(e) => {
            log::warn!("Failed to run eslint: {}", e);
            vec![]
        }
    }
}

/// Run tsc with JSON output and parse violations.
pub fn run_tsc(project_root: &str) -> Vec<ExternalViolation> {
    let result = Command::new("npx")
        .args(["tsc", "--noEmit", "--pretty", "false", "-d"])
        .current_dir(project_root)
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            // tsc outputs to stderr
            if stdout.is_empty() {
                parse_tsc_output(&stderr)
            } else {
                parse_tsc_output(&stdout)
            }
        }
        Err(e) => {
            log::warn!("Failed to run tsc: {}", e);
            vec![]
        }
    }
}

/// Run ruff with JSON output and parse violations.
pub fn run_ruff(project_root: &str) -> Vec<ExternalViolation> {
    let result = Command::new("ruff")
        .args(["check", "--output-format", "json"])
        .current_dir(project_root)
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            parse_ruff_output(&stdout)
        }
        Err(e) => {
            log::warn!("Failed to run ruff: {}", e);
            vec![]
        }
    }
}

// ── Parsers ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
struct ClippyMessage {
    reason: String,
    #[serde(default)]
    message: ClippyInner,
    #[serde(default)]
    target: Option<serde_json::Value>,
}

#[derive(Deserialize, Default)]
#[allow(dead_code)]
struct ClippyInner {
    message: String,
    #[serde(default)]
    spans: Vec<ClippySpan>,
    #[serde(default)]
    code: Option<ClippyCode>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ClippySpan {
    file_name: String,
    line_start: u32,
    #[serde(default)]
    text: Vec<ClippyText>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ClippyText {
    text: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ClippyCode {
    code: String,
    #[serde(default)]
    explanation: Option<String>,
}

pub fn parse_clippy_output(json: &str) -> Vec<ExternalViolation> {
    let mut violations = vec![];
    for line in json.lines() {
        if let Ok(msg) = serde_json::from_str::<ClippyMessage>(line) {
            if msg.reason == "compiler-message" {
                if let Some(span) = msg.message.spans.first() {
                    violations.push(ExternalViolation {
                        message: msg.message.message.clone(),
                        file: span.file_name.clone(),
                        line: Some(span.line_start),
                        severity: "error".to_string(),
                    });
                }
            }
        }
    }
    violations
}

#[derive(Deserialize)]
struct EslintFile {
    #[serde(rename = "filePath")]
    file_path: String,
    messages: Vec<EslintMessage>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct EslintMessage {
    rule_id: Option<String>,
    severity: u32,
    message: String,
    line: Option<u32>,
}

pub fn parse_eslint_output(json: &str) -> Vec<ExternalViolation> {
    let mut violations = vec![];
    if let Ok(files) = serde_json::from_str::<Vec<EslintFile>>(json) {
        for file in files {
            for msg in file.messages {
                violations.push(ExternalViolation {
                    message: msg.message.clone(),
                    file: file.file_path.clone(),
                    line: msg.line,
                    severity: if msg.severity >= 2 { "error" } else { "warn" }.to_string(),
                });
            }
        }
    }
    violations
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct TscError {
    file: Option<String>,
    start: Option<u32>,
    #[serde(default)]
    length: Option<u32>,
    #[serde(rename = "messageText")]
    message_text: String,
    #[serde(default)]
    code: Option<u32>,
}

pub fn parse_tsc_output(json: &str) -> Vec<ExternalViolation> {
    let mut violations = vec![];
    if let Ok(errors) = serde_json::from_str::<Vec<TscError>>(json) {
        for err in errors {
            violations.push(ExternalViolation {
                message: format!("{} (code: {:?})", err.message_text, err.code),
                file: err.file.unwrap_or_default(),
                line: err.start,
                severity: "error".to_string(),
            });
        }
    }
    violations
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct RuffViolation {
    location: RuffLocation,
    message: String,
    code: Option<String>,
    filename: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct RuffLocation {
    row: u32,
    column: u32,
}

pub fn parse_ruff_output(json: &str) -> Vec<ExternalViolation> {
    let mut violations = vec![];
    if let Ok(results) = serde_json::from_str::<Vec<RuffViolation>>(json) {
        for r in results {
            violations.push(ExternalViolation {
                message: format!("{} ({})", r.message, r.code.unwrap_or_default()),
                file: r.filename.clone(),
                line: Some(r.location.row),
                severity: "error".to_string(),
            });
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clippy_json_output() {
        let json = r#"{"reason":"compiler-message","message":{"message":"unused variable","spans":[{"file_name":"src/main.rs","line_start":5,"text":[{"text":"let x = 1;"}]}],"code":{"code":"unused_variables","explanation":null}}}"#;
        let violations = parse_clippy_output(json);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("unused variable"));
        assert_eq!(violations[0].file, "src/main.rs");
        assert_eq!(violations[0].line, Some(5));
    }

    #[test]
    fn parse_eslint_json_output() {
        let json = r#"[{"filePath":"src/app.ts","messages":[{"ruleId":"no-console","severity":2,"message":"Unexpected console statement.","line":10,"column":3}]}]"#;
        let violations = parse_eslint_output(json);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("console"));
        assert_eq!(violations[0].file, "src/app.ts");
        assert_eq!(violations[0].line, Some(10));
    }

    #[test]
    fn parse_tsc_json_output() {
        let json = r#"[{"file":"src/index.ts","start":10,"length":5,"messageText":"Cannot find name 'foo'.","code":2304}]"#;
        let violations = parse_tsc_output(json);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("Cannot find name"));
    }

    #[test]
    fn parse_ruff_json_output() {
        let json = r#"[{"location":{"row":3,"column":5},"message":"List comprehension redefines 'x'","code":"B023","filename":"src/utils.py"}]"#;
        let violations = parse_ruff_output(json);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains("redefines"));
        assert_eq!(violations[0].line, Some(3));
    }

    #[test]
    fn external_violation_to_violation() {
        let ext = ExternalViolation {
            message: "test error".into(),
            file: "src/main.rs".into(),
            line: Some(10),
            severity: "error".into(),
        };
        let v = ext.to_violation();
        assert_eq!(v.category, ViolationCategory::External);
        assert!(v.message.contains("test error"));
    }
}
