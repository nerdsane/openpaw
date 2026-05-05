use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
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
    pub(crate) duplicate_logic_candidates: usize,
    pub(crate) broad_cedar_policies: usize,
    pub(crate) dependency_risk_hits: usize,
    pub(crate) rust_orchestration_hits: usize,
    pub(crate) polling_loop_hits: usize,
    pub(crate) missing_test_coverage_hits: usize,
}

#[derive(Clone, Debug)]
struct ScannedFile {
    path: PathBuf,
    relative: String,
    content: String,
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
    let mut scanned_files = Vec::new();
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
        scanned_files.push(ScannedFile {
            path: path.clone(),
            relative: relative.clone(),
            content: content.clone(),
        });

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

        if is_dependency_manifest(&path)
            && let Some(finding) = dependency_risk_finding(&relative, &content)
        {
            graph.summary.dependency_risk_hits += 1;
            graph.security_findings.push(finding);
        }

        if is_source_like(&path) && contains_polling_loop(&content) {
            graph.summary.polling_loop_hits += 1;
            graph.quality_findings.push(QualityFindingRecord {
                title: format!("Polling loop needs Temper-native self-loop audit: {relative}"),
                severity: "medium".to_string(),
                evidence: format!(
                    "{relative} combines a loop/while construct with sleep-based waiting."
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

    add_duplicate_logic_findings(&scanned_files, &mut graph);
    add_missing_wasm_test_findings(root, &scanned_files, &mut graph);

    Ok(graph)
}

pub(crate) fn repo_sweep_summary_markdown(root: &Path, graph: &RepoSweepGraph) -> String {
    format!(
        "# Repo Sweep Summary\n\nRoot: {}\nScanned files: {}\nScanned lines: {}\nQuality findings: {}\nSecurity findings: {}\n\nSignals scanned: giant modules, duplicate logic candidates, TODO/HACK band-aids, Cedar drift, dependency/security risks, hidden Rust orchestration, polling loops, and missing WASM test coverage.\n\nDetected counts: giant_modules={}, duplicate_logic_candidates={}, todo_hack_hits={}, broad_cedar_policies={}, dependency_risk_hits={}, rust_orchestration_hits={}, polling_loop_hits={}, missing_test_coverage_hits={}.",
        root.display(),
        graph.summary.scanned_files,
        graph.summary.scanned_lines,
        graph.quality_findings.len(),
        graph.security_findings.len(),
        graph.summary.giant_modules,
        graph.summary.duplicate_logic_candidates,
        graph.summary.todo_hack_hits,
        graph.summary.broad_cedar_policies,
        graph.summary.dependency_risk_hits,
        graph.summary.rust_orchestration_hits,
        graph.summary.polling_loop_hits,
        graph.summary.missing_test_coverage_hits
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

fn add_duplicate_logic_findings(scanned_files: &[ScannedFile], graph: &mut RepoSweepGraph) {
    let mut windows: HashMap<String, Vec<(String, usize)>> = HashMap::new();
    for file in scanned_files
        .iter()
        .filter(|file| is_source_like(&file.path))
    {
        let logic_lines: Vec<(usize, String)> = file
            .content
            .lines()
            .enumerate()
            .filter_map(|(index, line)| normalize_logic_line(line).map(|line| (index + 1, line)))
            .collect();
        if logic_lines.len() < 6 {
            continue;
        }
        for window in logic_lines.windows(6) {
            let fingerprint = window
                .iter()
                .map(|(_, line)| line.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if fingerprint.len() < 80 {
                continue;
            }
            windows
                .entry(fingerprint)
                .or_default()
                .push((file.relative.clone(), window[0].0));
        }
    }

    let mut reported_path_groups = HashSet::new();
    let mut duplicate_groups = Vec::new();
    for entries in windows.into_values() {
        let mut paths = entries
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        if paths.len() < 2 {
            continue;
        }
        let group_key = paths.join("|");
        if reported_path_groups.insert(group_key) {
            duplicate_groups.push((paths, entries));
        }
    }

    duplicate_groups.sort_by(|left, right| left.0.cmp(&right.0));
    for (paths, entries) in duplicate_groups.into_iter().take(10) {
        graph.summary.duplicate_logic_candidates += 1;
        let evidence = entries
            .iter()
            .filter(|(path, _)| paths.contains(path))
            .take(4)
            .map(|(path, line)| format!("{path}:{line}"))
            .collect::<Vec<_>>()
            .join(", ");
        graph.quality_findings.push(QualityFindingRecord {
            title: format!("Duplicate logic candidate across {} files", paths.len()),
            severity: "medium".to_string(),
            evidence: format!(
                "The same normalized six-line logic block appears at {evidence}; review for shared helper extraction."
            ),
            affected_paths: paths,
        });
    }
}

fn add_missing_wasm_test_findings(
    root: &Path,
    scanned_files: &[ScannedFile],
    graph: &mut RepoSweepGraph,
) {
    for manifest in scanned_files
        .iter()
        .filter(|file| is_wasm_crate_manifest(&file.relative))
    {
        let lib_path = root.join(
            Path::new(&manifest.relative)
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join("src/lib.rs"),
        );
        let lib_relative = relative_path(root, &lib_path);
        let Some(lib) = scanned_files
            .iter()
            .find(|file| file.relative == lib_relative)
        else {
            continue;
        };
        if has_rust_tests(&lib.content) {
            continue;
        }
        graph.summary.missing_test_coverage_hits += 1;
        graph.quality_findings.push(QualityFindingRecord {
            title: format!("Missing WASM test coverage: {}", manifest.relative),
            severity: "medium".to_string(),
            evidence: format!(
                "{} has a src/lib.rs entry point but no #[test], #[cfg(test)], or test module marker.",
                manifest.relative
            ),
            affected_paths: vec![manifest.relative.clone(), lib.relative.clone()],
        });
    }
}

fn dependency_risk_finding(relative: &str, content: &str) -> Option<SecurityFindingRecord> {
    if relative.ends_with("Cargo.toml") && content.contains("git =") {
        let detail = if content.contains("rev =") {
            "git dependency is pinned; keep freshness and upstream review explicit"
        } else {
            "git dependency is not pinned to a rev"
        };
        return Some(SecurityFindingRecord {
            title: format!("Dependency risk requires freshness review: {relative}"),
            severity: "medium".to_string(),
            risk_lane: "L1".to_string(),
            evidence: format!("{relative} uses a Cargo git dependency; {detail}."),
            affected_paths: vec![relative.to_string()],
        });
    }

    if relative.ends_with("package.json") && contains_any(content, &["\"latest\"", "\": \"*\""]) {
        return Some(SecurityFindingRecord {
            title: format!("Dependency risk requires freshness review: {relative}"),
            severity: "medium".to_string(),
            risk_lane: "L1".to_string(),
            evidence: format!(
                "{relative} contains a loose npm dependency version such as latest or *."
            ),
            affected_paths: vec![relative.to_string()],
        });
    }

    None
}

fn normalize_logic_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed == "{"
        || trimmed == "}"
        || trimmed.starts_with("//")
        || trimmed.starts_with('#')
    {
        return None;
    }
    Some(trimmed.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn contains_polling_loop(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    let has_wait = lower.contains("sleep(") || lower.contains("settimeout(");
    let has_loop = lower.contains("loop {")
        || lower.contains("while ")
        || lower.contains("setinterval(")
        || lower.contains("for (;;)");
    has_wait && has_loop
}

fn is_dependency_manifest(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("Cargo.toml" | "package.json")
    )
}

fn is_wasm_crate_manifest(relative: &str) -> bool {
    let parts = relative.split('/').collect::<Vec<_>>();
    parts.len() == 5 && parts[0] == "os-apps" && parts[2] == "wasm" && parts[4] == "Cargo.toml"
}

fn has_rust_tests(content: &str) -> bool {
    content.contains("#[test]") || content.contains("#[cfg(test)]") || content.contains("mod tests")
}
