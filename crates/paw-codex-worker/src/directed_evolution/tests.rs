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
        assert!(prompt.contains("Generate one candidate variant"));
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
