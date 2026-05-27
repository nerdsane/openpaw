#[cfg(test)]
mod directed_evolution_tests {
    use super::*;

    #[test]
    fn directed_evolution_work_item_parses_odata_fields() {
        let value = json!({
            "entity_id": "wi-1",
            "status": "Queued",
            "fields": {
                "Role": "observer",
                "TargetEntityType": "Episode",
                "TargetEntityId": "episode-1",
                "PromptRef": "literal:observe this",
                "ContextRef": "ctx-1",
                "OutputSchemaRef": "schema-1",
                "CorrelationJson": "{\"episode_id\":\"episode-1\"}"
            }
        });

        let work_item =
            directed_evolution_work_item_from_odata_value(value).expect("WorkItem should parse");

        assert_eq!(work_item.id, "wi-1");
        assert_eq!(work_item.status, "Queued");
        assert_eq!(work_item.role, "observer");
        assert_eq!(work_item.target_entity_type, "Episode");
        assert_eq!(work_item.target_entity_id, "episode-1");
        assert_eq!(work_item.prompt_ref, "literal:observe this");
        assert_eq!(work_item.context_ref, "ctx-1");
        assert_eq!(work_item.output_schema_ref, "schema-1");
        assert_eq!(work_item.correlation_json, "{\"episode_id\":\"episode-1\"}");
    }

    #[test]
    fn directed_evolution_prompt_uses_literal_prompt_ref() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-1".to_string(),
            status: "Queued".to_string(),
            role: "variant_generator".to_string(),
            target_entity_type: "Generation".to_string(),
            target_entity_id: "gen-1".to_string(),
            prompt_ref: "literal:make three variants".to_string(),
            context_ref: "ctx-1".to_string(),
            output_schema_ref: "schema-1".to_string(),
            correlation_json: "{}".to_string(),
        };

        let prompt = directed_evolution_prompt(&work_item);

        assert!(prompt.contains("Role: variant_generator"));
        assert!(prompt.contains("Generate one bounded candidate variant"));
        assert!(prompt.contains("make three variants"));
        assert!(!prompt.contains("literal:make three variants"));
    }

    #[test]
    fn directed_evolution_codex_stdout_is_normalized_to_json() {
        let parsed = parse_codex_jsonish("codex\n{\"summary\":\"variant\",\"changed_files\":[\"app.ts\"]}\n")
            .expect("parse JSON from Codex output");

        assert_eq!(parsed["summary"], "variant");
        assert_eq!(parsed["changed_files"], json!(["app.ts"]));
    }

    #[test]
    fn stale_stage_work_cancels_when_stage_result_already_eliminated() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-review".to_string(),
            status: "Queued".to_string(),
            role: "reviewer".to_string(),
            target_entity_type: "StageResult".to_string(),
            target_entity_id: "sr-1".to_string(),
            prompt_ref: String::new(),
            context_ref: String::new(),
            output_schema_ref: String::new(),
            correlation_json: "{}".to_string(),
        };

        let reason = stale_directed_evolution_stage_work_reason(
            &work_item,
            &json!({ "Status": "Eliminated", "VariantId": "var-1" }),
            &json!({ "Status": "Active" }),
        )
        .expect("eliminated stage result should cancel");

        assert!(reason.contains("StageResult sr-1 is already Eliminated"));
    }

    #[test]
    fn stale_stage_work_cancels_when_variant_already_eliminated() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-user".to_string(),
            status: "Queued".to_string(),
            role: "simulated_user".to_string(),
            target_entity_type: "StageResult".to_string(),
            target_entity_id: "sr-2".to_string(),
            prompt_ref: String::new(),
            context_ref: String::new(),
            output_schema_ref: String::new(),
            correlation_json: "{}".to_string(),
        };

        let reason = stale_directed_evolution_stage_work_reason(
            &work_item,
            &json!({ "Status": "Running", "VariantId": "var-2" }),
            &json!({ "Status": "Eliminated" }),
        )
        .expect("eliminated variant should cancel");

        assert!(reason.contains("Target variant is already Eliminated"));
    }

    #[test]
    fn stage_work_continues_for_running_active_variant() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-review".to_string(),
            status: "Queued".to_string(),
            role: "reviewer".to_string(),
            target_entity_type: "StageResult".to_string(),
            target_entity_id: "sr-3".to_string(),
            prompt_ref: String::new(),
            context_ref: String::new(),
            output_schema_ref: String::new(),
            correlation_json: "{}".to_string(),
        };

        let reason = stale_directed_evolution_stage_work_reason(
            &work_item,
            &json!({ "Status": "Running", "VariantId": "var-3" }),
            &json!({ "Status": "Active" }),
        );

        assert!(reason.is_none());
    }

    #[test]
    fn directed_evolution_repo_mapping_accepts_app_ref_prefix() {
        let previous = env::var_os("DIRECTED_EVOLUTION_ORGANISM_REPOS_JSON");
        unsafe {
            env::set_var(
                "DIRECTED_EVOLUTION_ORGANISM_REPOS_JSON",
                r#"{"arni-labs/agent-answers":"/tmp/agent-answers"}"#,
            );
        }

        let path = directed_evolution_repo_from_mapping(
            "org-agent-answers",
            "arni-labs/agent-answers@abc123",
        )
        .expect("repo should resolve");

        assert_eq!(path, PathBuf::from("/tmp/agent-answers"));
        unsafe {
            if let Some(value) = previous {
                env::set_var("DIRECTED_EVOLUTION_ORGANISM_REPOS_JSON", value);
            } else {
                env::remove_var("DIRECTED_EVOLUTION_ORGANISM_REPOS_JSON");
            }
        }
    }

    #[test]
    fn directed_evolution_app_ref_maps_to_genesis_app_id() {
        let app = directed_evolution_genesis_app_from_ref("nerdsane/agent-answers@abc123")
            .expect("app ref should parse");

        assert_eq!(app.owner, "nerdsane");
        assert_eq!(app.name, "agent-answers");
        assert_eq!(app.hash, "abc123");
        assert_eq!(app.app_id(), "app-nerdsane-agent-answers");
        assert_eq!(app.pinned_ref(), "nerdsane/agent-answers@abc123");
    }

    #[test]
    fn directed_evolution_variant_tenant_is_stable_and_safe() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "en-019e6713-ae4a/unsafe work item".to_string(),
            status: "Queued".to_string(),
            role: "variant_generator".to_string(),
            target_entity_type: "Generation".to_string(),
            target_entity_id: "gen-1".to_string(),
            prompt_ref: String::new(),
            context_ref: String::new(),
            output_schema_ref: String::new(),
            correlation_json: String::new(),
        };

        let tenant = directed_evolution_variant_tenant("de-variant", &work_item);

        assert_eq!(tenant, "de-variant-en-019e6713-ae4a-unsafe-work-item");
    }

    #[test]
    fn directed_evolution_variant_push_uses_registry_tenant_header() {
        let app = directed_evolution_genesis_app_from_ref("nerdsane/agent-answers@abc123")
            .expect("app ref should parse");

        let args = directed_evolution_variant_push_args(
            "https://genesis-production-164d.up.railway.app/",
            &app,
            "directed-evolution/wi-1",
            "default",
        );

        assert_eq!(
            args,
            vec![
                "-c",
                "http.extraHeader=x-tenant-id: default",
                "push",
                "https://genesis-production-164d.up.railway.app/nerdsane/agent-answers.git",
                "HEAD:refs/heads/directed-evolution/wi-1",
            ]
        );
    }

    #[test]
    fn directed_evolution_hotloaded_output_overrides_local_runtime_ref() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-variant-1".to_string(),
            status: "Running".to_string(),
            role: "variant_generator".to_string(),
            target_entity_type: "Generation".to_string(),
            target_entity_id: "gen-1".to_string(),
            prompt_ref: String::new(),
            context_ref: String::new(),
            output_schema_ref: String::new(),
            correlation_json: String::new(),
        };
        let mut payload = json!({
            "summary": "Adds answer evidence confidence.",
            "runtime_ref": "local-worktree:/tmp/variant"
        });
        let materialization = DirectedEvolutionVariantRuntime {
            app_ref: "nerdsane/agent-answers@abc123".to_string(),
            tenant: "de-variant-wi-variant-1".to_string(),
            runtime_ref: "temper://tenant/de-variant-wi-variant-1/app/nerdsane/agent-answers@abc123"
                .to_string(),
        };

        apply_directed_evolution_variant_runtime(&work_item, &mut payload, &materialization);

        assert_eq!(payload["app_ref"], "nerdsane/agent-answers@abc123");
        assert_eq!(payload["variant_tenant"], "de-variant-wi-variant-1");
        assert_eq!(
            payload["runtime_ref"],
            "temper://tenant/de-variant-wi-variant-1/app/nerdsane/agent-answers@abc123"
        );
        assert_eq!(payload["hot_loaded"], true);
    }

    #[test]
    fn directed_evolution_evidence_uri_prefers_agent_reference() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-1".to_string(),
            status: "Queued".to_string(),
            role: "reviewer".to_string(),
            target_entity_type: "StageResult".to_string(),
            target_entity_id: "sr-1".to_string(),
            prompt_ref: String::new(),
            context_ref: String::new(),
            output_schema_ref: String::new(),
            correlation_json: String::new(),
        };

        let uri = directed_evolution_evidence_uri(
            &work_item,
            &json!({
                "evidence_refs": ["temper://evidence/stage-result"]
            }),
        );

        assert_eq!(uri, "temper://evidence/stage-result");
    }

    #[test]
    fn directed_evolution_datadog_url_encodes_work_item_query() {
        let encoded = encode_url_component("service:temperpaw env:local @work_item_id:wi-1");

        assert_eq!(
            encoded,
            "service%3Atemperpaw%20env%3Alocal%20%40work_item_id%3Awi-1"
        );
    }
}
