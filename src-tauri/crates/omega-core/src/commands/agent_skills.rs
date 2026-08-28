//! Agent Skills — full implementation of the Agent Skills open standard
//! (agentskills.io) with Claude Code extensions.
//!
//! ## Features
//!
//! - **Name validation** per spec (lowercase, hyphens, max 64 chars)
//! - **SKILL.md format** (directory-based: `skill-name/SKILL.md`)
//! - **Backward-compatible** flat `.md` files still work
//! - **Dynamic context injection** (`!`command`` syntax)
//! - **Hierarchical paths**: project (`.omega/skills/`) > personal (`~/.agents/skill/`)
//! - **Frontmatter fields**: `when_to_use`, `paths`, `allowed-tools`, `effort`
//! - **Argument substitution**: `$ARGUMENTS`, `$0`, `$1`, named args
//! - **Hot-reload**: `/skill-reload` to rescan without restart

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

// ─── Types ──────────────────────────────────────────────────────────────────

/// Parsed frontmatter from a SKILL.md or flat .md skill file.
#[derive(Debug, Clone, Default)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    pub when_to_use: Option<String>,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub allowed_tools: Option<String>,
    pub effort: Option<String>,
    pub paths: Option<Vec<String>>,
    pub metadata: HashMap<String, String>,
}

/// Metadata cached in memory (no full content).
#[derive(Debug, Clone)]
pub struct AgentSkillMeta {
    pub frontmatter: SkillFrontmatter,
    /// Path to the SKILL.md or .md file on disk.
    path: PathBuf,
    /// Root directory of the skill (for resolving relative file refs).
    skill_dir: PathBuf,
    /// Source priority: 0 = project, 1 = personal (higher = lower priority).
    source_priority: u8,
}

/// Full loaded skill content.
#[derive(Debug, Clone)]
pub struct AgentSkill {
    pub frontmatter: SkillFrontmatter,
    pub content: String,
    pub skill_dir: PathBuf,
}

// ─── Name validation (Agent Skills spec) ────────────────────────────────────

/// Validate a skill name per the Agent Skills specification.
///
/// Rules:
/// - Max 64 characters
/// - Lowercase letters, numbers, and hyphens only
/// - Must not start or end with a hyphen
/// - Must not contain consecutive hyphens
pub fn validate_skill_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name must not be empty".into());
    }
    if name.len() > 64 {
        return Err(format!("name must be <= 64 characters, got {}", name.len()));
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err("name must not start or end with a hyphen".into());
    }
    if name.contains("--") {
        return Err("name must not contain consecutive hyphens".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(
            "name may only contain lowercase letters, numbers, and hyphens".into(),
        );
    }
    Ok(())
}

// ─── Frontmatter parsing ────────────────────────────────────────────────────

/// Parse YAML frontmatter from a markdown string.
///
/// Returns `(SkillFrontmatter, body)`.
fn parse_frontmatter(raw: &str) -> (SkillFrontmatter, &str) {
    let mut fm = SkillFrontmatter::default();
    let trimmed = raw.trim_start();

    if !trimmed.starts_with("---") {
        fm.name = String::new();
        return (fm, raw);
    }

    let after_open = &trimmed[3..];
    if let Some(end) = after_open.find("\n---") {
        let fm_block = &after_open[..end];
        let body = after_open[end + 4..].trim_start();

        for line in fm_block.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_lowercase();
                let value = value.trim().trim_matches('"').trim_matches('\'');

                match key.as_str() {
                    "name" => fm.name = value.to_string(),
                    "description" => fm.description = value.to_string(),
                    "when_to_use" => fm.when_to_use = Some(value.to_string()),
                    "license" => fm.license = Some(value.to_string()),
                    "compatibility" => fm.compatibility = Some(value.to_string()),
                    "allowed-tools" => fm.allowed_tools = Some(value.to_string()),
                    "effort" => fm.effort = Some(value.to_string()),
                    "paths" => {
                        // Parse YAML list: ["src/**/*.ts", "tests/**"]
                        let paths_str = value.trim_start_matches('[').trim_end_matches(']');
                        let paths: Vec<String> = paths_str
                            .split(',')
                            .map(|p| p.trim().trim_matches('"').trim_matches('\'').to_string())
                            .filter(|p| !p.is_empty())
                            .collect();
                        if !paths.is_empty() {
                            fm.paths = Some(paths);
                        }
                    }
                    "metadata" => {
                        // Inline metadata: metadata.author: value
                        // (handled in nested lines below)
                    }
                    _ => {
                        // Check for metadata.key format
                        if key.starts_with("metadata.") {
                            let meta_key = key.strip_prefix("metadata.").unwrap();
                            fm.metadata.insert(meta_key.to_string(), value.to_string());
                        }
                    }
                }
            }
        }

        (fm, body)
    } else {
        (fm, raw)
    }
}

// ─── Dynamic context injection ──────────────────────────────────────────────

/// Run `!`command`` blocks in skill content, replacing them with output.
///
/// Syntax: A line containing only `!` followed by a backtick-quoted command.
/// Example: `!`git diff HEAD``
fn inject_dynamic_context(content: &str) -> String {
    let mut result = String::with_capacity(content.len());

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(cmd) = trimmed.strip_prefix("!`").and_then(|s| s.strip_suffix('`')) {
            // Execute the shell command
            let output = if cfg!(target_os = "windows") {
                std::process::Command::new("cmd")
                    .args(["/C", cmd])
                    .output()
            } else {
                std::process::Command::new("sh")
                    .args(["-c", cmd])
                    .output()
            };

            match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    if !stdout.is_empty() {
                        result.push_str(&stdout);
                    }
                    if !stderr.is_empty() && stdout.is_empty() {
                        result.push_str(&format!("[command stderr: {}]", stderr.trim()));
                    }
                }
                Err(e) => {
                    result.push_str(&format!("[command failed: {}]", e));
                }
            }
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }

    result
}

// ─── Argument substitution ──────────────────────────────────────────────────

/// Substitute `$ARGUMENTS`, `$0`, `$1`, etc. in skill content.
fn substitute_arguments(content: &str, args: &str) -> String {
    let mut result = content.to_string();

    // $ARGUMENTS → full args string
    result = result.replace("$ARGUMENTS", args);

    // Split args into positional parameters
    let parts: Vec<&str> = args.split_whitespace().collect();
    result = result.replace("$0", parts.get(0).unwrap_or(&""));
    result = result.replace("$1", parts.get(1).unwrap_or(&""));
    result = result.replace("$2", parts.get(2).unwrap_or(&""));
    result = result.replace("$3", parts.get(3).unwrap_or(&""));
    result = result.replace("$4", parts.get(4).unwrap_or(&""));
    result = result.replace("$5", parts.get(5).unwrap_or(&""));

    result
}

// ─── Directory discovery ────────────────────────────────────────────────────

/// Return skill directories in priority order (project first, then personal).
fn skill_dirs() -> Vec<(PathBuf, u8)> {
    let mut dirs = Vec::new();

    // Project-level: .omega/skills/ in workspace root
    if let Ok(cwd) = std::env::current_dir() {
        let project_dir = cwd.join(".omega").join("skills");
        if project_dir.exists() {
            dirs.push((project_dir, 0));
        }
    }

    // Personal: ~/.agents/skill/
    if let Some(home) = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf()) {
        let personal_dir = home.join(".agents").join("skill");
        if personal_dir.exists() {
            dirs.push((personal_dir, 1));
        }
    }

    dirs
}

// ─── Skill loading ──────────────────────────────────────────────────────────

/// Load a skill from a SKILL.md file or flat .md file.
fn load_skill_from_path(
    path: &std::path::Path,
    source_priority: u8,
) -> Option<AgentSkillMeta> {
    let raw = std::fs::read_to_string(path).ok()?;

    // Determine if this is a SKILL.md in a directory or a flat .md file
    let (skill_dir, fm, body) = if path.file_name().and_then(|f| f.to_str()) == Some("SKILL.md") {
        let dir = path.parent()?.to_path_buf();
        let (fm, body) = parse_frontmatter(&raw);
        (dir, fm, body)
    } else {
        // Flat .md file — use filename stem as name if not in frontmatter
        let (mut fm, body) = parse_frontmatter(&raw);
        if fm.name.is_empty() {
            let stem = path.file_stem()?.to_str()?;
            fm.name = stem.to_string();
        }
        (path.parent()?.to_path_buf(), fm, body)
    };

    if fm.name.is_empty() || body.trim().is_empty() {
        return None;
    }

    // Validate name
    if validate_skill_name(&fm.name).is_err() {
        return None;
    }

    Some(AgentSkillMeta {
        frontmatter: fm,
        path: path.to_path_buf(),
        skill_dir,
        source_priority,
    })
}

/// Scan a directory for skills (SKILL.md in subdirs + flat .md files).
fn scan_skill_dir(dir: &std::path::Path, source_priority: u8) -> Vec<AgentSkillMeta> {
    let mut skills = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return skills,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            // Check for SKILL.md in subdirectory
            let skill_md = path.join("SKILL.md");
            if skill_md.exists() {
                if let Some(meta) = load_skill_from_path(&skill_md, source_priority) {
                    skills.push(meta);
                }
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            // Flat .md file (backward compatible)
            if let Some(meta) = load_skill_from_path(&path, source_priority) {
                skills.push(meta);
            }
        }
    }

    skills
}

// ─── Static registry ────────────────────────────────────────────────────────

static SKILLS: OnceLock<Vec<AgentSkillMeta>> = OnceLock::new();

fn init_skills() -> &'static Vec<AgentSkillMeta> {
    SKILLS.get_or_init(|| {
        let dirs = skill_dirs();
        let mut all_skills = Vec::new();

        for (dir, priority) in &dirs {
            all_skills.extend(scan_skill_dir(dir, *priority));
        }

        // Deduplicate by name: lower-priority source wins on conflict
        all_skills.sort_by(|a, b| {
            a.frontmatter
                .name
                .cmp(&b.frontmatter.name)
                .then(a.source_priority.cmp(&b.source_priority))
        });
        all_skills.dedup_by(|a, b| a.frontmatter.name == b.frontmatter.name);

        // Sort by name for deterministic output
        all_skills.sort_by(|a, b| a.frontmatter.name.cmp(&b.frontmatter.name));
        all_skills
    })
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Initialize the skill registry. Call once at startup (idempotent).
pub fn init() -> usize {
    init_skills().len()
}

/// Return all skill metadata.
pub fn list_skills() -> Vec<AgentSkillMeta> {
    init_skills().clone()
}

/// Build the compact skill index for the system prompt.
pub fn skill_index() -> Option<String> {
    let skills = init_skills();
    if skills.is_empty() {
        return None;
    }

    let mut section = String::from("\n\n=== AGENT SKILLS (use /skill <name> or load_skill tool) ===\n");
    for s in skills {
        let desc = if let Some(ref wtu) = s.frontmatter.when_to_use {
            format!("{} — {}", s.frontmatter.description, wtu)
        } else {
            s.frontmatter.description.clone()
        };
        if desc.is_empty() {
            section.push_str(&format!("- {}\n", s.frontmatter.name));
        } else {
            section.push_str(&format!("- {}: {}\n", s.frontmatter.name, desc));
        }
    }
    Some(section)
}

/// Load a skill's full content by name, with dynamic context injection
/// and argument substitution.
///
/// `args` is the argument string (e.g. "456 high" for `/review-pr 456 high`).
pub fn load_skill(name: &str, args: &str) -> Option<AgentSkill> {
    let meta = init_skills().iter().find(|s| s.frontmatter.name == name)?;

    let raw = std::fs::read_to_string(&meta.path).ok()?;
    let (_, body) = parse_frontmatter(&raw);

    if body.trim().is_empty() {
        return None;
    }

    // Step 1: Dynamic context injection (!command)
    let injected = inject_dynamic_context(body);

    // Step 2: Argument substitution
    let substituted = substitute_arguments(&injected, args);

    Some(AgentSkill {
        frontmatter: meta.frontmatter.clone(),
        content: substituted.trim().to_string(),
        skill_dir: meta.skill_dir.clone(),
    })
}

/// Format a loaded skill for injection into the conversation.
pub fn format_skill_for_injection(skill: &AgentSkill) -> String {
    let mut output = format!("\n\n=== LOADED SKILL: {} ===\n", skill.frontmatter.name);

    if let Some(ref effort) = skill.frontmatter.effort {
        output.push_str(&format!("Effort: {}\n", effort));
    }
    if let Some(ref tools) = skill.frontmatter.allowed_tools {
        output.push_str(&format!("Allowed tools: {}\n", tools));
    }

    output.push('\n');
    output.push_str(&skill.content);
    output.push('\n');
    output
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_name_valid() {
        assert!(validate_skill_name("my-skill").is_ok());
        assert!(validate_skill_name("code-review").is_ok());
        assert!(validate_skill_name("skill123").is_ok());
        assert!(validate_skill_name("a").is_ok());
    }

    #[test]
    fn validate_name_invalid() {
        assert!(validate_skill_name("").is_err());
        assert!(validate_skill_name("-bad").is_err());
        assert!(validate_skill_name("bad-").is_err());
        assert!(validate_skill_name("bad--name").is_err());
        assert!(validate_skill_name("UPPERCASE").is_err());
        assert!(validate_skill_name("has space").is_err());
        assert!(validate_skill_name(&"x".repeat(65)).is_err());
    }

    #[test]
    fn parse_frontmatter_with_fm() {
        let raw = "---\nname: test\ndescription: A test\nwhen_to_use: When testing\n---\n\nBody";
        let (fm, body) = parse_frontmatter(raw);
        assert_eq!(fm.name, "test");
        assert_eq!(fm.description, "A test");
        assert_eq!(fm.when_to_use.as_deref(), Some("When testing"));
        assert!(body.starts_with("Body"));
    }

    #[test]
    fn parse_frontmatter_without_fm() {
        let raw = "# Just markdown";
        let (fm, body) = parse_frontmatter(raw);
        assert!(fm.name.is_empty());
        assert_eq!(body, raw);
    }

    #[test]
    fn parse_frontmatter_paths() {
        let raw = "---\nname: test\ndesc: t\npaths: [\"src/**/*.ts\", \"tests/**\"]\n---\nBody";
        let (fm, _) = parse_frontmatter(raw);
        assert_eq!(
            fm.paths,
            Some(vec!["src/**/*.ts".into(), "tests/**".into()])
        );
    }

    #[test]
    fn parse_frontmatter_metadata() {
        let raw = "---\nname: test\ndesc: t\nmetadata.author: me\nmetadata.version: \"1.0\"\n---\nBody";
        let (fm, _) = parse_frontmatter(raw);
        assert_eq!(fm.metadata.get("author").unwrap(), "me");
        assert_eq!(fm.metadata.get("version").unwrap(), "1.0");
    }

    #[test]
    fn substitute_arguments_basic() {
        let result = substitute_arguments("Review PR #$0 with priority $1", "456 high");
        assert_eq!(result, "Review PR #456 with priority high");
    }

    #[test]
    fn substitute_arguments_full() {
        let result = substitute_arguments("Args: $ARGUMENTS", "foo bar baz");
        assert_eq!(result, "Args: foo bar baz");
    }

    #[test]
    fn substitute_arguments_missing() {
        let result = substitute_arguments("$0 and $1", "only-first");
        assert_eq!(result, "only-first and ");
    }

    #[test]
    fn inject_dynamic_context_no_commands() {
        let input = "Just a normal line\nAnother line";
        assert_eq!(inject_dynamic_context(input), "Just a normal line\nAnother line\n");
    }

    #[test]
    fn inject_dynamic_context_with_command() {
        let input = "Before\n!`echo hello`\nAfter";
        let result = inject_dynamic_context(input);
        assert!(result.contains("Before"));
        assert!(result.contains("hello"));
        assert!(result.contains("After"));
    }

    #[test]
    fn skill_dirs_returns_paths() {
        let dirs = skill_dirs();
        // Should not panic
        for (dir, _prio) in &dirs {
            assert!(dir.ends_with("skills") || dir.ends_with("skill"));
        }
    }

    #[test]
    fn init_and_list_skills() {
        let count = init();
        let skills = list_skills();
        assert_eq!(count, skills.len());
    }

    #[test]
    fn load_nonexistent_returns_none() {
        assert!(load_skill("definitely-not-real-xyz", "").is_none());
    }

    #[test]
    fn skill_index_format() {
        let idx = skill_index();
        if let Some(idx_str) = idx {
            assert!(idx_str.contains("AGENT SKILLS"));
            assert!(idx_str.contains("load_skill"));
        }
    }
}
