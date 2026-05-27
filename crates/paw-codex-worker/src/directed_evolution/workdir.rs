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
        ("promoter", "Promotion") => {
            directed_evolution_promotion_context(client, config, &work_item.target_entity_id).await
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

async fn directed_evolution_promotion_context(
    client: &reqwest::Client,
    config: &Config,
    promotion_id: &str,
) -> Result<Option<DirectedEvolutionWorkContext>> {
    let promotion =
        fetch_directed_evolution_entity_fields(client, config, "Promotions", promotion_id).await?;
    let winning_variant_id =
        value_field_string(&promotion, &["WinningVariantId", "winning_variant_id"]);
    let promotion_app_ref = value_field_string(&promotion, &["AppRef", "app_ref"]);
    if winning_variant_id.trim().is_empty() {
        if promotion_app_ref.trim().is_empty() {
            return Ok(None);
        }
        return Ok(Some(DirectedEvolutionWorkContext {
            organism_id: String::new(),
            app_ref: promotion_app_ref,
            branch_ref: String::new(),
        }));
    }

    let variant =
        fetch_directed_evolution_entity_fields(client, config, "Variants", &winning_variant_id)
            .await?;
    let variant_app_ref = value_field_string(&variant, &["AppRef", "app_ref"]);
    let branch_ref = value_field_string(&variant, &["BranchRef", "branch_ref"]);
    let generation_id = value_field_string(&variant, &["GenerationId", "generation_id"]);
    let mut context = if generation_id.trim().is_empty() {
        DirectedEvolutionWorkContext {
            organism_id: String::new(),
            app_ref: promotion_app_ref.clone(),
            branch_ref: String::new(),
        }
    } else {
        directed_evolution_generation_context(client, config, &generation_id)
            .await?
            .unwrap_or(DirectedEvolutionWorkContext {
                organism_id: String::new(),
                app_ref: promotion_app_ref.clone(),
                branch_ref: String::new(),
            })
    };
    if !variant_app_ref.trim().is_empty() {
        context.app_ref = variant_app_ref;
    } else if !promotion_app_ref.trim().is_empty() {
        context.app_ref = promotion_app_ref;
    }
    if !branch_ref.trim().is_empty() {
        context.branch_ref = branch_ref;
    }
    Ok(Some(context))
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
    client: &reqwest::Client,
    config: &Config,
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
    if let Some(runtime) =
        hotload_directed_evolution_variant(client, config, work_item, workdir, app_name, &commit)
            .await?
    {
        apply_directed_evolution_variant_runtime(work_item, &mut payload, &runtime);
    }
    Ok(payload)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DirectedEvolutionGenesisApp {
    owner: String,
    name: String,
    hash: String,
}

impl DirectedEvolutionGenesisApp {
    fn app_id(&self) -> String {
        format!(
            "app-{}-{}",
            sanitize_id_component(&self.owner),
            sanitize_id_component(&self.name)
        )
    }

    fn pinned_ref(&self) -> String {
        format!("{}/{}@{}", self.owner, self.name, self.hash)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DirectedEvolutionVariantRuntime {
    app_ref: String,
    tenant: String,
    runtime_ref: String,
}

fn directed_evolution_genesis_app_from_ref(app_ref: &str) -> Result<DirectedEvolutionGenesisApp> {
    let (path, hash) = app_ref
        .split_once('@')
        .filter(|(_, hash)| !hash.trim().is_empty())
        .with_context(|| format!("Directed Evolution app_ref '{app_ref}' must be owner/name@hash"))?;
    let (owner, name) = path
        .split_once('/')
        .filter(|(owner, name)| !owner.trim().is_empty() && !name.trim().is_empty())
        .with_context(|| format!("Directed Evolution app_ref '{app_ref}' must be owner/name@hash"))?;
    Ok(DirectedEvolutionGenesisApp {
        owner: owner.to_string(),
        name: name.to_string(),
        hash: hash.trim().to_string(),
    })
}

fn directed_evolution_variant_tenant(prefix: &str, work_item: &DirectedEvolutionWorkItemState) -> String {
    let prefix = sanitize_id_component(prefix);
    let suffix = sanitize_id_component(&work_item.id);
    format!("{prefix}-{suffix}")
}

fn apply_directed_evolution_variant_runtime(
    _work_item: &DirectedEvolutionWorkItemState,
    payload: &mut Value,
    runtime: &DirectedEvolutionVariantRuntime,
) {
    if let Some(object) = payload.as_object_mut() {
        object.insert("app_ref".to_string(), json!(runtime.app_ref));
        object.insert("variant_tenant".to_string(), json!(runtime.tenant));
        object.insert("runtime_ref".to_string(), json!(runtime.runtime_ref));
        object.insert("hot_loaded".to_string(), json!(true));
    }
}

async fn hotload_directed_evolution_variant(
    client: &reqwest::Client,
    config: &Config,
    work_item: &DirectedEvolutionWorkItemState,
    workdir: &DirectedEvolutionWorkdir,
    app_name: &str,
    commit: &str,
) -> Result<Option<DirectedEvolutionVariantRuntime>> {
    let Some(genesis_url) = directed_evolution_genesis_url() else {
        return Ok(None);
    };
    if env_flag("PAW_DE_HOTLOAD_VARIANTS") == Some(false) {
        return Ok(None);
    }
    let app_ref = format!("{app_name}@{commit}");
    let app = directed_evolution_genesis_app_from_ref(&app_ref)?;
    let branch = if workdir.branch_ref.trim().is_empty() {
        directed_evolution_variant_branch(work_item)
    } else {
        workdir.branch_ref.clone()
    };
    let registry_tenant = directed_evolution_registry_tenant(config);
    push_directed_evolution_variant_commit(&workdir.path, &genesis_url, &app, &branch, &registry_tenant)
        .await?;
    let tenant_prefix = env::var("DIRECTED_EVOLUTION_VARIANT_TENANT_PREFIX")
        .unwrap_or_else(|_| "de-variant".to_string());
    let tenant = directed_evolution_variant_tenant(&tenant_prefix, work_item);
    install_directed_evolution_variant(client, config, &genesis_url, &app, &tenant, &registry_tenant)
        .await?;
    let pinned_ref = app.pinned_ref();
    Ok(Some(DirectedEvolutionVariantRuntime {
        runtime_ref: format!("temper://tenant/{tenant}/app/{pinned_ref}"),
        app_ref: pinned_ref,
        tenant,
    }))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DirectedEvolutionPromotionMaterialization {
    canonical_app_ref: String,
    production_tenant: String,
    runtime_ref: String,
    summary: String,
    digest: String,
}

async fn materialize_directed_evolution_promotion(
    client: &reqwest::Client,
    config: &Config,
    work_item: &DirectedEvolutionWorkItemState,
) -> Result<DirectedEvolutionPromotionMaterialization> {
    let genesis_url = directed_evolution_genesis_url()
        .context("DIRECTED_EVOLUTION_GENESIS_URL or GENESIS_URL is required for promoter WorkItems")?;
    let promotion_fields = fetch_directed_evolution_entity_fields(
        client,
        config,
        "Promotions",
        &work_item.target_entity_id,
    )
    .await?;
    let app_ref = value_field_string(&promotion_fields, &["AppRef", "app_ref"]);
    let app = directed_evolution_genesis_app_from_ref(&app_ref)?;
    let workdir = resolve_directed_evolution_workdir(client, config, work_item).await?;
    let head = git_capture(&workdir.path, &["rev-parse", "HEAD"]).await?;
    let head = head.trim();
    if head != app.hash {
        bail!(
            "Directed Evolution promoter expected worktree {} at {}, found {}",
            workdir.path.display(),
            app.hash,
            head
        );
    }

    let registry_tenant = directed_evolution_registry_tenant(config);
    push_directed_evolution_canonical_commit(&workdir.path, &genesis_url, &app, &registry_tenant)
        .await?;
    publish_directed_evolution_app_version(client, config, &genesis_url, &app, &registry_tenant)
        .await?;
    let production_tenant = directed_evolution_production_tenant();
    install_directed_evolution_variant(
        client,
        config,
        &genesis_url,
        &app,
        &production_tenant,
        &registry_tenant,
    )
    .await?;

    let canonical_app_ref = app.pinned_ref();
    Ok(DirectedEvolutionPromotionMaterialization {
        runtime_ref: format!("temper://tenant/{production_tenant}/app/{canonical_app_ref}"),
        summary: format!(
            "Published {} to Genesis main and hot-loaded it into tenant {}.",
            canonical_app_ref, production_tenant
        ),
        digest: app.hash.clone(),
        canonical_app_ref,
        production_tenant,
    })
}

fn directed_evolution_promotion_output(
    materialization: &DirectedEvolutionPromotionMaterialization,
) -> Value {
    json!({
        "status": "succeeded",
        "canonical_app_ref": materialization.canonical_app_ref,
        "app_ref": materialization.canonical_app_ref,
        "production_tenant": materialization.production_tenant,
        "runtime_ref": materialization.runtime_ref,
        "summary": materialization.summary,
        "digest": materialization.digest,
        "evidence_refs": [materialization.runtime_ref],
        "reasoning_summary": "The promoter materialized the selector-owned winning variant into the canonical Genesis runtime.",
    })
}

fn directed_evolution_genesis_url() -> Option<String> {
    env::var("DIRECTED_EVOLUTION_GENESIS_URL")
        .ok()
        .or_else(|| env::var("GENESIS_URL").ok())
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
}

async fn push_directed_evolution_variant_commit(
    workdir: &Path,
    genesis_url: &str,
    app: &DirectedEvolutionGenesisApp,
    branch: &str,
    registry_tenant: &str,
) -> Result<()> {
    git_capture_owned(
        workdir,
        directed_evolution_variant_push_args(genesis_url, app, branch, registry_tenant),
    )
    .await?;
    Ok(())
}

async fn push_directed_evolution_canonical_commit(
    workdir: &Path,
    genesis_url: &str,
    app: &DirectedEvolutionGenesisApp,
    registry_tenant: &str,
) -> Result<()> {
    git_capture_owned(
        workdir,
        directed_evolution_canonical_push_args(genesis_url, app, registry_tenant),
    )
    .await?;
    Ok(())
}

fn directed_evolution_canonical_push_args(
    genesis_url: &str,
    app: &DirectedEvolutionGenesisApp,
    registry_tenant: &str,
) -> Vec<String> {
    directed_evolution_variant_push_args(genesis_url, app, "main", registry_tenant)
}

fn directed_evolution_variant_push_args(
    genesis_url: &str,
    app: &DirectedEvolutionGenesisApp,
    branch: &str,
    registry_tenant: &str,
) -> Vec<String> {
    let remote = format!(
        "{}/{}/{}.git",
        genesis_url.trim_end_matches('/'),
        app.owner,
        app.name
    );
    let refspec = format!("HEAD:refs/heads/{}", sanitize_git_branch(branch));
    vec![
        "-c".to_string(),
        format!("http.extraHeader=x-tenant-id: {registry_tenant}"),
        "push".to_string(),
        remote,
        refspec,
    ]
}

async fn install_directed_evolution_variant(
    client: &reqwest::Client,
    config: &Config,
    genesis_url: &str,
    app: &DirectedEvolutionGenesisApp,
    target_tenant: &str,
    registry_tenant: &str,
) -> Result<()> {
    let url = format!(
        "{}/tdata/Apps('{}')/App.Install?await_integration=true",
        genesis_url.trim_end_matches('/'),
        app.app_id().replace('\'', "''")
    );
    let response = client
        .post(&url)
        .headers(headers_for_tenant(config, registry_tenant)?)
        .json(&json!({
            "TargetTenant": target_tenant,
            "AppRef": app.pinned_ref(),
            "Installer": config.worker_id,
            "RegistryUrl": genesis_url.trim_end_matches('/'),
            "RegistryTenant": registry_tenant,
        }))
        .send()
        .await
        .with_context(|| format!("install Directed Evolution variant {}", app.pinned_ref()))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!(
            "Directed Evolution variant install failed for {} into tenant {}: HTTP {} body={}",
            app.pinned_ref(),
            target_tenant,
            status,
            body
        );
    }
    Ok(())
}

async fn publish_directed_evolution_app_version(
    client: &reqwest::Client,
    config: &Config,
    genesis_url: &str,
    app: &DirectedEvolutionGenesisApp,
    registry_tenant: &str,
) -> Result<()> {
    let url = format!(
        "{}/tdata/Apps('{}')/App.PublishNewVersion?await_integration=true",
        genesis_url.trim_end_matches('/'),
        app.app_id().replace('\'', "''")
    );
    let response = client
        .post(&url)
        .headers(headers_for_tenant(config, registry_tenant)?)
        .json(&json!({
            "NewHash": app.hash,
            "RefName": "main",
        }))
        .send()
        .await
        .with_context(|| format!("publish Directed Evolution app version {}", app.pinned_ref()))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!(
            "Directed Evolution app publish failed for {}: HTTP {} body={}",
            app.pinned_ref(),
            status,
            body
        );
    }
    Ok(())
}

fn directed_evolution_registry_tenant(config: &Config) -> String {
    env::var("DIRECTED_EVOLUTION_REGISTRY_TENANT").unwrap_or_else(|_| config.tenant.clone())
}

fn directed_evolution_production_tenant() -> String {
    env::var("DIRECTED_EVOLUTION_PRODUCTION_TENANT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "default".to_string())
}

fn headers_for_tenant(config: &Config, tenant: &str) -> Result<HeaderMap> {
    let mut request_headers = headers(config)?;
    request_headers.insert(
        "x-tenant-id",
        HeaderValue::from_str(tenant).context("invalid Directed Evolution registry tenant")?,
    );
    request_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    request_headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    Ok(request_headers)
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

fn sanitize_id_component(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "item".to_string()
    } else {
        trimmed
    }
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
