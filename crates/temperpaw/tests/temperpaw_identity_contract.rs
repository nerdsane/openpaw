use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

const LEGACY_IDENTITY_TERMS: [&str; 8] = [
    "OPENPAW", "OpenPAW", "OpenPaw", "Open Paw", "openpaw", "open paw", "open_paw", "open-paw",
];

const LEGACY_IDENTITY_ALLOWLIST: [(&str, &str, &str); 24] = [
    (
        "crates/temperpaw/tests/datadog_observability_contract.rs",
        "\"openpaw.\"",
        "test-only assertions for Datadog legacy cleanup paths",
    ),
    (
        "crates/temperpaw/tests/datadog_observability_contract.rs",
        "\"legacy_openpaw_monitor\"",
        "test-only assertions for Datadog legacy cleanup paths",
    ),
    (
        "crates/temperpaw/tests/datadog_observability_contract.rs",
        "\"legacy_openpaw_dashboard\"",
        "test-only assertions for Datadog legacy cleanup paths",
    ),
    (
        "crates/temperpaw/tests/datadog_observability_contract.rs",
        "\"slack-openpaw-alerts\"",
        "test-only assertions for Datadog legacy cleanup paths",
    ),
    (
        "crates/temperpaw/tests/datadog_observability_contract.rs",
        "\"service:openpaw\"",
        "test-only assertions for Datadog legacy cleanup paths",
    ),
    (
        "scripts/deploy_monitors.py",
        "LEGACY_OPENPAW_MONITOR_TERMS",
        "Datadog monitor deploy must find and delete live legacy monitors",
    ),
    (
        "scripts/deploy_monitors.py",
        "\"OpenPaw\"",
        "Datadog monitor deploy must find and delete live legacy monitors",
    ),
    (
        "scripts/deploy_monitors.py",
        "\"OpenPAW\"",
        "Datadog monitor deploy must find and delete live legacy monitors",
    ),
    (
        "scripts/deploy_monitors.py",
        "\"openpaw\"",
        "Datadog monitor deploy must find and delete live legacy monitors",
    ),
    (
        "scripts/deploy_monitors.py",
        "\"service:openpaw\"",
        "Datadog monitor deploy must find and delete live legacy monitors",
    ),
    (
        "scripts/deploy_monitors.py",
        "\"slack-openpaw-alerts\"",
        "Datadog monitor deploy must find and delete live legacy monitors",
    ),
    (
        "scripts/deploy_monitors.py",
        "legacy_openpaw_monitor",
        "Datadog monitor deploy must find and delete live legacy monitors",
    ),
    (
        "scripts/deploy_monitors.py",
        "legacy OpenPaw identity",
        "Datadog monitor deploy must document legacy cleanup matching",
    ),
    (
        "scripts/deploy_pipelines.py",
        "legacy openpaw",
        "Datadog pipeline deploy must delete live legacy log metrics",
    ),
    (
        "scripts/deploy_pipelines.py",
        "LEGACY_LOG_METRIC_PREFIXES",
        "Datadog pipeline deploy must delete live legacy log metrics",
    ),
    (
        "scripts/deploy_dashboard.py",
        "LEGACY_DASHBOARD_TERMS",
        "Datadog dashboard deploy must find and delete live legacy dashboards",
    ),
    (
        "scripts/deploy_dashboard.py",
        "\"OpenPaw\"",
        "Datadog dashboard deploy must find and delete live legacy dashboards",
    ),
    (
        "scripts/deploy_dashboard.py",
        "\"OpenPAW\"",
        "Datadog dashboard deploy must find and delete live legacy dashboards",
    ),
    (
        "scripts/deploy_dashboard.py",
        "\"openpaw\"",
        "Datadog dashboard deploy must find and delete live legacy dashboards",
    ),
    (
        "scripts/deploy_dashboard.py",
        "\"service:openpaw\"",
        "Datadog dashboard deploy must find and delete live legacy dashboards",
    ),
    (
        "scripts/deploy_dashboard.py",
        "\"slack-openpaw-alerts\"",
        "Datadog dashboard deploy must find and delete live legacy dashboards",
    ),
    (
        "scripts/deploy_dashboard.py",
        "legacy_openpaw_dashboard",
        "Datadog dashboard deploy must find and delete live legacy dashboards",
    ),
    (
        "docs/temperpaw-datadog-observability-guide.md",
        "PUBLISHED_BLOB_BUCKET=openpaw-fs-seshendranalla",
        "operator guide documents the current live bucket/domain migration gap",
    ),
    (
        "docs/temperpaw-datadog-observability-guide.md",
        "service:openpaw OR OpenPAW OR OpenPaw",
        "operator guide records the legacy-query proof used to verify cleanup",
    ),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn is_historical_or_proof(path: &Path) -> bool {
    let path = path.to_string_lossy();
    path.starts_with("docs/adrs/")
        || path.starts_with("docs/proofs/")
        || path.starts_with(".proofs/")
        || path.contains("/adrs/")
        || path.ends_with("crates/temperpaw/tests/temperpaw_identity_contract.rs")
        || path.ends_with("docs/temperpaw-identity-and-observability-success-contract.md")
}

fn is_generated_or_build_artifact(path: &Path) -> bool {
    path.components().any(|component| {
        let value = component.as_os_str();
        value == OsStr::new("target") || value == OsStr::new("node_modules")
    }) || path
        .file_name()
        .is_some_and(|name| name == OsStr::new("Cargo.lock"))
}

fn is_text_candidate(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some(
            "cedar"
                | "env"
                | "example"
                | "json"
                | "md"
                | "py"
                | "rs"
                | "sh"
                | "svelte"
                | "toml"
                | "ts"
                | "txt"
                | "yaml"
                | "yml"
        )
    ) || path.file_name().is_some_and(|name| {
        matches!(
            name.to_str(),
            Some("Dockerfile" | "Makefile" | "AGENTS.md" | "README.md")
        )
    })
}

fn is_allowlisted_legacy_reference(path: &Path, line: &str) -> bool {
    let path = path.to_string_lossy();
    LEGACY_IDENTITY_ALLOWLIST
        .iter()
        .any(|(allowed_path, allowed_line, _reason)| {
            path.as_ref() == *allowed_path
                && (allowed_line.is_empty() || line.contains(allowed_line))
        })
}

fn collect_files(root: &Path, relative_dir: &Path, files: &mut Vec<PathBuf>) {
    let dir = root.join(relative_dir);
    let entries =
        fs::read_dir(&dir).unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}"));
        let relative_path = relative_dir.join(entry.file_name());
        let file_type = entry
            .file_type()
            .unwrap_or_else(|err| panic!("failed to stat {}: {err}", relative_path.display()));

        if is_generated_or_build_artifact(&relative_path) || is_historical_or_proof(&relative_path)
        {
            continue;
        }

        if file_type.is_dir() {
            collect_files(root, &relative_path, files);
        } else if file_type.is_file() && is_text_candidate(&relative_path) {
            files.push(relative_path);
        }
    }
}

#[test]
fn active_surfaces_do_not_use_legacy_openpaw_identity() {
    let root = repo_root();
    let mut files = Vec::new();

    for dir in [
        Path::new(".github"),
        Path::new("crates"),
        Path::new("dashboard"),
        Path::new("dd-dashboards"),
        Path::new("dd-log-metrics"),
        Path::new("dd-monitors"),
        Path::new("dd-pipelines"),
        Path::new("docs"),
        Path::new("os-apps"),
        Path::new("scripts"),
    ] {
        collect_files(&root, dir, &mut files);
    }

    for file in [
        Path::new(".env.example"),
        Path::new("DEPLOYMENT.md"),
        Path::new("Dockerfile"),
        Path::new("README.md"),
        Path::new("railway.toml"),
    ] {
        files.push(file.to_path_buf());
    }

    let mut failures = Vec::new();
    for relative_path in files {
        let path = root.join(&relative_path);
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", relative_path.display()));
        for (line_idx, line) in content.lines().enumerate() {
            if LEGACY_IDENTITY_TERMS.iter().any(|term| line.contains(term))
                && !is_allowlisted_legacy_reference(&relative_path, line)
            {
                failures.push(format!(
                    "{}:{}: {}",
                    relative_path.display(),
                    line_idx + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "active TemperPaw surfaces still contain legacy OpenPAW identity:\n{}",
        failures.join("\n")
    );
}

#[test]
fn dockerignore_excludes_local_runtime_state_from_production_images() {
    let dockerignore = fs::read_to_string(repo_root().join(".dockerignore"))
        .expect(".dockerignore should be readable");

    for required in [
        ".git",
        "target",
        "**/target",
        "dashboard/node_modules",
        "**/node_modules",
        ".env",
        ".proofs",
        ".wrangler",
    ] {
        assert!(
            dockerignore.lines().any(|line| line.trim() == required),
            ".dockerignore must exclude `{required}` so production image contexts do not include local state or proof artifacts"
        );
    }
}

#[test]
fn docker_image_metadata_uses_temperpaw_identity() {
    let workflow_path = repo_root().join(".github/workflows/docker.yml");
    let workflow = fs::read_to_string(&workflow_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", workflow_path.display()));

    assert!(
        workflow.contains("org.opencontainers.image.description=TemperPaw"),
        "Docker OCI description must be pinned to TemperPaw so metadata-action cannot inherit stale repository descriptions"
    );

    assert!(
        workflow.contains("annotations: |\n            org.opencontainers.image.title=TemperPaw\n            org.opencontainers.image.description=TemperPaw - Agent daemon built on Temper platform"),
        "Docker manifest annotations must be pinned to TemperPaw so GHCR package metadata cannot inherit stale repository descriptions"
    );

    assert!(
        workflow.contains("annotations: ${{ steps.meta.outputs.annotations }}"),
        "Docker build-push-action must publish docker/metadata-action annotations"
    );

    assert!(
        workflow.contains("DOCKER_METADATA_ANNOTATIONS_LEVELS: manifest")
            && !workflow.contains("DOCKER_METADATA_ANNOTATIONS_LEVELS: manifest,index"),
        "Docker annotations must target manifest level only because the single-platform build cannot export index annotations"
    );

    assert!(
        !workflow.contains("org.opencontainers.image.description=Open Paw")
            && !workflow.contains("org.opencontainers.image.description=OpenPaw")
            && !workflow.contains("org.opencontainers.image.description=OpenPAW"),
        "Docker OCI description must not carry legacy OpenPAW identity"
    );
}
