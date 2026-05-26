async fn resolve_directed_evolution_workdir(
    client: &reqwest::Client,
    config: &Config,
    work_item: &DirectedEvolutionWorkItemState,
) -> Result<DirectedEvolutionWorkdir> {
    let Some(context) =
        directed_evolution_work_item_context(client, config, work_item).await?
    else {
        return Ok(DirectedEvolutionWorkdir {
            path: config.repo_root.clone(),
            app_ref: String::new(),
            branch_ref: String::new(),
        });
    };
    if let Some(path) =
        directed_evolution_repo_from_mapping(&context.organism_id, &context.app_ref)
    {
        if directed_evolution_role_may_write_repo(&work_item.role) {
            let branch_ref = directed_evolution_variant_branch(work_item);
            let worktree =
                ensure_directed_evolution_worktree(config, &path, &branch_ref, true).await?;
            return Ok(DirectedEvolutionWorkdir {
                path: worktree,
                app_ref: context.app_ref,
                branch_ref,
            });
        }
        if !context.branch_ref.trim().is_empty() {
            let worktree =
                ensure_directed_evolution_worktree(config, &path, &context.branch_ref, false)
                    .await?;
            return Ok(DirectedEvolutionWorkdir {
                path: worktree,
                app_ref: context.app_ref,
                branch_ref: context.branch_ref,
            });
        }
        return Ok(DirectedEvolutionWorkdir {
            path,
            app_ref: context.app_ref,
            branch_ref: context.branch_ref,
        });
    }
    if directed_evolution_role_may_write_repo(&work_item.role)
        && env_flag("PAW_DE_ALLOW_GLOBAL_REPO_FOR_VARIANTS") != Some(true)
    {
        bail!(
            "Directed Evolution {} WorkItem {} resolved app_ref '{}' for organism '{}' but no repository mapping was configured. Set DIRECTED_EVOLUTION_ORGANISM_REPOS_JSON or PAW_DE_ALLOW_GLOBAL_REPO_FOR_VARIANTS=true.",
            work_item.role,
            work_item.id,
            context.app_ref,
            context.organism_id
        );
    }
    Ok(DirectedEvolutionWorkdir {
        path: config.repo_root.clone(),
        app_ref: context.app_ref,
        branch_ref: context.branch_ref,
    })
}

#[derive(Clone, Debug)]
struct DirectedEvolutionWorkdir {
    path: PathBuf,
    app_ref: String,
    branch_ref: String,
}

#[derive(Clone, Debug)]
struct DirectedEvolutionWorkContext {
    organism_id: String,
    app_ref: String,
    branch_ref: String,
}

async fn directed_evolution_work_item_context(
    client: &reqwest::Client,
    config: &Config,
    work_item: &DirectedEvolutionWorkItemState,
) -> Result<Option<DirectedEvolutionWorkContext>> {
    match (work_item.role.as_str(), work_item.target_entity_type.as_str()) {
        ("variant_generator", "Generation") | ("selector", "Generation") => {
            directed_evolution_generation_context(client, config, &work_item.target_entity_id).await
        }
        ("reviewer", "StageResult") | ("simulated_user", "StageResult") => {
            let stage_result =
                fetch_directed_evolution_entity_fields(client, config, "StageResults", &work_item.target_entity_id)
                    .await?;
            let variant_id = value_field_string(&stage_result, &["VariantId", "variant_id"]);
            if variant_id.trim().is_empty() {
                return Ok(None);
            }
            let variant =
                fetch_directed_evolution_entity_fields(client, config, "Variants", &variant_id).await?;
            let app_ref = value_field_string(&variant, &["AppRef", "app_ref"]);
            let generation_id = value_field_string(&variant, &["GenerationId", "generation_id"]);
            let mut context = if generation_id.trim().is_empty() {
                None
            } else {
                directed_evolution_generation_context(client, config, &generation_id).await?
            };
            if let Some(context) = context.as_mut() {
                if !app_ref.trim().is_empty() {
                    context.app_ref = app_ref;
                }
                context.branch_ref = value_field_string(&variant, &["BranchRef", "branch_ref"]);
            }
            Ok(context)
        }
        _ => Ok(None),
    }
}

async fn directed_evolution_generation_context(
    client: &reqwest::Client,
    config: &Config,
    generation_id: &str,
) -> Result<Option<DirectedEvolutionWorkContext>> {
    let generation =
        fetch_directed_evolution_entity_fields(client, config, "Generations", generation_id).await?;
    let parent_version_id = value_field_string(&generation, &["ParentVersionId", "parent_version_id"]);
    if parent_version_id.trim().is_empty() {
        return Ok(None);
    }
    let parent =
        fetch_directed_evolution_entity_fields(client, config, "OrganismVersions", &parent_version_id)
            .await?;
    let organism_id = value_field_string(&parent, &["OrganismId", "organism_id"]);
    let app_ref = value_field_string(&parent, &["AppRef", "app_ref"]);
    if organism_id.trim().is_empty() && app_ref.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(DirectedEvolutionWorkContext {
        organism_id,
        app_ref,
        branch_ref: String::new(),
    }))
}

async fn fetch_directed_evolution_entity_fields(
    client: &reqwest::Client,
    config: &Config,
    entity_set: &str,
    entity_id: &str,
) -> Result<Value> {
    let response = client
        .get(config.entity_url(entity_set, entity_id))
        .headers(headers(config)?)
        .send()
        .await
        .with_context(|| format!("fetch Directed Evolution {entity_set}('{entity_id}')"))?;
    if !response.status().is_success() {
        bail!(
            "fetch Directed Evolution {}('{}') returned {}",
            entity_set,
            entity_id,
            response.status()
        );
    }
    let body: Value = response.json().await.context("parse Directed Evolution entity")?;
    Ok(body.get("fields").cloned().unwrap_or_else(|| json!({})))
}

fn directed_evolution_repo_from_mapping(organism_id: &str, app_ref: &str) -> Option<PathBuf> {
    let raw = env::var("DIRECTED_EVOLUTION_ORGANISM_REPOS_JSON").ok()?;
    let mapping: Value = serde_json::from_str(&raw).ok()?;
    let app_name = app_ref.split('@').next().unwrap_or(app_ref);
    for key in [organism_id, app_ref, app_name] {
        if key.trim().is_empty() {
            continue;
        }
        if let Some(path) = mapping.get(key).and_then(Value::as_str) {
            return Some(PathBuf::from(path));
        }
    }
    None
}

fn directed_evolution_role_may_write_repo(role: &str) -> bool {
    matches!(role, "variant_generator")
}

fn directed_evolution_variant_branch(work_item: &DirectedEvolutionWorkItemState) -> String {
    format!("directed-evolution/{}", sanitize_git_ref_component(&work_item.id))
}

async fn ensure_directed_evolution_worktree(
    config: &Config,
    base_repo: &Path,
    branch_ref: &str,
    reset_branch: bool,
) -> Result<PathBuf> {
    let branch = sanitize_git_branch(branch_ref);
    let worktree = config
        .workspace_root
        .join("directed-evolution")
        .join(sanitize_path_component(&branch));
    if worktree.join(".git").exists() {
        return Ok(worktree);
    }
    fs::create_dir_all(worktree.parent().unwrap_or(&config.workspace_root))
        .with_context(|| format!("create {}", worktree.display()))?;
    let inside = git_capture(base_repo, &["rev-parse", "--is-inside-work-tree"]).await?;
    if inside.trim() != "true" {
        bail!(
            "Directed Evolution organism repo {} is not a git work tree",
            base_repo.display()
        );
    }
    let mut args = vec!["worktree".to_string(), "add".to_string()];
    if reset_branch {
        args.push("-B".to_string());
        args.push(branch.clone());
    }
    args.push(worktree.display().to_string());
    args.push(if reset_branch { "HEAD" } else { &branch }.to_string());
    git_capture_owned(base_repo, args).await?;
    ensure_git_identity(&worktree).await?;
    Ok(worktree)
}

async fn finalize_directed_evolution_output(
    work_item: &DirectedEvolutionWorkItemState,
    workdir: &DirectedEvolutionWorkdir,
    mut payload: Value,
) -> Result<Value> {
    if work_item.role != "variant_generator" {
        return Ok(payload);
    }
    let changed_files = git_changed_files(&workdir.path).await?;
    if changed_files.is_empty() {
        bail!(
            "Directed Evolution variant_generator WorkItem {} produced no git changes in {}",
            work_item.id,
            workdir.path.display()
        );
    }
    git_capture(&workdir.path, &["add", "-A"]).await?;
    git_capture_owned(
        &workdir.path,
        vec![
            "commit".to_string(),
            "-m".to_string(),
            format!("Directed Evolution variant {}", work_item.id),
        ],
    )
    .await?;
    let commit = git_capture(&workdir.path, &["rev-parse", "HEAD"]).await?;
    let app_name = workdir
        .app_ref
        .split('@')
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("organism");

    if let Some(object) = payload.as_object_mut() {
        object.insert("app_ref".to_string(), json!(format!("{app_name}@{commit}")));
        object.insert("branch_ref".to_string(), json!(workdir.branch_ref));
        object.insert(
            "runtime_ref".to_string(),
            json!(format!("local-worktree:{}", workdir.path.display())),
        );
        object.insert("changed_files".to_string(), json!(changed_files));
        object.insert(
            "diff_ref".to_string(),
            json!(format!("git:{}:{commit}", workdir.branch_ref)),
        );
    }
    Ok(payload)
}

async fn git_changed_files(workdir: &Path) -> Result<Vec<String>> {
    let status = git_capture(workdir, &["status", "--porcelain"]).await?;
    let mut files = changed_paths_from_status(&status);
    files.sort();
    files.dedup();
    Ok(files)
}

async fn directed_evolution_git_status_snapshot(workdir: &Path) -> Result<Option<String>> {
    let inside = match git_capture(workdir, &["rev-parse", "--is-inside-work-tree"]).await {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    if inside.trim() != "true" {
        return Ok(None);
    }
    git_capture(workdir, &["status", "--porcelain"])
        .await
        .map(Some)
}

async fn ensure_directed_evolution_readonly_workdir_unchanged(
    workdir: &Path,
    status_before: &str,
) -> Result<()> {
    let status_after = git_capture(workdir, &["status", "--porcelain"]).await?;
    if status_after != status_before {
        bail!(
            "Directed Evolution read-only brain role modified {}. Before status: {:?}. After status: {:?}",
            workdir.display(),
            status_before,
            status_after
        );
    }
    Ok(())
}

fn sanitize_git_branch(value: &str) -> String {
    let sanitized = value
        .split('/')
        .map(sanitize_git_ref_component)
        .collect::<Vec<_>>()
        .join("/");
    if sanitized.trim().is_empty() {
        "directed-evolution/variant".to_string()
    } else {
        sanitized
    }
}

fn sanitize_git_ref_component(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(['.', '-'])
        .to_string();
    if cleaned.is_empty() {
        "variant".to_string()
    } else {
        cleaned
    }
}

fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn value_field_string(fields: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| fields.get(*key).and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn env_flag(key: &str) -> Option<bool> {
    env::var(key).ok().map(|value| {
        value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
    })
}

fn parse_codex_jsonish(raw: &str) -> Option<Value> {
    serde_json::from_str::<Value>(raw).ok().or_else(|| {
        let start = raw.find('{')?;
        let end = raw.rfind('}')?;
        if end <= start {
            return None;
        }
        serde_json::from_str::<Value>(&raw[start..=end]).ok()
    })
}
