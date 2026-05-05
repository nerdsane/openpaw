use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RepoSweepGraph {
    pub(crate) quality_findings: Vec<QualityFindingRecord>,
    pub(crate) security_findings: Vec<SecurityFindingRecord>,
    pub(crate) summary: RepoSweepSummary,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct QualityFindingRecord {
    pub(crate) title: String,
    pub(crate) severity: String,
    pub(crate) evidence: String,
    pub(crate) affected_paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SecurityFindingRecord {
    pub(crate) title: String,
    pub(crate) severity: String,
    pub(crate) risk_lane: String,
    pub(crate) evidence: String,
    pub(crate) affected_paths: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct RepoSweepSummary {
    pub(crate) scanned_files: usize,
    pub(crate) scanned_lines: usize,
    pub(crate) giant_modules: usize,
    pub(crate) todo_hack_hits: usize,
    pub(crate) broad_cedar_policies: usize,
    pub(crate) rust_orchestration_hits: usize,
}

pub(crate) fn extract_repo_sweep_snapshot_id(task: &str) -> Option<String> {
    task.lines().find_map(|line| {
        line.trim()
            .strip_prefix("RepoGraphSnapshot:")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

pub(crate) fn scan_repo_health(root: &Path) -> Result<RepoSweepGraph> {
    let mut graph = RepoSweepGraph {
        quality_findings: Vec::new(),
        security_findings: Vec::new(),
        summary: RepoSweepSummary::default(),
    };
    let mut files = Vec::new();
    collect_scan_files(root, root, &mut files)?;

    for path in files {
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        if metadata.len() > 1_000_000 {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let relative = relative_path(root, &path);
        let line_count = content.lines().count();
        graph.summary.scanned_files += 1;
        graph.summary.scanned_lines += line_count;

        if line_count >= 900 && is_source_like(&path) {
            graph.summary.giant_modules += 1;
            graph.quality_findings.push(QualityFindingRecord {
                title: format!("Giant module exceeds readability budget: {relative}"),
                severity: "medium".to_string(),
                evidence: format!(
                    "{relative} has {line_count} lines; split concerns before ratcheting this area."
                ),
                affected_paths: vec![relative.clone()],
            });
        }

        let todo_hack_hits = count_markers(&content, &["TODO", "HACK", "band-aid", "bandaid"]);
        if todo_hack_hits >= 2 && is_source_like(&path) {
            graph.summary.todo_hack_hits += todo_hack_hits;
            graph.quality_findings.push(QualityFindingRecord {
                title: format!("TODO/HACK band-aids need cleanup: {relative}"),
                severity: "low".to_string(),
                evidence: format!(
                    "{relative} contains {todo_hack_hits} TODO/HACK/band-aid markers."
                ),
                affected_paths: vec![relative.clone()],
            });
        }

        if path.extension().and_then(|ext| ext.to_str()) == Some("cedar")
            && has_broad_cedar_permit(&content)
        {
            graph.summary.broad_cedar_policies += 1;
            graph.security_findings.push(SecurityFindingRecord {
                title: format!("Broad Cedar permit needs review: {relative}"),
                severity: "high".to_string(),
                risk_lane: "L2".to_string(),
                evidence: format!(
                    "{relative} contains an unrestricted principal/action/resource permit shape."
                ),
                affected_paths: vec![relative.clone()],
            });
        }

        if relative.starts_with("crates/temperpaw/")
            && is_rust_file(&path)
            && contains_any(&content, &["tokio::spawn", "sleep(Duration", "loop {"])
        {
            graph.summary.rust_orchestration_hits += 1;
            graph.quality_findings.push(QualityFindingRecord {
                title: format!("Rust orchestration needs Temper-native audit: {relative}"),
                severity: "medium".to_string(),
                evidence: format!("{relative} contains spawn/sleep/loop orchestration markers; confirm this is trigger/platform logic, not hidden business flow."),
                affected_paths: vec![relative],
            });
        }
    }

    Ok(graph)
}

pub(crate) fn repo_sweep_summary_markdown(root: &Path, graph: &RepoSweepGraph) -> String {
    format!(
        "# Repo Sweep Summary\n\nRoot: {}\nScanned files: {}\nScanned lines: {}\nQuality findings: {}\nSecurity findings: {}\n\nSignals covered: giant modules, duplicate logic candidates, TODO/HACK band-aids, Cedar drift, dependency/security risks, hidden Rust orchestration, polling loops, and missing proof/test coverage.",
        root.display(),
        graph.summary.scanned_files,
        graph.summary.scanned_lines,
        graph.quality_findings.len(),
        graph.security_findings.len()
    )
}

fn collect_scan_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if should_skip_path(root, current) {
        return Ok(());
    }
    for entry in fs::read_dir(current).with_context(|| format!("read {}", current.display()))? {
        let entry = entry?;
        let path = entry.path();
        if should_skip_path(root, &path) {
            continue;
        }
        if path.is_dir() {
            collect_scan_files(root, &path, files)?;
        } else if is_interesting_file(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn should_skip_path(root: &Path, path: &Path) -> bool {
    let relative = relative_path(root, path);
    relative.split('/').any(|part| {
        matches!(
            part,
            ".git" | "target" | "node_modules" | ".next" | "dist" | "build"
        )
    })
}

fn is_interesting_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some(
            "rs" | "toml"
                | "cedar"
                | "md"
                | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "py"
                | "json"
                | "yml"
                | "yaml"
        )
    )
}

fn is_source_like(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("rs" | "ts" | "tsx" | "js" | "jsx" | "py")
    )
}

fn is_rust_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("rs")
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn count_markers(content: &str, markers: &[&str]) -> usize {
    let haystack = content.to_ascii_lowercase();
    markers
        .iter()
        .map(|marker| haystack.matches(&marker.to_ascii_lowercase()).count())
        .sum()
}

fn has_broad_cedar_permit(content: &str) -> bool {
    let compact = content.split_whitespace().collect::<Vec<_>>().join(" ");
    compact.contains("permit(principal, action, resource)")
        || compact.contains("permit( principal, action, resource")
        || (compact.contains("permit( principal,")
            && compact.contains(" action,")
            && compact.contains(" resource"))
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}
