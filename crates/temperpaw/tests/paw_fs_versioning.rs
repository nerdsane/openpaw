use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path.as_ref())
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.as_ref().display()))
}

#[test]
fn paw_fs_specs_use_explicit_counter_assignment_and_spawn_fresh_versions() {
    let root = repo_root();
    let file_spec = read(root.join("os-apps/paw-fs/specs/file.ioa.toml"));
    let file_version_spec = read(root.join("os-apps/paw-fs/specs/file_version.ioa.toml"));
    let workspace_spec = read(root.join("os-apps/paw-fs/specs/workspace.ioa.toml"));

    for needle in [
        "name = \"last_version_id\"",
        "type = \"set_counter_from_param\", var = \"size_bytes\", param = \"size_bytes\"",
        "type = \"spawn\", entity_type = \"FileVersion\"",
        "store_id_in = \"last_version_id\"",
        "\"version_number\", \"previous_version_id\", \"created_by\"",
    ] {
        assert!(
            file_spec.contains(needle),
            "file spec should contain {needle}"
        );
    }

    for needle in [
        "name = \"mime_type\"",
        "name = \"previous_version_id\"",
        "type = \"set_counter_from_param\", var = \"version_number\", param = \"version_number\"",
        "type = \"set_counter_from_param\", var = \"size_bytes\", param = \"size_bytes\"",
    ] {
        assert!(
            file_version_spec.contains(needle),
            "file_version spec should contain {needle}"
        );
    }

    for needle in
        ["type = \"set_counter_from_param\", var = \"quota_limit\", param = \"quota_limit\""]
    {
        assert!(
            workspace_spec.contains(needle),
            "workspace spec should contain {needle}"
        );
    }
}

#[test]
fn paw_fs_reactions_supersede_previous_version_without_create_if_missing() {
    let root = repo_root();
    let reactions = read(root.join("os-apps/paw-fs/reactions/reactions.toml"));
    let csdl = read(root.join("os-apps/paw-fs/specs/model.csdl.xml"));

    assert!(
        !reactions.contains("CreateIfMissing"),
        "paw-fs reactions should not use CreateIfMissing for file versioning"
    );
    for needle in [
        "name = \"file_version_create_supersedes_previous\"",
        "entity_type = \"FileVersion\"",
        "action = \"Create\"",
        "field = \"previous_version_id\"",
        "field = \"workspace_id\"",
    ] {
        assert!(
            reactions.contains(needle),
            "reactions should contain {needle}"
        );
    }

    for needle in [
        "<Property Name=\"LastVersionId\" Type=\"Edm.Guid\"/>",
        "<NavigationProperty Name=\"LastVersion\" Type=\"Paw.FS.FileVersion\">",
        "<Property Name=\"MimeType\" Type=\"Edm.String\" Nullable=\"false\"/>",
        "<Property Name=\"PreviousVersionId\" Type=\"Edm.Guid\"/>",
        "<NavigationProperty Name=\"PreviousVersion\" Type=\"Paw.FS.FileVersion\">",
        "<Parameter Name=\"version_number\" Type=\"Edm.Int32\" Nullable=\"false\"/>",
    ] {
        assert!(csdl.contains(needle), "CSDL should contain {needle}");
    }
}
