use std::{fs, path::PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("temperpaw crate should live under crates/temperpaw")
        .to_path_buf()
}

fn repo_file(path: &str) -> String {
    let root = repo_root();
    fs::read_to_string(root.join(path)).unwrap_or_else(|err| panic!("read {path}: {err}"))
}

#[test]
fn docker_and_ci_verify_bundled_route_message_wasm() {
    let dockerfile = repo_file("Dockerfile");
    assert!(
        dockerfile.contains("scripts/verify_route_message_wasm.sh"),
        "Docker image builds must verify the bundled route_message.wasm before packaging os-apps"
    );

    let ci = repo_file(".github/workflows/ci.yml");
    assert!(
        ci.contains("scripts/verify_route_message_wasm.sh"),
        "CI WASM builds must run the route_message.wasm verifier"
    );
}

#[test]
fn route_message_wasm_verifier_rejects_unbounded_session_entry_lookup() {
    let script = repo_file("scripts/verify_route_message_wasm.sh");
    assert!(
        script.contains("route_message.wasm"),
        "verifier must inspect the packaged route_message.wasm artifact"
    );
    assert!(
        script.contains("$orderby") && script.contains("Sequence desc"),
        "verifier must reject the production-failing ordered SessionEntries lookup"
    );
    assert!(
        script.contains("SessionEntries"),
        "verifier must specifically guard the SessionEntries route_message lookup"
    );
    assert!(
        script.contains("sha256sum") || script.contains("shasum"),
        "verifier must print the packaged route_message.wasm hash for deploy evidence"
    );
}
