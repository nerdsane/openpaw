#[test]
fn codex_plan_args_use_read_only_sandbox_without_bypass() {
    let args = codex_plan_args(Path::new("/tmp/paw-worktree"), "Plan the fix");
    let args = args
        .iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        args,
        vec![
            "exec",
            "--ignore-user-config",
            "--ephemeral",
            "--sandbox",
            "read-only",
            "--cd",
            "/tmp/paw-worktree",
            "--skip-git-repo-check",
            "Plan the fix"
        ]
    );
    assert!(!args.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
}

#[test]
fn implementation_prompt_carries_active_workcycle_plan() {
    let prompt = implementation_prompt_with_plan(
        "Fix the Discord typing indicator.",
        "## Context\nTyping indicator vanishes.\n\n## Verification Plan\nRun the DM smoke.",
    );

    assert!(prompt.contains("Fix the Discord typing indicator."));
    assert!(prompt.contains("<active_workcycle_plan>"));
    assert!(prompt.contains("## Verification Plan"));
    assert!(prompt.contains("</active_workcycle_plan>"));
}
