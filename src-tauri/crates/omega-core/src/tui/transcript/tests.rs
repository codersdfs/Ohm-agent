//! Transcript tests (P5 split).

use super::state::{parse_args_kv, ToolCallState};
use super::toolbox::{MAX_RETAINED_SOURCE_LINES, MAX_SOURCE_COLUMNS};
use super::*;

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn text_to_string(t: &Text<'static>) -> String {
        t.lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.clone())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn test_has_attachments_url() {
        let entry = TranscriptEntry::User {
            content: "check https://example.com".into(),
        };
        assert!(entry.has_attachments());
    }

    #[test]
    fn test_has_attachments_http() {
        let entry = TranscriptEntry::User {
            content: "see http://localhost:8080".into(),
        };
        assert!(entry.has_attachments());
    }

    #[test]
    fn test_no_attachments_plain_text() {
        let entry = TranscriptEntry::User {
            content: "hello world".into(),
        };
        assert!(!entry.has_attachments());
    }

    #[test]
    fn test_has_attachments_file_path() {
        let entry = TranscriptEntry::User {
            content: "read /etc/config.toml".into(),
        };
        assert!(entry.has_attachments());
    }

    #[test]
    fn test_has_attachments_file_extension() {
        let entry = TranscriptEntry::User {
            content: "look at Cargo.toml".into(),
        };
        assert!(entry.has_attachments());
    }

    #[test]
    fn test_no_attachments_bare_domain() {
        let entry = TranscriptEntry::User {
            content: "visit example.com".into(),
        };
        assert!(!entry.has_attachments());
    }

    #[test]
    fn test_has_attachments_non_user_entry() {
        let entry = TranscriptEntry::Notice {
            text: "hello".into(),
            is_error: false,
        };
        assert!(!entry.has_attachments());
    }

    #[test]
    fn streaming_assistant_no_spinner() {
        let mut entry = TranscriptEntry::Assistant {
            content: String::new(),
            rendered: None,
            is_streaming: true,
            thinking: String::new(),
        };
        let first = text_to_string(&entry.get_rendered(60, 0));
        assert!(!first.contains("⠋"));
        assert!(!first.contains("Cooking…"));
        assert!(!first.to_lowercase().contains("thinking"));
    }

    #[test]
    fn test_parse_args_kv_empty() {
        let kv = parse_args_kv("");
        assert!(kv.is_empty());
    }

    #[test]
    fn test_parse_args_kv_json_object() {
        let kv = parse_args_kv(r#"{"filePath": "src/main.rs", "limit": 2000}"#);
        assert!(kv.len() >= 2);
        assert!(kv.iter().any(|(k, _)| k == "filePath"));
        assert!(kv.iter().any(|(k, _)| k == "limit"));
    }

    #[test]
    fn test_tool_call_state_title_pending() {
        let state = ToolCallState::new("read".into(), r#"{}"#.into());
        let title = state.title();
        assert!(title.contains("read"));
        assert!(title.contains("▶"));
    }

    #[test]
    fn print_all_tool_boxes() {
        let test_data: Vec<(&str, &str, Option<&str>)> = vec![
            ("read", r#"{"filePath": "src/main.rs"}"#, Some("pub fn main() {\n    println!(\"hello\");\n}")),
            ("write", r#"{"filePath": "hello.txt", "content": "Hello world"}"#, Some("wrote 11 bytes to hello.txt")),
            ("edit", r#"{"filePath": "src/main.rs", "oldString": "foo", "newString": "bar"}"#, Some("patched src/main.rs")),
            ("bash", "cargo build", Some("Compiling omega-core v0.1.0\nerror[E0425]: cannot find value `x` in this scope\n\nerror: could not compile `omega-core` (lib) due to 1 previous error")),
            ("glob", r#"{"pattern": "**/*.rs"}"#, Some("src/main.rs\nsrc/lib.rs\nsrc/utils.rs")),
            ("grep", r#"{"pattern": "fn main", "include": "*.rs"}"#, Some("src/main.rs:42: pub fn main() {")),
            ("bash", "", None),
        ];

        println!("\n═══ Tool call box renders ═══\n");
        for (tool, args, result) in &test_data {
            let result_owned = result.map(|s| s.to_string());
            let entry = TranscriptEntry::ToolCall {
                tool_name: tool.to_string(),
                args: args.to_string(),
                result: result_owned,
            };
            let mut entry_clone = entry.clone();
            let rendered = entry_clone.get_rendered(80, 0);
            println!("→ {} {} {}", tool, args, result.unwrap_or("(running)"));
            println!("{}", text_to_string(&rendered));
            println!();
        }
    }

    #[test]
    fn write_tool_code_is_boxed_and_width_safe() {
        let content = (1..=10)
            .map(|n| format!("\tprintln!(\"line {} — Ω\");", n))
            .collect::<Vec<_>>()
            .join("\n");
        let args = serde_json::json!({
            "filePath": "src/Ωmega.rs",
            "content": content,
        })
        .to_string();
        let state = ToolCallState::new("write".into(), args);
        assert!(
            state.args.is_empty(),
            "full write payload must not be retained"
        );
        assert_eq!(state.write_preview.as_ref().unwrap().lines.len(), 10);
        let mut entry = TranscriptEntry::ToolCallBox { state };

        let rendered = entry.get_rendered(60, 0);
        let output = text_to_string(&rendered);
        let lines: Vec<&str> = output.lines().collect();

        assert!(output.contains("write  src/Ωmega.rs"));
        assert!(output.contains("RUNNING"));
        assert!(output.contains("println!"));
        assert!(output.contains("[Ctrl+E] expand"));

        let expected = lines[0].chars().count();
        for (index, line) in lines.iter().enumerate() {
            assert_eq!(
                line.chars().count(),
                expected,
                "write panel line {} has inconsistent width: '{}'",
                index,
                line
            );
        }
    }

    #[test]
    fn large_write_event_keeps_bounded_transcript_state() {
        let long_line = "Ω".repeat(10_000);
        let content = std::iter::repeat(long_line)
            .take(2_000)
            .collect::<Vec<_>>()
            .join("\n");
        let original_chars = content.chars().count();
        let args = serde_json::json!({
            "filePath": "src/huge.rs",
            "content": content,
        })
        .to_string();

        let mut transcript = Transcript::new();
        transcript.process_stream_event(&crate::tui::component::UiStreamEvent::ToolCall {
            name: "write".into(),
            args,
        });

        let TranscriptEntry::ToolCallBox { state } = transcript.entries.last().unwrap() else {
            panic!("write event should create bounded ToolCallBox state");
        };
        let preview = state.write_preview.as_ref().unwrap();
        assert!(state.args.is_empty());
        assert_eq!(preview.lines.len(), MAX_RETAINED_SOURCE_LINES);
        assert_eq!(preview.omitted_lines, 2_000 - MAX_RETAINED_SOURCE_LINES);
        let retained_chars: usize = preview.lines.iter().map(|line| line.chars().count()).sum();
        assert!(retained_chars <= MAX_RETAINED_SOURCE_LINES * MAX_SOURCE_COLUMNS);
        assert!(retained_chars < original_chars / 500);
    }

    #[test]
    fn large_edit_is_bounded_and_does_not_render_all_code() {
        let old = (1..=2_000)
            .map(|n| format!("old line {} {}", n, "Ω".repeat(200)))
            .collect::<Vec<_>>()
            .join("\n");
        let new = (1..=2_000)
            .map(|n| {
                if n == 2_000 {
                    "SENTINEL_MUST_NOT_RENDER".to_string()
                } else {
                    format!("new line {} {}", n, "λ".repeat(200))
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let args = serde_json::json!({
            "filePath": "src/large.rs",
            "oldString": old,
            "newString": new,
        })
        .to_string();

        let state = ToolCallState::new("edit".into(), args);
        assert!(
            state.args.is_empty(),
            "full edit payload must not be retained"
        );
        let preview = state.edit_preview.as_ref().unwrap();
        assert_eq!(preview.removed.len(), MAX_RETAINED_SOURCE_LINES);
        assert_eq!(preview.added.len(), MAX_RETAINED_SOURCE_LINES);
        assert_eq!(preview.omitted_removed, 2_000 - MAX_RETAINED_SOURCE_LINES);
        assert_eq!(preview.omitted_added, 2_000 - MAX_RETAINED_SOURCE_LINES);

        let mut entry = TranscriptEntry::ToolCallBox { state };
        let rendered = entry.get_rendered(60, 0);
        let output = text_to_string(&rendered);
        assert!(output.contains("edit  src/large.rs"));
        assert!(output.contains("RUNNING"));
        assert!(output.contains("more lines"));
        assert!(!output.contains("SENTINEL_MUST_NOT_RENDER"));

        let lines: Vec<&str> = output.lines().collect();
        let expected = lines[0].chars().count();
        for (index, line) in lines.iter().enumerate() {
            assert_eq!(
                line.chars().count(),
                expected,
                "edit panel line {} has inconsistent width: '{}'",
                index,
                line
            );
        }
    }

    #[test]
    fn global_expansion_applies_to_existing_and_new_source_tools() {
        let args = serde_json::json!({
            "filePath": "src/main.rs",
            "content": (1..=20).map(|n| format!("line {}", n)).collect::<Vec<_>>().join("\n"),
        })
        .to_string();
        let mut transcript = Transcript::new();
        transcript.process_stream_event(&crate::tui::component::UiStreamEvent::ToolCall {
            name: "write".into(),
            args: args.clone(),
        });
        transcript.set_tools_expanded(true);
        transcript.process_stream_event(&crate::tui::component::UiStreamEvent::ToolCall {
            name: "write".into(),
            args,
        });

        for entry in &transcript.entries {
            let TranscriptEntry::ToolCallBox { state } = entry else {
                continue;
            };
            assert!(state.expanded);
        }
    }

    #[test]
    fn tool_box_accent_bar() {
        let mut entry = TranscriptEntry::ToolCall {
            tool_name: "bash".into(),
            args: "cargo build --release".into(),
            result: Some("Compiling...\nFinished\n".into()),
        };
        let rendered = entry.get_rendered(60, 0);
        let s = text_to_string(&rendered);
        let lines: Vec<&str> = s.lines().collect();

        // Left-accent-bar design: first line starts with │ (accent)
        assert!(
            lines[0].starts_with("│"),
            "accent bar should start with │, got: {:?}",
            lines[0]
        );
        // Tool name should appear somewhere in first line
        assert!(
            lines[0].contains("bash"),
            "tool name \"bash\" should appear in first line, got: {:?}",
            lines[0]
        );

        // All subsequent lines should also start with │ (accent continuation)
        for line in &lines[1..] {
            assert!(
                line.starts_with("│"),
                "continuation line should start with │, got: {:?}",
                line
            );
        }

        println!("Tool accent bar OK — {} lines", lines.len(),);
    }
}
