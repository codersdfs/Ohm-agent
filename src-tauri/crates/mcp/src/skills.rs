use crate::Skill;

pub struct SkillsRegistry {
    skills: Vec<Skill>,
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
}
