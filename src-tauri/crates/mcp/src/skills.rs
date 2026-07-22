use crate::Skill;
use std::path::Path;

pub struct SkillsRegistry {
    skills: Vec<Skill>,
}

impl Default for SkillsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillsRegistry {
    pub fn new() -> Self {
        Self { skills: vec![] }
    }

    pub fn register(&mut self, skill: Skill) {
        self.skills.push(skill);
    }

    pub fn list(&self) -> &[Skill] {
        &self.skills
    }

    pub fn find(&self, name: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.name == name)
    }

    pub fn load_dir(path: &Path) -> (Self, Vec<String>) {
        let mut registry = Self::new();
        let mut errors = Vec::new();

        if !path.exists() {
            return (
                registry,
                vec![format!("skills directory not found: {}", path.display())],
            );
        }

        let entries = match std::fs::read_dir(path) {
            Ok(e) => e,
            Err(e) => {
                return (
                    registry,
                    vec![format!("failed to read skills directory: {e}")],
                );
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    errors.push(format!("failed to read dir entry: {e}"));
                    continue;
                }
            };

            let file_path = entry.path();
            let file_name = file_path.to_string_lossy();

            if !file_name.ends_with(".mcp.json") {
                continue;
            }

            match Skill::from_file(&file_path) {
                Ok(skill) => registry.register(skill),
                Err(e) => errors.push(format!("{}: {e}", file_path.display())),
            }
        }

        (registry, errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_find() {
        let mut reg = SkillsRegistry::new();
        reg.register(Skill {
            name: "test".into(),
            description: "A test skill".into(),
            endpoint: "http://localhost:8080".into(),
            parameters: None,
        });

        assert_eq!(reg.list().len(), 1);
        assert!(reg.find("test").is_some());
        assert!(reg.find("nonexistent").is_none());
    }
}
