use crate::Language;
use tree_sitter::{Language as TSLanguage, Node, Parser};

#[derive(Debug, Clone)]
pub struct FunctionMetric {
    pub name: String,
    pub start_line: u32,
    pub end_line: u32,
    pub line_count: u32,
    pub cyclomatic_complexity: u32,
}

pub fn analyze_file(content: &str, lang: &Language) -> Vec<FunctionMetric> {
    let ts_lang = get_ts_language(lang);
    let ts_lang = match ts_lang {
        Some(l) => l,
        None => return vec![],
    };

    let mut parser = Parser::new();
    if parser.set_language(ts_lang).is_err() {
        log::warn!("Failed to set tree-sitter language");
        return vec![];
    }

    let tree = match parser.parse(content.as_bytes(), None) {
        Some(t) => t,
        None => {
            log::warn!("Failed to parse content with tree-sitter");
            return vec![];
        }
    };

    let root = tree.root_node();

    let source = content.as_bytes();
    let mut metrics = vec![];
    collect_functions(root, source, &mut metrics);
    metrics
}

fn get_ts_language(lang: &Language) -> Option<TSLanguage> {
    match lang {
        Language::Rust => Some(tree_sitter_rust::language()),
        Language::TypeScript | Language::TypeScriptReact => Some(tree_sitter_typescript::language_typescript()),
        Language::JavaScript => Some(tree_sitter_typescript::language_typescript()),
        Language::Python => Some(tree_sitter_python::language()),
        _ => None,
    }
}

fn collect_functions(node: Node, source: &[u8], metrics: &mut Vec<FunctionMetric>) {
    let kind = node.kind();
    if is_function_kind(kind) {
        if let Some(name) = get_function_name(&node, source) {
            let start_line = node.start_position().row as u32 + 1;
            let end_line = node.end_position().row as u32 + 1;
            let line_count = end_line - start_line + 1;
            let complexity = cyclomatic_complexity(&node, source);
            metrics.push(FunctionMetric {
                name,
                start_line,
                end_line,
                line_count,
                cyclomatic_complexity: complexity,
            });
        }
    }

    // Recurse into children
    let mut i = 0;
    while i < node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            collect_functions(child, source, metrics);
        }
        i += 1;
    }
}

fn is_function_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_definition" | "function_declaration" | "arrow_function" | "method_definition" | "fn_item" | "function_item"
    )
}

fn get_function_name(node: &Node, source: &[u8]) -> Option<String> {
    let kind = node.kind();
    match kind {
        "function_item" | "function_definition" | "function_declaration" => {
            // Try field name first, then fall back to child lookup
            if let Some(name_node) = node.child_by_field_name("name") {
                return name_node.utf8_text(source).ok().map(|s| s.to_string());
            }
            // Fall back: find the identifier child (skip 'fn' keyword)
            for i in 0..node.named_child_count() {
                if let Some(child) = node.named_child(i) {
                    if child.kind() == "identifier" {
                        return child.utf8_text(source).ok().map(|s| s.to_string());
                    }
                }
            }
            None
        }
        "method_definition" => {
            // TypeScript: class { method() { ... } }
            node.child_by_field_name("name").and_then(|n| n.utf8_text(source).ok().map(|s| s.to_string()))
        }
        "arrow_function" => {
            // Arrow functions may not have names; try parent
            node.parent().and_then(|p| {
                if p.kind() == "variable_declarator" {
                    p.child_by_field_name("name").and_then(|n| n.utf8_text(source).ok().map(|s| s.to_string()))
                } else {
                    None
                }
            })
        }
        _ => None,
    }
}

/// Calculate cyclomatic complexity by counting decision points.
pub fn cyclomatic_complexity(node: &Node, source: &[u8]) -> u32 {
    let mut complexity = 1u32;
    count_decision_points(node, source, &mut complexity);
    complexity
}

fn count_decision_points(node: &Node, source: &[u8], complexity: &mut u32) {
    let kind = node.kind();

    // Decision points that increase cyclomatic complexity
    if matches!(
        kind,
        "if_expression"
            | "if_statement"
            | "while_expression"
            | "while_statement"
            | "for_expression"
            | "for_statement"
            | "do_statement"
            | "match_arm"
            | "switch_case"
            | "catch_clause"
            | "conditional_expression"
    ) {
        *complexity += 1;
    }

    // Logical operators && and ||
    if kind == "binary_expression" || kind == "binary_operator" {
        if let Ok(text) = node.utf8_text(source) {
            if text.contains("&&") || text.contains("||") {
                *complexity += 1;
            }
        }
    }

    // Recurse
    let mut i = 0;
    while i < node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            count_decision_points(&child, source, complexity);
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_rust_function_length() {
        let content = r#"
fn short() {
    let x = 1;
}

fn too_long() {
    let a = 1;
    let b = 2;
    let c = 3;
    let d = 4;
    let e = 5;
    let f = 6;
    let g = 7;
    let h = 8;
    let i = 9;
    let j = 10;
}
"#;
        let metrics = analyze_file(content, &Language::Rust);
        assert_eq!(metrics.len(), 2);
        let long_fn = metrics.iter().find(|m| m.name == "too_long").unwrap();
        assert!(long_fn.line_count > 5);
    }

    #[test]
    fn analyze_rust_cyclomatic_complexity() {
        let content = r#"
fn complex(a: i32, b: i32) -> i32 {
    if a > 0 && b > 0 {
        if a > b {
            return a;
        } else {
            return b;
        }
    } else {
        return 0;
    }
}
"#;
        let metrics = analyze_file(content, &Language::Rust);
        let complex_fn = metrics.iter().find(|m| m.name == "complex").unwrap();
        assert!(complex_fn.cyclomatic_complexity >= 3, "complexity should be >= 3, got {}", complex_fn.cyclomatic_complexity);
    }

    #[test]
    fn analyze_typescript_function_length() {
        let content = r#"
function short() {
    return 1;
}

function tooLong() {
    const a = 1;
    const b = 2;
    const c = 3;
    const d = 4;
    const e = 5;
    const f = 6;
    const g = 7;
    const h = 8;
    const i = 9;
    const j = 10;
}
"#;
        let metrics = analyze_file(content, &Language::TypeScript);
        assert_eq!(metrics.len(), 2);
        let long_fn = metrics.iter().find(|m| m.name == "tooLong").unwrap();
        assert!(long_fn.line_count > 5);
    }

    #[test]
    fn analyze_python_function_length() {
        let content = r#"
def short():
    return 1

def too_long():
    a = 1
    b = 2
    c = 3
    d = 4
    e = 5
    f = 6
    g = 7
    h = 8
    i = 9
    j = 10
"#;
        let metrics = analyze_file(content, &Language::Python);
        assert_eq!(metrics.len(), 2);
        let long_fn = metrics.iter().find(|m| m.name == "too_long").unwrap();
        assert!(long_fn.line_count > 5);
    }
}
