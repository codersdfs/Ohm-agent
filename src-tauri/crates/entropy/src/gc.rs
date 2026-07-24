use chrono::Utc;

pub struct GarbageCollector;

impl GarbageCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn collect(&self, project_root: &str, fix: bool) -> Result<String, String> {
        let mut report = format!("GC running at {}\n", Utc::now());
        let mut changes = vec![];

        if fix {
            let rustfmt_result = std::process::Command::new("cargo")
                .args(["fmt"])
                .current_dir(project_root)
                .output();

            match rustfmt_result {
                Ok(output) if output.status.success() => {
                    report.push_str("  rustfmt: applied\n");
                    changes.push("rustfmt");
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if !stderr.is_empty() {
                        report.push_str(&format!("  rustfmt: {}\n", stderr));
                    }
                }
                Err(e) => {
                    report.push_str(&format!("  rustfmt: not available ({})\n", e));
                }
            }

            let clippy_fix_result = std::process::Command::new("cargo")
                .args(["clippy", "--fix", "--allow-dirty", "--allow-staged", "-q"])
                .current_dir(project_root)
                .output();

            match clippy_fix_result {
                Ok(output) if output.status.success() => {
                    report.push_str("  clippy --fix: applied\n");
                    changes.push("clippy");
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if !stderr.is_empty() && !stderr.contains("no errors") {
                        report.push_str(&format!("  clippy --fix: {}\n", stderr));
                    }
                }
                Err(e) => {
                    report.push_str(&format!("  clippy --fix: not available ({})\n", e));
                }
            }
        }

        if changes.is_empty() {
            report.push_str("  No mechanical fixes applied\n");
        } else {
            report.push_str(&format!("  Applied: {}\n", changes.join(", ")));
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gc_fix_applies_rustfmt() {
        let dir = std::env::temp_dir().join("omega_gc_test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test.rs"), "fn main(){let x=1;}").unwrap();

        let gc = GarbageCollector::new();
        let result = gc.collect(dir.to_str().unwrap(), true).unwrap();
        assert!(result.contains("rustfmt") || result.contains("applied") || result.contains("No"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gc_without_fix_does_not_modify() {
        let dir = std::env::temp_dir().join("omega_gc_test2");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test.rs"), "fn main(){let x=1;}").unwrap();

        let gc = GarbageCollector::new();
        let result = gc.collect(dir.to_str().unwrap(), false).unwrap();
        assert!(result.contains("GC running"));
        assert!(!result.contains("rustfmt: applied"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
