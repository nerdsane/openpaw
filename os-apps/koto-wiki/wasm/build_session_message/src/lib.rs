use temper_wasm_sdk::prelude::*;

/// Build the user_message prompt based on job_type, then spawn a Session entity
/// and dispatch Configure on it with the constructed message and session params.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "build_session_message: starting");

        // --- Read WikiJob entity fields ---
        let fields = ctx
            .entity_state
            .get("fields")
            .cloned()
            .unwrap_or(json!({}));

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

        let persona_id = fields
            .get("persona_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let soul_id = fields
            .get("soul_id")
            .and_then(|v| v.as_str())
            .unwrap_or("app-soul-curator")
            .to_string();

        let model = fields
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("gpt-5")
            .to_string();

        let provider = fields
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("openai")
            .to_string();

        let tools_enabled = fields
            .get("tools_enabled")
            .and_then(|v| v.as_str())
            .unwrap_or("temper_get,temper_list,temper_create,temper_action,temper_write,temper_read,temper_web_search,temper_web_fetch")
            .to_string();

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

        let tenant = &ctx.tenant;

        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Tenant-Id".to_string(), tenant.to_string()),
            (
                "x-temper-principal-kind".to_string(),
                "agent".to_string(),
            ),
            (
                "x-temper-principal-id".to_string(),
                "system".to_string(),
            ),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // --- Build user_message based on job_type ---
        let user_message = match job_type.as_str() {
            "source_search" => build_source_search_message(&input, &persona_id, &entity_id),
            "synthesize" => build_synthesize_message(&persona_id, &entity_id),
            other => {
                return Err(format!(
                    "build_session_message: unsupported job_type '{other}'"
                ));
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

        let created: serde_json::Value =
            serde_json::from_str(&create_resp.body).map_err(|e| {
                format!("Failed to parse Session creation response: {e}")
            })?;

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
        let config_body = json!({
            "soul_id": soul_id,
            "user_message": user_message,
            "model": model,
            "provider": provider,
            "tools_enabled": tools_enabled,
            "max_turns": max_turns,
        });

        let configure_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Sessions('{session_id}')/OpenPaw.Configure"),
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
        });

        let spawned_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/WikiJobs('{entity_id}')/Kotowari.Wiki.SessionSpawned"),
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

/// Build the user_message for a source_search job.
///
/// Instructs the Curator agent to search the web, fetch content, store it in
/// paw-fs, and create WikiSource entities for each discovered source.
fn build_source_search_message(input: &str, persona_id: &str, job_id: &str) -> String {
    format!(
        r#"You are executing a WikiJob (source_search) for persona '{persona_id}'.
Job ID: {job_id}

## Your Task

Search the web for high-quality sources matching the scope below, fetch their content,
store it in paw-fs, and create WikiSource entities for each source you find.

## Search Scope

{input}

## Python Sandbox Rules

When writing Python code in any tool:
- Do NOT use any `import` statements — all functions are available in the global scope
- Do NOT use `enumerate()` with keyword arguments (e.g., `enumerate(items, start=1)` is forbidden; use `enumerate(items)` and add the offset manually)
- Available built-in functions: `temper.web_search(query)`, `temper.web_fetch(url)`, `temper.write(path, content)`, `temper.read(path)`

## Workflow

1. **Search**: Use `temper.web_search()` with varied queries derived from the search scope
2. **Fetch**: Use `temper.web_fetch()` to retrieve full content from promising URLs
3. **Store**: Use `temper.write()` to save raw content to paw-fs at paths like `wiki/sources/{{source_slug}}.md`
4. **Create WikiSource**: For each stored source, create a WikiSource entity:
   ```
   temper.create("WikiSources", {{
     "PersonaId": "{persona_id}",
     "Title": "<source title>",
     "SourceType": "<article|reference|tutorial|documentation>",
     "SourceUrl": "<original URL>",
     "FileId": "<file_id from temper.write()>",
     "Metadata": "<JSON string with author, date, word_count, etc.>"
   }})
   ```
5. **Index each source**: After creating each WikiSource, dispatch the Index action:
   ```
   temper.action("WikiSources", "<source_entity_id>", "Index", {{
     "extracted_topics": "<JSON array of topic strings>"
   }})
   ```
6. **Update job progress**: Periodically dispatch RecordProgress on the WikiJob:
   ```
   temper.action("WikiJobs", "{job_id}", "RecordProgress", {{
     "progress_log": "<JSON array of progress entries>"
   }})
   ```
7. **Complete**: When done, dispatch Complete on the WikiJob:
   ```
   temper.action("WikiJobs", "{job_id}", "Complete", {{
     "output": "<JSON summary: sources_found, sources_indexed, topics_discovered>"
   }})
   ```
8. **Trigger synthesis**: After completing sourcing, create a synthesis WikiJob so pages get built automatically:
   ```
   synth = temper.create("WikiJobs", {{}})
   temper.action("WikiJobs", synth["entity_id"], "Configure", {{
     "job_type": "synthesize",
     "persona_id": "{persona_id}",
     "input": '{{"task": "Synthesize wiki pages from all gathered sources"}}'
   }})
   temper.action("WikiJobs", synth["entity_id"], "Submit", {{}})
   ```
   This will reactively spawn a new Curator session for synthesis.

## Quality Standards

- Prefer authoritative sources (academic, official documentation, established references)
- Minimum 5 sources, aim for 10-20 for comprehensive coverage
- Extract topics from each source for later synthesis cross-referencing
- If a fetch fails, log it and continue with other sources
"#
    )
}

/// Build the user_message for a synthesize job.
///
/// Instructs the Curator agent to read all Indexed WikiSources, synthesize
/// knowledge into formatted WikiPage entities, and store page content in paw-fs.
fn build_synthesize_message(persona_id: &str, job_id: &str) -> String {
    format!(
        r#"You are executing a WikiJob (synthesize) for persona '{persona_id}'.
Job ID: {job_id}

## Your Task

Read all Indexed WikiSources for this persona, synthesize the knowledge into
well-structured wiki pages, store page content in paw-fs, and create WikiPage entities.

## Python Sandbox Rules

When writing Python code in any tool:
- Do NOT use any `import` statements — all functions are available in the global scope
- Do NOT use `enumerate()` with keyword arguments (e.g., `enumerate(items, start=1)` is forbidden; use `enumerate(items)` and add the offset manually)
- Available built-in functions: `temper.list(entity_set, filter)`, `temper.get(entity_set, id)`, `temper.create(entity_set, fields)`, `temper.action(entity_set, id, action, params)`, `temper.write(path, content)`, `temper.read(path)`

## Workflow

1. **List sources**: Retrieve all Indexed WikiSources for this persona:
   ```
   temper.list("WikiSources", "$filter=PersonaId eq '{persona_id}' and Status eq 'Indexed'")
   ```
2. **Read content**: For each source, use `temper.read()` to load the stored content via its FileId
3. **Plan pages**: Identify logical topics/themes that span multiple sources. Plan a coherent page structure.
4. **Write pages**: For each planned page, synthesize content from relevant sources and write it:
   ```
   temper.write("/wiki/pages/{{page_slug}}.md", content)
   ```
5. **Create WikiPage**: For each page:
   ```
   temper.create("WikiPages", {{
     "PersonaId": "{persona_id}",
     "Slug": "<page-slug>",
     "Title": "<Page Title>",
     "Category": "<grammar|vocabulary|culture|usage|reference>",
     "FileId": "<file_id from temper.write()>",
     "Summary": "<2-3 sentence summary>",
     "CrossReferences": "<JSON array of related page slugs>",
     "SourceIds": "<JSON array of WikiSource entity IDs used>",
     "Tags": "<JSON array of topic tags>",
     "DifficultyRange": "<JSON: {{\"min\": \"N5\", \"max\": \"N3\"}}>"
   }})
   ```
6. **Publish each page**: Dispatch Publish on each WikiPage:
   ```
   temper.action("WikiPages", "<page_entity_id>", "Publish", {{}})
   ```
7. **Update job progress**: Periodically dispatch RecordProgress on the WikiJob
8. **Complete**: When done, dispatch Complete on the WikiJob:
   ```
   temper.action("WikiJobs", "{job_id}", "Complete", {{
     "output": "<JSON summary: pages_created, topics_covered, source_coverage>"
   }})
   ```

## Markdown Formatting Standards

All wiki page content MUST follow these conventions:
- **Headers**: Use `##` for main sections, `###` for subsections
- **Japanese terms**: Always **bold** Japanese text, followed by reading in parentheses: **食べる** (taberu)
- **Examples**: Use blockquotes with Japanese, romanization, and translation:
  > **日本語を勉強しています。**
  > *Nihongo o benkyou shite imasu.*
  > "I am studying Japanese."
- **Tables**: Use for conjugation patterns, comparison charts, vocabulary lists
- **Wikilinks**: Use `[[page-slug]]` syntax for cross-references to other wiki pages
- **Source attribution**: Include a "Sources" section at the bottom referencing WikiSource IDs

## Quality Standards

- Each page should synthesize information from multiple sources
- Aim for comprehensive but focused pages (1000-3000 words)
- Include practical examples for every concept
- Cross-reference related pages extensively
- Maintain consistent terminology across all pages
"#
    )
}
