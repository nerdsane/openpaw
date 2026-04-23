use temper_wasm_sdk::prelude::*;

/// Build the user_message prompt based on job_type, then spawn a Session entity
/// and dispatch Configure on it with the constructed message and session params.
///
/// Reads `odata_namespace` from [integration.config] to construct OData action
/// URLs. Defaults to "WikiCore" if not configured. Consuming apps override this
/// in their own .ioa.toml integration config.
///
/// If the job's `input` JSON contains a `mission_template` key, that template is
/// used instead of the built-in default templates. The shared operating model
/// (workspace rules, tooling rules) is always prepended.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "build_session_message: starting");

        // --- Read WikiJob entity fields ---
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let job_type = fields
            .get("job_type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let input = fields
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or("{}")
            .to_string();

        let scope_id = fields
            .get("scope_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let soul_id = fields
            .get("soul_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let model = required_field(&fields, &["model", "Model"], "WikiJob.model")?;
        let provider = required_field(&fields, &["provider", "Provider"], "WikiJob.provider")?;

        let temperature = fields
            .get("temperature")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0")
            .to_string();

        let tools_enabled = fields
            .get("tools_enabled")
            .and_then(|v| v.as_str())
            .unwrap_or("temper_get,temper_list,temper_create,temper_action,temper_write,temper_read,temper_web_search,temper_web_fetch")
            .to_string();
        let tools_enabled = sanitize_tools_enabled(&tools_enabled);

        let max_turns = fields
            .get("max_turns")
            .and_then(|v| v.as_str())
            .unwrap_or("250")
            .to_string();

        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&ctx.entity_id)
            .to_string();

        // --- Config ---
        let api_url = ctx
            .config
            .get("temper_api_url")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());

        let odata_namespace = ctx
            .config
            .get("odata_namespace")
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| "WikiCore".to_string());

        let tenant = &ctx.tenant;

        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Tenant-Id".to_string(), tenant.to_string()),
            ("x-temper-principal-kind".to_string(), "agent".to_string()),
            ("x-temper-principal-id".to_string(), "system".to_string()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // --- Build user_message based on job_type ---
        let existing_workspace_id = fields
            .get("workspace_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let workspace_name = shared_workspace_name(&scope_id);
        let workspace_id = if existing_workspace_id.is_empty() {
            ensure_workspace(&ctx, &api_url, tenant, &headers, &workspace_name)?
        } else {
            existing_workspace_id
        };
        let workspace_label = if scope_id.is_empty() {
            format!("attached workspace ({workspace_id})")
        } else {
            workspace_name.clone()
        };

        // Check for custom mission_template in input JSON
        let parsed_input = serde_json::from_str::<serde_json::Value>(&input).ok();
        let custom_template = parsed_input
            .as_ref()
            .and_then(|v| v.get("mission_template"))
            .and_then(|v| v.as_str());

        let user_message = if let Some(template) = custom_template {
            build_custom_message(
                template,
                &scope_id,
                &entity_id,
                &workspace_id,
                &workspace_label,
                &input,
                &job_type,
            )
        } else {
            match job_type.as_str() {
                "source_search" => build_source_search_message(
                    &input,
                    &scope_id,
                    &entity_id,
                    &workspace_id,
                    &workspace_label,
                ),
                "synthesize" => build_synthesize_message(
                    &fields,
                    &input,
                    &scope_id,
                    &entity_id,
                    &workspace_id,
                    &workspace_label,
                ),
                other => {
                    return Err(format!(
                        "build_session_message: unsupported job_type '{other}' (provide a mission_template in input JSON for custom job types)"
                    ));
                }
            }
        };

        ctx.log(
            "info",
            &format!(
                "build_session_message: built user_message for job_type='{}' ({} chars)",
                job_type,
                user_message.len()
            ),
        );

        // --- Create Session entity ---
        let session_body = json!({
            "fields": {}
        });

        let create_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Sessions"),
            &headers,
            &session_body.to_string(),
        )?;
        if create_resp.status < 200 || create_resp.status >= 300 {
            return Err(format!(
                "Failed to create Session: HTTP {}: {}",
                create_resp.status,
                &create_resp.body[..create_resp.body.len().min(500)]
            ));
        }

        let created: serde_json::Value = serde_json::from_str(&create_resp.body)
            .map_err(|e| format!("Failed to parse Session creation response: {e}"))?;

        let session_id = created
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or("Created Session has no entity_id")?
            .to_string();

        ctx.log(
            "info",
            &format!("build_session_message: created Session '{session_id}'"),
        );

        // --- Dispatch Configure on the Session ---
        let mut config_body = json!({
            "user_message": user_message,
            "model": model,
            "provider": provider,
            "temperature": temperature,
            "tools_enabled": tools_enabled,
            "max_turns": max_turns,
            "workspace_id": workspace_id,
        });
        if !soul_id.is_empty() {
            config_body["soul_id"] = json!(soul_id);
        }

        let configure_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Sessions('{session_id}')/TemperPaw.Configure"),
            &headers,
            &config_body.to_string(),
        )?;
        if configure_resp.status < 200 || configure_resp.status >= 300 {
            return Err(format!(
                "Failed to Configure Session: HTTP {}: {}",
                configure_resp.status,
                &configure_resp.body[..configure_resp.body.len().min(500)]
            ));
        }

        ctx.log(
            "info",
            &format!("build_session_message: dispatched Configure on Session '{session_id}'"),
        );

        // --- Dispatch SessionSpawned on the WikiJob ---
        let spawned_body = json!({
            "session_id": session_id,
            "workspace_id": workspace_id,
        });

        let spawned_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/WikiJobs('{entity_id}')/{odata_namespace}.SessionSpawned"),
            &headers,
            &spawned_body.to_string(),
        )?;
        if spawned_resp.status < 200 || spawned_resp.status >= 300 {
            return Err(format!(
                "Failed to dispatch SessionSpawned: HTTP {}: {}",
                spawned_resp.status,
                &spawned_resp.body[..spawned_resp.body.len().min(500)]
            ));
        }

        if let Err(link_error) = create_session_link(
            &ctx,
            &api_url,
            &headers,
            &entity_id,
            &odata_namespace,
            &session_id,
        ) {
            let message =
                format!("SessionLink setup failed for child Session '{session_id}': {link_error}");
            if let Err(fail_error) = dispatch_wiki_job_failure(
                &ctx,
                &api_url,
                &headers,
                &entity_id,
                &odata_namespace,
                &message,
            ) {
                return Err(format!(
                    "{message}; additionally failed to mark WikiJob failed: {fail_error}"
                ));
            }
            return Err(message);
        }

        ctx.log("info", "build_session_message: completed successfully");

        set_success_result(
            "",
            &json!({
                "status": "ok",
                "session_id": session_id,
                "job_type": job_type,
            }),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn required_field(fields: &Value, keys: &[&str], name: &str) -> Result<String, String> {
    keys.iter()
        .find_map(|key| fields.get(*key).and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            format!("{name} is required; configure an Agent or pass an explicit override")
        })
}

fn create_session_link(
    ctx: &Context,
    api_url: &str,
    headers: &[(String, String)],
    parent_job_id: &str,
    parent_action_namespace: &str,
    child_session_id: &str,
) -> Result<(), String> {
    let create_resp = ctx.http_call(
        "POST",
        &format!("{api_url}/tdata/SessionLinks"),
        headers,
        "{}",
    )?;
    if create_resp.status < 200 || create_resp.status >= 300 {
        return Err(format!(
            "Failed to create SessionLink: HTTP {}: {}",
            create_resp.status,
            &create_resp.body[..create_resp.body.len().min(500)]
        ));
    }
    let created: Value = serde_json::from_str(&create_resp.body)
        .map_err(|err| format!("Failed to parse SessionLink creation response: {err}"))?;
    let link_id = created
        .get("entity_id")
        .or_else(|| created.get("Id"))
        .and_then(|value| value.as_str())
        .ok_or("Created SessionLink has no entity_id")?;

    let configure_body = json!({
        "ParentEntitySet": "WikiJobs",
        "ParentEntityId": parent_job_id,
        "ParentActionNamespace": parent_action_namespace,
        "ChildSessionId": child_session_id,
        "OnCompletedAction": "Complete",
        "OnFailureAction": "Fail",
        "MaxChecks": "180",
    });
    let configure_resp = ctx.http_call(
        "POST",
        &format!("{api_url}/tdata/SessionLinks('{link_id}')/TemperPaw.Configure"),
        headers,
        &configure_body.to_string(),
    )?;
    if configure_resp.status < 200 || configure_resp.status >= 300 {
        return Err(format!(
            "Failed to configure SessionLink: HTTP {}: {}",
            configure_resp.status,
            &configure_resp.body[..configure_resp.body.len().min(500)]
        ));
    }

    ctx.log(
        "info",
        &format!("build_session_message: linked WikiJob '{parent_job_id}' to Session '{child_session_id}'"),
    );
    Ok(())
}

fn dispatch_wiki_job_failure(
    ctx: &Context,
    api_url: &str,
    headers: &[(String, String)],
    wiki_job_id: &str,
    odata_namespace: &str,
    error_message: &str,
) -> Result<(), String> {
    let body = json!({
        "error_message": error_message,
    });
    let response = ctx.http_call(
        "POST",
        &format!("{api_url}/tdata/WikiJobs('{wiki_job_id}')/{odata_namespace}.Fail"),
        headers,
        &body.to_string(),
    )?;
    if response.status < 200 || response.status >= 300 {
        return Err(format!(
            "Failed to dispatch WikiJob.Fail after SessionLink setup failure: HTTP {}: {}",
            response.status,
            &response.body[..response.body.len().min(500)]
        ));
    }
    Ok(())
}

/// Shared operating model prepended to all mission templates.
fn operating_model(
    scope_id: &str,
    job_id: &str,
    workspace_id: &str,
    workspace_label: &str,
    job_type: &str,
) -> String {
    format!(
        r#"You are executing a WikiJob ({job_type}) for scope '{scope_id}'.
Job ID: {job_id}
Workspace ID: {workspace_id}
Shared workspace: {workspace_label}

## Operating Model

- This session already has a shared wiki workspace attached. Use `temper.read()` and `temper.write()` against that workspace by default.
- The Monty REPL is persistent within this session. Python variables, helper functions, and working sets survive across `execute` calls.
- Persist durable artifacts to the workspace and Temper entities. Use files/entities for sources, published pages, checkpoints you would want after a crash, and anything another job should read later.
- Do not use `bash` or `sandbox.*` here. Stay inside Temper tools only.
- Do not print progress to stdout/stderr. Report externally-visible progress through `RecordProgress`.

## Tooling Rules

- No `import` statements
- No `enumerate(..., start=...)`
- Available tools include `temper.web_search(query)`, `temper.web_fetch(url)`, `temper.write(path, content)`, `temper.read(path)`, `temper.list(...)`, `temper.get(...)`, `temper.create(...)`, `temper.action(...)`
- Keep code focused and incremental. Use multiple small execute calls instead of a giant framework.
- When you build JSON payloads, always serialize with `json.dumps(...)`. Do not use `str(dict)` and do not hand-format JSON-like text.
- `temper.web_fetch(url)` returns a structured object. Read the body with `fetched.get("text", "")` (or `fetched.get("content", "")` as fallback). Do not assume it returns a raw string.
- Treat the following runtime constants as authoritative. Do not rediscover them:
  - `job_id = "{job_id}"`
  - `scope_id = "{scope_id}"`
  - `workspace_id = "{workspace_id}"`
"#
    )
}

/// Build a message from a custom mission_template with variable substitution.
fn build_custom_message(
    template: &str,
    scope_id: &str,
    job_id: &str,
    workspace_id: &str,
    workspace_label: &str,
    input: &str,
    job_type: &str,
) -> String {
    let header = operating_model(scope_id, job_id, workspace_id, workspace_label, job_type);
    let expanded = template
        .replace("{{scope_id}}", scope_id)
        .replace("{{job_id}}", job_id)
        .replace("{{workspace_id}}", workspace_id)
        .replace("{{workspace_label}}", workspace_label)
        .replace("{{input}}", input)
        .replace("{{job_type}}", job_type);
    format!("{header}\n## Mission\n\n{expanded}")
}

/// Build the user_message for a source_search job.
fn build_source_search_message(
    input: &str,
    scope_id: &str,
    job_id: &str,
    workspace_id: &str,
    workspace_label: &str,
) -> String {
    let header = operating_model(
        scope_id,
        job_id,
        workspace_id,
        workspace_label,
        "source_search",
    );
    format!(
        r#"{header}
## Orient First

1. Read `/wiki/SCHEMA.md`, `/wiki/index.md`, and `/wiki/log.md` if they exist.
2. If `/wiki/SCHEMA.md` is missing, create a compact one that covers:
   - domain and scope focus for `{scope_id}`
   - frontmatter requirements for pages
   - tag taxonomy
   - page quality thresholds
3. Build on the existing wiki instead of recreating it.

## Search Scope

{input}

## Mission

Research the requested scope, fetch the strongest in-scope sources, store them in the shared workspace, create `WikiSource` entities for them, and then complete this job with a structured output payload. The app will create the follow-up `synthesize` job deterministically after this job completes.

## Exact Entity Shapes

- To register a source, always use this two-step pattern:
  ```
  src = temper.create('WikiSources', {{}})
  temper.action('WikiSources', src['entity_id'], 'Submit', {{
      'scope_id': scope_id,
      'title': title,
      'source_type': source_type,
      'source_url': url,
      'file_id': file_id,
      'metadata': metadata_json
  }})
  temper.action('WikiSources', src['entity_id'], 'Index', {{
      'extracted_topics': topics_json,
      'derived_page_ids': '[]'
  }})
  ```
- `scope_id` is required on `WikiSource.Submit` even when it is empty. Pass it through exactly as provided.
- `metadata`, `extracted_topics`, and `derived_page_ids` are JSON text fields. Pass JSON strings, not Python dict/list objects when possible.
- If you need to fail the job, `WikiJobs.Fail` expects the `error_message` field:
  ```
  temper.action('WikiJobs', job_id, 'Fail', {{
      'error_message': reason_text
  }})
  temper.done("source_search failed")
  ```
- If you report progress, `RecordProgress` expects:
  ```
  progress_json = json.dumps(progress_obj, ensure_ascii=False)
  temper.action('WikiJobs', job_id, 'RecordProgress', {{
      'progress_log': progress_json
  }})
  ```
- To complete this job successfully, dispatch:
  ```
  output_json = json.dumps(output_obj, ensure_ascii=False)
  temper.action('WikiJobs', job_id, 'Complete', {{
      'output': output_json
  }})
  temper.done("source_search complete")
  ```
- `output_json` must be a JSON string containing at least:
  - `task`
  - `scope`
  - `source_ids`
  - `topic_allowlist`
- Do not create or submit the synth job yourself from source-search. The app will do that after `Complete`.

## Required Flow

1. Search with a small number of focused queries derived from the task.
2. Shortlist high-signal, directly in-scope sources.
3. Fetch and store raw source content at stable workspace paths like `/wiki/sources/<source-slug>.md`.
   Use this exact pattern:
   ```
   fetched = temper.web_fetch(url)
   text = fetched.get("text", "") or fetched.get("content", "")
   if not text:
       continue
   result = temper.write(path, markdown_prefix + text)
   file_id = result["file_id"]
   ```
4. Create one `WikiSource` entity per accepted source and dispatch `Index` with extracted topics.
5. Update `/wiki/log.md` and `/wiki/index.md` so the workspace stays navigable.
6. Optionally dispatch `RecordProgress` with compact JSON-string summaries.
7. Dispatch `Complete` on this `WikiJob` only after at least one concrete `WikiSource` exists and `output_json` contains the accepted source IDs, task, scope, and topic allowlist needed for synthesis.
8. Immediately call `temper.done("source_search complete")` after dispatching `Complete` or `Fail`. Do not keep working after the job reaches a terminal state.

## Source Quality Standards

- Prefer authoritative sources: official docs, academic/technical references, well-maintained guides, or high-quality primary material.
- Target 5-8 strong in-scope sources before synthesis, unless the task is genuinely narrower.
- Reject broad hubs, category pages, SEO filler, and generic adjacent background pieces.
- Source file paths must be unique per URL. Similar titles must not overwrite each other.
- Extract canonical topics from the source content; never use source titles themselves as topic taxonomy.
- If you cannot produce any usable sources, dispatch `Fail` with a concrete reason instead of completing with zero output.
"#
    )
}

/// Build the user_message for a synthesize job.
fn build_synthesize_message(
    fields: &serde_json::Value,
    input: &str,
    scope_id: &str,
    job_id: &str,
    workspace_id: &str,
    workspace_label: &str,
) -> String {
    let header = operating_model(
        scope_id,
        job_id,
        workspace_id,
        workspace_label,
        "synthesize",
    );
    let scope_block = render_synthesis_scope(fields, input);
    format!(
        r#"{header}
## Orient First

1. Read `/wiki/SCHEMA.md`, `/wiki/index.md`, and `/wiki/log.md`.
2. List the indexed `WikiSources` referenced by this job and any already-published `WikiPages` in the shared workspace so you can avoid duplicates and choose merge targets when appropriate.

## Mission

Turn the indexed sources for this job into a clean, navigable wiki. The markdown files in the workspace are the document truth; `WikiPage` entities are the metadata registry for those files.

## Required Scope

{scope_block}

## Scope Discipline

- Stay inside the required scope above.
- Every page title and slug must name a studyable concept, pattern, contrast, or reference page.
- Do not use source titles, website names, or URL fragments as page titles.
- Prefer fewer strong in-scope pages over a larger set with drift.

## Exact Entity Shapes

- To draft a page:
  ```
  page = temper.create('WikiPages', {{}})
  temper.action('WikiPages', page['entity_id'], 'Draft', {{
      'scope_id': scope_id,
      'slug': slug,
      'title': title,
      'category': category,
      'file_id': file_id,
      'summary': summary,
      'cross_references': cross_refs_json,
      'source_ids': source_ids_json,
      'tags': tags_json
  }})
  temper.action('WikiPages', page['entity_id'], 'Publish', {{}})
  ```
- `cross_references`, `source_ids`, and `tags` are JSON text fields. Pass JSON strings, not Python dict/list objects when possible.
- To complete this job successfully, dispatch:
  ```
  output_json = json.dumps(output_obj, ensure_ascii=False)
  temper.action('WikiJobs', job_id, 'Complete', {{ 'output': output_json }})
  temper.done("synthesis complete")
  ```

## Required Flow

1. Inspect indexed `WikiSources` and existing `WikiPages` for this scope.
2. Form a concise page plan for the required scope. Keep it in memory unless you want a durable checkpoint for recovery.
3. Create or revise pages one page at a time:
   - draft markdown
   - write it to `/wiki/pages/<slug>.md`
   - create or revise the `WikiPage` entity
   - publish it
   - update `/wiki/index.md` and `/wiki/log.md`
4. Use `WikiPage` as metadata plus file pointer:
   - `slug`
   - `title`
   - `category`
   - `file_id`
   - `summary`
   - `cross_references`
   - `source_ids`
   - `tags`
5. Dispatch `RecordProgress` with compact summaries when useful.
6. Dispatch `Complete` only after every intended page for this run has been written, published, and indexed.
7. Immediately call `temper.done("synthesis complete")` after dispatching `Complete` or `Fail`. Do not keep working after the job reaches a terminal state.

## Page Standards

- One page per topic; avoid broad catch-all overviews unless the scope explicitly calls for them.
- Each page should synthesize across the available sources for that topic.
- Every page must start with YAML frontmatter:
  ```
  ---
  title: Page Title
  created: YYYY-MM-DD
  updated: YYYY-MM-DD
  type: concept
  tags: [from-schema-taxonomy]
  sources: [source-slug-1, source-slug-2]
  ---
  ```
- Every page should contain meaningful outbound `[[wikilinks]]` to related pages.
- Source attribution belongs in a `Sources` section using `[[source-slug]]` links or equivalent stable references.
- Never let the display title equal the slug.
- Never create a duplicate `WikiPage` for an already-published slug; revise the existing page or choose the next unpublished topic.

## Markdown Expectations

- Use `##` and `###` headings to structure the page.
- Include concrete examples, contrasts, and edge cases where the topic benefits from them.
- Prefer concept pages, contrast pages, and tightly scoped reference pages over source-by-source summaries.
- Keep terminology consistent across the wiki and align with `/wiki/SCHEMA.md`.
"#
    )
}

fn render_synthesis_scope(fields: &serde_json::Value, input: &str) -> String {
    let parsed = serde_json::from_str::<serde_json::Value>(input).ok();
    let task = parsed
        .as_ref()
        .and_then(|value| value.get("task"))
        .and_then(|value| value.as_str())
        .or_else(|| fields.get("task").and_then(|value| value.as_str()))
        .unwrap_or(input);
    let scope = parsed
        .as_ref()
        .and_then(|value| value.get("scope"))
        .and_then(|value| value.as_str())
        .or_else(|| fields.get("scope").and_then(|value| value.as_str()))
        .unwrap_or("");
    let allowlist = parsed
        .as_ref()
        .and_then(|value| value.get("topic_allowlist"))
        .and_then(|value| value.as_array())
        .map(|topics| {
            topics
                .iter()
                .filter_map(|topic| topic.as_str())
                .collect::<Vec<_>>()
        })
        .or_else(|| {
            fields
                .get("topic_allowlist")
                .and_then(|value| value.as_array())
                .map(|topics| {
                    topics
                        .iter()
                        .filter_map(|topic| topic.as_str())
                        .collect::<Vec<_>>()
                })
        })
        .unwrap_or_default();
    let source_ids = parsed
        .as_ref()
        .and_then(|value| value.get("source_ids"))
        .and_then(|value| value.as_array())
        .map(|ids| ids.iter().filter_map(|id| id.as_str()).collect::<Vec<_>>())
        .or_else(|| {
            fields
                .get("source_ids")
                .and_then(|value| value.as_array())
                .map(|ids| ids.iter().filter_map(|id| id.as_str()).collect::<Vec<_>>())
        })
        .unwrap_or_default();

    let mut lines = vec![format!("- Task: {task}")];
    if !scope.is_empty() {
        lines.push(format!("- Scope: {scope}"));
    }
    if !allowlist.is_empty() {
        lines.push(format!("- Topic allowlist: {}", allowlist.join(", ")));
    } else {
        lines.push(
            "- Topic allowlist: derive only from the explicit task above and directly matching indexed sources"
                .to_string(),
        );
    }
    if !source_ids.is_empty() {
        lines.push(format!("- Source IDs: {}", source_ids.join(", ")));
    }
    lines.join("\n")
}

fn sanitize_tools_enabled(raw: &str) -> String {
    let allowed = [
        "temper_get",
        "temper_list",
        "temper_create",
        "temper_action",
        "temper_write",
        "temper_read",
        "temper_web_search",
        "temper_web_fetch",
    ];

    let mut selected: Vec<&str> = Vec::new();
    for tool in raw
        .split(',')
        .map(str::trim)
        .filter(|tool| !tool.is_empty())
    {
        if allowed.contains(&tool) && !selected.contains(&tool) {
            selected.push(tool);
        }
    }

    if selected.is_empty() {
        allowed.join(",")
    } else {
        selected.join(",")
    }
}

fn shared_workspace_name(scope_id: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in scope_id.chars() {
        let lowered = ch.to_ascii_lowercase();
        if lowered.is_ascii_alphanumeric() {
            slug.push(lowered);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "wiki-default".to_string()
    } else {
        format!("wiki-{slug}")
    }
}

fn ensure_workspace(
    ctx: &Context,
    api_url: &str,
    _tenant: &str,
    headers: &[(String, String)],
    name: &str,
) -> Result<String, String> {
    let find_resp = ctx.http_call(
        "GET",
        &format!(
            "{api_url}/tdata/Workspaces?$filter=Name%20eq%20'{}'",
            urlenc(name)
        ),
        headers,
        "",
    )?;
    if find_resp.status >= 200 && find_resp.status < 300 {
        let existing: serde_json::Value = serde_json::from_str(&find_resp.body)
            .map_err(|e| format!("Failed to parse workspace lookup response: {e}"))?;
        if let Some(id) = existing
            .get("value")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("entity_id").or_else(|| v.get("Id")))
            .and_then(|v| v.as_str())
        {
            return Ok(id.to_string());
        }
    }

    let create_resp = ctx.http_call(
        "POST",
        &format!("{api_url}/tdata/Workspaces"),
        headers,
        &json!({ "Name": name }).to_string(),
    )?;
    if create_resp.status < 200 || create_resp.status >= 300 {
        return Err(format!(
            "Failed to create workspace '{name}': HTTP {}: {}",
            create_resp.status,
            &create_resp.body[..create_resp.body.len().min(500)]
        ));
    }

    let created: serde_json::Value = serde_json::from_str(&create_resp.body)
        .map_err(|e| format!("Failed to parse workspace creation response: {e}"))?;
    created
        .get("entity_id")
        .or_else(|| created.get("Id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Created workspace has no entity_id".to_string())
}

fn urlenc(s: &str) -> String {
    s.replace('%', "%25")
        .replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('?', "%3F")
        .replace('#', "%23")
        .replace('\'', "%27")
}
