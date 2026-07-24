use crate::{AppState, MutexExt};
use entropy::{DriftScanner, GarbageCollector};

pub async fn run_entropy_scan(state: &AppState, project_root: &str) -> Result<String, String> {
    let scanner = DriftScanner::new();
    let report = scanner.scan(project_root).await?;

    let mut output = format!("Entropy Scan Report {}\n\n", report.timestamp);
    for domain in &report.domains {
        let priority_str = match domain.priority {
            1 => "HIGH",
            2 => "MEDIUM",
            _ => "LOW",
        };
        output.push_str(&format!(
            "  {} [{}]: drift={:.1}, violations={}\n",
            domain.name, priority_str, domain.drift, domain.violations
        ));
    }

    Ok(output)
}

pub async fn run_entropy_gc(state: &AppState, project_root: &str, fix: bool) -> Result<String, String> {
    let gc = GarbageCollector::new();
    gc.collect(project_root, fix)
}
