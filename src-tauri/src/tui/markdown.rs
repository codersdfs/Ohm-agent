use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Tag, TagEnd};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use std::fmt::Write;

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const UNDERLINE: &str = "\x1b[4m";
const RESET: &str = "\x1b[0m";

fn dim(s: &str) -> String {
    format!("{}{}{}", DIM, s, RESET)
}
fn bold(s: &str) -> String {
    format!("{}{}{}", BOLD, s, RESET)
}
fn bold_underline(s: &str) -> String {
    format!("{}{}{}{}", BOLD, UNDERLINE, s, RESET)
}

fn highlight_code(code: &str, lang: Option<&str>) -> String {
    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let syntax = lang
        .and_then(|l| ss.find_syntax_by_token(l))
        .unwrap_or_else(|| ss.find_syntax_plain_text());
    let theme = &ts.themes["base16-ocean.dark"];
    let mut h = HighlightLines::new(syntax, theme);
    let mut out = String::new();

    for line in LinesWithEndings::from(code) {
        let ranges = h.highlight_line(line, &ss).unwrap();
        for (style, text) in &ranges {
            if text.is_empty() {
                continue;
            }
            if *text == "\n" || *text == "\r\n" {
                out.push_str(text);
                continue;
            }
            if style.font_style.contains(FontStyle::BOLD) {
                write!(out, "{}{}{}", BOLD, text, RESET).unwrap();
            } else if style.font_style.contains(FontStyle::ITALIC) {
                write!(out, "{}{}{}", DIM, text, RESET).unwrap();
            } else {
                out.push_str(text);
            }
        }
    }
    out
}

pub fn render_markdown(text: &str) -> String {
    let parser = pulldown_cmark::Parser::new(text);
    let mut out = String::new();
    let mut list_depth: Vec<usize> = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_buf = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                if out.ends_with('\n') || out.is_empty() {
                } else if !out.ends_with('\n') {
                    out.push('\n');
                }
                match level {
                    HeadingLevel::H1 => write!(out, "{} ", bold_underline("")).unwrap(),
                    _ => write!(out, "{} ", bold("")).unwrap(),
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                out.push('\n');
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n');
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                in_code_block = true;
                code_buf.clear();
                write!(out, "{}\n", dim("───")).unwrap();
            }
            Event::End(TagEnd::CodeBlock) => {
                 let highlighted = highlight_code(&code_buf, Some(code_lang.as_str()).filter(|l| !l.is_empty()));
                for line in highlighted.lines() {
                    writeln!(out, "  {}", dim(line)).unwrap();
                }
                write!(out, "{}\n", dim("───")).unwrap();
                in_code_block = false;
                code_lang.clear();
                code_buf.clear();
            }
            Event::Text(t) => {
                if in_code_block {
                    code_buf.push_str(&t);
                } else {
                    out.push_str(&t);
                }
            }
            Event::Code(t) => {
                write!(out, "{}{}{}", DIM, t, RESET).unwrap();
            }
            Event::Start(Tag::Emphasis) => {
                write!(out, "{}", DIM).unwrap();
            }
            Event::End(TagEnd::Emphasis) => {
                write!(out, "{}", RESET).unwrap();
            }
            Event::Start(Tag::Strong) => {
                write!(out, "{}", BOLD).unwrap();
            }
            Event::End(TagEnd::Strong) => {
                write!(out, "{}", RESET).unwrap();
            }
            Event::Start(Tag::List(..)) => {
                list_depth.push(0);
            }
            Event::End(TagEnd::List(_)) => {
                list_depth.pop();
            }
            Event::Start(Tag::Item) => {
                if let Some(d) = list_depth.last_mut() {
                    *d += 1;
                }
                let indent = "  ".repeat(list_depth.len());
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                write!(out, "{}{} ", indent, "•").unwrap();
            }
            Event::End(TagEnd::Item) => {
                out.push('\n');
            }
            Event::Start(Tag::BlockQuote(_)) => {}
            Event::End(TagEnd::BlockQuote(_)) => {}
            Event::Start(Tag::Link { .. }) => {}
            Event::End(TagEnd::Link) => {}
            Event::Start(Tag::Table(_)) => {}
            Event::End(TagEnd::Table) => {}
            Event::Start(Tag::TableHead) => {}
            Event::End(TagEnd::TableHead) => { out.push('\n'); }
            Event::Start(Tag::TableRow) => {}
            Event::End(TagEnd::TableRow) => { out.push('\n'); }
            Event::Start(Tag::TableCell) => { out.push_str("│ "); }
            Event::End(TagEnd::TableCell) => { out.push(' '); }
            Event::SoftBreak | Event::HardBreak => {
                out.push('\n');
            }
            Event::Rule => {
                write!(out, "{}\n", dim("───")).unwrap();
            }
            Event::Html(t) => {
                out.push_str(&t);
            }
            Event::FootnoteReference(t) => {
                write!(out, "[^{}]", t).unwrap();
            }
            Event::TaskListMarker(checked) => {
                if checked {
                    out.push_str("[x]");
                } else {
                    out.push_str("[ ]");
                }
            }
            Event::InlineHtml(t) => {
                out.push_str(&t);
            }
            Event::Start(Tag::Strikethrough) => {}
            Event::End(TagEnd::Strikethrough) => {}
            Event::Start(Tag::DefinitionList) => {}
            Event::End(TagEnd::DefinitionList) => {}
            Event::Start(Tag::DefinitionListTitle) => {}
            Event::End(TagEnd::DefinitionListTitle) => {}
            Event::Start(Tag::DefinitionListDefinition) => {}
            Event::End(TagEnd::DefinitionListDefinition) => {}
            Event::Start(Tag::MetadataBlock(_)) => {}
            Event::End(TagEnd::MetadataBlock(_)) => {}
            Event::Start(Tag::Superscript) => {}
            Event::End(TagEnd::Superscript) => {}
            Event::Start(Tag::Subscript) => {}
            Event::End(TagEnd::Subscript) => {}
            Event::InlineMath(_) | Event::DisplayMath(_) => {}
            _ => {}
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bold_text() {
        let result = render_markdown("hello **world**");
        assert!(result.contains("hello"));
        assert!(result.contains(BOLD));
        assert!(result.contains("world"));
    }

    #[test]
    fn test_italic_text() {
        let result = render_markdown("hello *world*");
        assert!(result.contains(DIM));
    }

    #[test]
    fn test_inline_code() {
        let result = render_markdown("use `let` keyword");
        assert!(result.contains(DIM));
        assert!(result.contains("let"));
    }

    #[test]
    fn test_code_block() {
        let result = render_markdown("```rust\nfn main() {}\n```");
        assert!(result.contains("fn main()"));
        assert!(result.contains(DIM));
    }

    #[test]
    fn test_heading() {
        let result = render_markdown("# Title");
        assert!(result.contains(BOLD));
        assert!(result.contains("Title"));
    }

    #[test]
    fn test_unordered_list() {
        let result = render_markdown("- item");
        assert!(result.contains("•"));
        assert!(result.contains("item"));
    }

    #[test]
    fn test_link() {
        let result = render_markdown("[text](url)");
        assert!(result.contains("text"));
    }

    #[test]
    fn test_horizontal_rule() {
        let result = render_markdown("---");
        assert!(result.contains("───"));
    }

    #[test]
    fn test_blockquote() {
        let result = render_markdown("> quote");
        assert!(result.contains("quote"));
    }

    #[test]
    fn test_plain_text() {
        let result = render_markdown("hello world");
        assert!(result.contains("hello world"));
    }
}
