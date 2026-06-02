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
    fn observer_prompt_requires_datadog_evidence_scope() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-observer".to_string(),
            status: "Queued".to_string(),
            role: "observer".to_string(),
            target_entity_type: "Signal".to_string(),
            target_entity_id: "sig-1".to_string(),
            prompt_ref: "literal:Source: datadog\nSummary: p95 latency climbed".to_string(),
            context_ref: "signal:sig-1".to_string(),
            output_schema_ref: "schema-1".to_string(),
            correlation_json: "{\"source\":\"datadog\"}".to_string(),
        };

        let prompt = directed_evolution_prompt(&work_item);

        assert!(prompt.contains("Datadog MCP"));
        assert!(prompt.contains("evidence_scope"));
        assert!(prompt.contains("datadog_url"));
        assert!(prompt.contains("Do not treat the signal summary as proof"));
    }

    #[test]
    fn evaluation_prompt_treats_required_datadog_as_mandatory() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-review".to_string(),
            status: "Queued".to_string(),
            role: "telemetry_evaluator".to_string(),
            target_entity_type: "StageResult".to_string(),
            target_entity_id: "sr-1".to_string(),
            prompt_ref: "literal:RequiredEvidence: [\"datadog_evidence_scope\"]".to_string(),
            context_ref: "stage-result:sr-1".to_string(),
            output_schema_ref: "schema-1".to_string(),
            correlation_json: "{}".to_string(),
        };

        let prompt = directed_evolution_prompt(&work_item);

        assert!(prompt.contains("Datadog is the primary judging surface"));
        assert!(prompt.contains("directed_evolution.variant_id"));
        assert!(prompt.contains("provenance_kind=datadog-measured"));
        assert!(prompt.contains("result_count"));
        assert!(prompt.contains("zero_result_meaning"));
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
            role: "viability_evaluator".to_string(),
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
    fn stale_stage_work_terminalizes_running_stage_result() {
        assert!(stale_stage_result_should_eliminate(&json!({
            "Status": "Running"
        })));
        assert!(stale_stage_result_should_eliminate(&json!({
            "Status": "Failed"
        })));
        assert!(!stale_stage_result_should_eliminate(&json!({
            "Status": "Eliminated"
        })));
        assert!(!stale_stage_result_should_eliminate(&json!({
            "Status": "Passed"
        })));
    }

    #[test]
    fn stale_stage_work_targets_only_evaluation_stage_results() {
        let mut work_item = DirectedEvolutionWorkItemState {
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

        assert!(stale_stage_work_targets_stage_result(&work_item));
        work_item.role = "variant_generator".to_string();
        assert!(!stale_stage_work_targets_stage_result(&work_item));
        work_item.role = "simulated_user".to_string();
        work_item.target_entity_type = "Variant".to_string();
        assert!(!stale_stage_work_targets_stage_result(&work_item));
    }

    #[test]
    fn mechanical_state_verifier_passes_bounded_agent_answers_mutation() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-state".to_string(),
            status: "Running".to_string(),
            role: "state_verifier".to_string(),
            target_entity_type: "StageResult".to_string(),
            target_entity_id: "sr-1".to_string(),
            prompt_ref: String::new(),
            context_ref: String::new(),
            output_schema_ref: String::new(),
            correlation_json: "{}".to_string(),
        };

        let output = directed_evolution_state_verifier_output(
            &work_item,
            &json!({ "VariantId": "var-1", "EvaluationStageId": "stage-1" }),
            &json!({ "MutationId": "mut-1" }),
            &json!({
                "ChangedFilesJson": "[\"apps/agent-answers/specs/question.ioa.toml\",\"apps/agent-answers/APP.md\"]"
            }),
            &json!({
                "EvaluatorRef": "genesis://nerdsane/agent-answers-evaluation@abc",
                "EvaluatorModule": "state-verifier"
            }),
        );

        assert_eq!(output["passed"], true);
        assert_eq!(output["provenance_kind"], "state-verified");
        assert_eq!(output["metrics"]["viability_regression_count"]["value"], 0);
        assert_eq!(output["metrics"]["mutation_file_count"]["value"], 2);
    }

    #[test]
    fn mechanical_state_verifier_passes_canonical_app_root_mutation() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-state".to_string(),
            status: "Running".to_string(),
            role: "state_verifier".to_string(),
            target_entity_type: "StageResult".to_string(),
            target_entity_id: "sr-root".to_string(),
            prompt_ref: String::new(),
            context_ref: String::new(),
            output_schema_ref: String::new(),
            correlation_json: "{}".to_string(),
        };

        let output = directed_evolution_state_verifier_output(
            &work_item,
            &json!({ "VariantId": "var-root", "EvaluationStageId": "stage-root" }),
            &json!({ "MutationId": "mut-root" }),
            &json!({
                "ChangedFilesJson": "[\"APP.md\",\"adrs/0005-question-intent-context.md\",\"policies/agent_answers.cedar\",\"specs/model.csdl.xml\",\"specs/question.ioa.toml\"]"
            }),
            &json!({
                "EvaluatorRef": "genesis://nerdsane/agent-answers-evaluation@abc",
                "EvaluatorModule": "state-verifier"
            }),
        );

        assert_eq!(output["passed"], true);
        assert_eq!(output["metrics"]["viability_regression_count"]["value"], 0);
        assert_eq!(output["metrics"]["mutation_file_count"]["value"], 5);
    }

    #[test]
    fn mechanical_state_verifier_rejects_evaluator_mutation() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-state".to_string(),
            status: "Running".to_string(),
            role: "state_verifier".to_string(),
            target_entity_type: "StageResult".to_string(),
            target_entity_id: "sr-2".to_string(),
            prompt_ref: String::new(),
            context_ref: String::new(),
            output_schema_ref: String::new(),
            correlation_json: "{}".to_string(),
        };

        let output = directed_evolution_state_verifier_output(
            &work_item,
            &json!({ "VariantId": "var-2", "EvaluationStageId": "stage-2" }),
            &json!({ "MutationId": "mut-2" }),
            &json!({
                "ChangedFilesJson": "[\"apps/agent-answers-evaluation/wasm/state_verifier/src/lib.rs\"]"
            }),
            &json!({}),
        );

        assert_eq!(output["passed"], false);
        assert_eq!(
            output["metrics"]["evaluator_boundary_violation_count"]["value"],
            1
        );
        assert!(
            output["failure_reason"]
                .as_str()
                .unwrap()
                .contains("pinned evaluator files")
        );
    }

    #[test]
    fn mechanical_state_verifier_rejects_missing_required_changed_file() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-state".to_string(),
            status: "Running".to_string(),
            role: "state_verifier".to_string(),
            target_entity_type: "StageResult".to_string(),
            target_entity_id: "sr-required-file".to_string(),
            prompt_ref: String::new(),
            context_ref: String::new(),
            output_schema_ref: String::new(),
            correlation_json: "{}".to_string(),
        };

        let output = directed_evolution_state_verifier_output(
            &work_item,
            &json!({ "VariantId": "var-required-file", "EvaluationStageId": "stage-required-file" }),
            &json!({ "MutationId": "mut-required-file" }),
            &json!({
                "ChangedFilesJson": "[\"APP.md\",\"specs/question.ioa.toml\",\"specs/model.csdl.xml\"]"
            }),
            &json!({
                "RequiredEvidenceJson": "[\"changed_file:specs/answer.ioa.toml\",\"no_evaluator_mutation\"]"
            }),
        );

        assert_eq!(output["passed"], false);
        assert_eq!(
            output["metrics"]["required_changed_file_missing_count"]["value"],
            1
        );
        assert!(
            output["failure_reason"]
                .as_str()
                .unwrap()
                .contains("specs/answer.ioa.toml")
        );
    }

    #[test]
    fn mechanical_evaluator_roles_do_not_spawn_codex_agents() {
        assert!(directed_evolution_mechanical_evaluator_role("state_verifier"));
        assert!(directed_evolution_mechanical_evaluator_role("wasm_evaluator"));
        assert_eq!(
            directed_evolution_agent_kind_for_role("state_verifier"),
            "temperpaw-worker"
        );
        assert_eq!(
            directed_evolution_model_for_role("wasm_evaluator"),
            "deterministic-worker"
        );
        assert!(!directed_evolution_mechanical_evaluator_role("simulated_user"));
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
    fn directed_evolution_repo_mapping_accepts_object_path() {
        let previous = env::var_os("DIRECTED_EVOLUTION_ORGANISM_REPOS_JSON");
        unsafe {
            env::set_var(
                "DIRECTED_EVOLUTION_ORGANISM_REPOS_JSON",
                r#"{"org-agent-answers":{"path":"/tmp/agent-answers","owner":"arni-labs","app":"agent-answers"}}"#,
            );
        }

        let path =
            directed_evolution_repo_from_mapping("org-agent-answers", "arni-labs/agent-answers@abc123")
                .expect("repo should resolve from object path");

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
    fn directed_evolution_genesis_bundle_url_targets_pinned_version() {
        let app = directed_evolution_genesis_app_from_ref("nerdsane/agent-answers@abc123")
            .expect("app ref should parse");

        let url = directed_evolution_genesis_bundle_url(
            "https://genesis-production-164d.up.railway.app/",
            &app,
        );

        assert_eq!(
            url,
            "https://genesis-production-164d.up.railway.app/api/genesis/apps/nerdsane/agent-answers/versions/abc123/bundle"
        );
    }

    #[test]
    fn directed_evolution_bundle_path_rejects_traversal() {
        assert!(safe_directed_evolution_bundle_path("specs/answer.ioa.toml").is_ok());
        assert!(safe_directed_evolution_bundle_path("../answer.ioa.toml").is_err());
        assert!(safe_directed_evolution_bundle_path("/tmp/answer.ioa.toml").is_err());
        assert!(safe_directed_evolution_bundle_path(".git/config").is_err());
        assert!(safe_directed_evolution_bundle_path(".Git/config").is_err());
        assert!(safe_directed_evolution_bundle_path("specs/.git/config").is_err());
        assert!(safe_directed_evolution_bundle_path("specs/.GIT/config").is_err());
    }

    #[tokio::test]
    async fn directed_evolution_bundle_materializes_git_repo() {
        let repo = env::temp_dir().join(format!(
            "paw-codex-worker-bundle-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let app = directed_evolution_genesis_app_from_ref("nerdsane/agent-answers@abc123")
            .expect("app ref should parse");
        let bundle = json!({
            "apps": [{
                "files": [
                    {
                        "path": "APP.md",
                        "content_base64": "IyBBZ2VudCBBbnN3ZXJzCg=="
                    },
                    {
                        "path": "specs/answer.ioa.toml",
                        "content_base64": "W2F1dG9tYXRvbl0KbmFtZSA9ICJBbnN3ZXIiCg=="
                    },
                    {
                        "path": ".gitignore",
                        "content_base64": "ZGlzdC8K"
                    },
                    {
                        "path": "dist/app.wasm",
                        "content_base64": "AAECAw=="
                    }
                ]
            }]
        });

        materialize_directed_evolution_bundle_repo(&repo, &app, &bundle)
            .await
            .expect("bundle materializes");

        assert!(repo.join(".git").exists());
        assert_eq!(
            fs::read_to_string(repo.join("APP.md")).expect("read APP.md"),
            "# Agent Answers\n"
        );
        let head = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("run git");
        assert!(
            head.status.success(),
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&head.stderr)
        );
        assert!(!String::from_utf8_lossy(&head.stdout).trim().is_empty());
        let listed = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["ls-files", "--", "dist/app.wasm"])
            .output()
            .expect("run git ls-files");
        assert!(
            listed.status.success(),
            "git ls-files failed: {}",
            String::from_utf8_lossy(&listed.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&listed.stdout).trim(),
            "dist/app.wasm",
            "forced bundle materialization should commit files ignored by bundled .gitignore"
        );
        fs::remove_dir_all(repo).ok();
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
    fn directed_evolution_production_tenant_defaults_to_default() {
        let previous = env::var_os("DIRECTED_EVOLUTION_PRODUCTION_TENANT");
        unsafe {
            env::remove_var("DIRECTED_EVOLUTION_PRODUCTION_TENANT");
        }

        assert_eq!(directed_evolution_production_tenant(), "default");

        unsafe {
            if let Some(value) = previous {
                env::set_var("DIRECTED_EVOLUTION_PRODUCTION_TENANT", value);
            }
        }
    }

    #[test]
    fn directed_evolution_registry_tenant_defaults_to_default() {
        let previous = env::var_os("DIRECTED_EVOLUTION_REGISTRY_TENANT");
        unsafe {
            env::remove_var("DIRECTED_EVOLUTION_REGISTRY_TENANT");
        }

        let config = Config {
            temper_url: "http://temper.test".to_string(),
            tenant: "de-control".to_string(),
            worker_id: "worker".to_string(),
            worker_token: None,
            workspace_root: PathBuf::from("/tmp/workspaces"),
            repo_root: PathBuf::from("/tmp/repo"),
            codex_bin: "codex".to_string(),
            max_concurrent_runs: 1,
            enable_execution: true,
            poll_on_start: true,
            codex_exec_smoke: false,
            codex_exec_timeout: Duration::from_secs(60),
        };

        assert_eq!(directed_evolution_registry_tenant(&config), "default");

        unsafe {
            if let Some(value) = previous {
                env::set_var("DIRECTED_EVOLUTION_REGISTRY_TENANT", value);
            }
        }
    }

    #[test]
    fn promotion_materialization_output_reports_runtime_ref() {
        let materialization = DirectedEvolutionPromotionMaterialization {
            canonical_app_ref: "nerdsane/agent-answers@abc123".to_string(),
            production_tenant: "default".to_string(),
            runtime_ref: "temper://tenant/default/app/nerdsane/agent-answers@abc123".to_string(),
            summary: "Published and installed winner".to_string(),
            digest: "abc123".to_string(),
        };

        let payload = directed_evolution_promotion_output(&materialization);

        assert_eq!(payload["canonical_app_ref"], "nerdsane/agent-answers@abc123");
        assert_eq!(payload["production_tenant"], "default");
        assert_eq!(
            payload["runtime_ref"],
            "temper://tenant/default/app/nerdsane/agent-answers@abc123"
        );
        assert_eq!(payload["evidence_refs"][0], payload["runtime_ref"]);
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
    fn directed_evolution_evidence_uri_prefers_datadog_scope_url() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-logs".to_string(),
            status: "Queued".to_string(),
            role: "observer".to_string(),
            target_entity_type: "Signal".to_string(),
            target_entity_id: "sig-1".to_string(),
            prompt_ref: String::new(),
            context_ref: String::new(),
            output_schema_ref: String::new(),
            correlation_json: String::new(),
        };

        let uri = directed_evolution_evidence_uri(
            &work_item,
            &json!({
                "evidence_scope": [
                    {
                        "surface": "logs",
                        "query": "service:temperpaw",
                        "result_summary": "no errors",
                        "datadog_url": "https://app.datadoghq.com/logs?query=service%3Atemperpaw"
                    }
                ]
            }),
        );

        assert_eq!(
            uri,
            "https://app.datadoghq.com/logs?query=service%3Atemperpaw"
        );
    }

    #[test]
    fn directed_evolution_evidence_uri_accepts_datadog_site_hosts() {
        assert!(is_datadog_app_url("https://app.datadoghq.com/logs"));
        assert!(is_datadog_app_url("https://app.us3.datadoghq.com/apm/traces"));
        assert!(is_datadog_app_url("https://app.datadoghq.eu/metric/explorer"));
        assert!(is_datadog_app_url("https://app.ap2.datadoghq.com/dashboard"));
        assert!(!is_datadog_app_url("https://example.com/logs"));
    }

    #[test]
    fn directed_evolution_evidence_summary_preserves_numeric_result_count() {
        let summary = directed_evolution_first_evidence_scope_summary(&json!({
            "evidence_scope": [
                {
                    "surface": "logs",
                    "query": "service:temper-platform",
                    "time_window": "2026-05-28T00:00:00Z to 2026-05-29T23:59:59Z",
                    "result_count": 78,
                    "interpretation": "Datadog returned matching runtime-request logs.",
                    "zero_result_meaning": "failure"
                }
            ]
        }));

        assert_eq!(summary.query, "service:temper-platform");
        assert_eq!(summary.result_count, "78");
        assert_eq!(summary.zero_result_meaning, "failure");
    }

    #[test]
    fn directed_evolution_evidence_summary_prefers_complete_datadog_scope() {
        let summary = directed_evolution_first_evidence_scope_summary(&json!({
            "evidence_scope": [
                {
                    "surface": "genesis",
                    "query": "app history",
                    "result_summary": "Found prior changes."
                },
                {
                    "surface": "logs",
                    "query": "service:temper-platform @directed_evolution.variant_id:v3",
                    "time_window": "2026-06-02T14:00:00Z/2026-06-02T15:00:00Z",
                    "result_count": 31,
                    "interpretation": "Runtime logs were present for v3.",
                    "zero_result_meaning": "failure",
                    "datadog_url": "https://app.datadoghq.com/logs?query=v3"
                }
            ]
        }));

        assert_eq!(
            summary.query,
            "service:temper-platform @directed_evolution.variant_id:v3"
        );
        assert_eq!(summary.result_count, "31");
        assert_eq!(summary.interpretation, "Runtime logs were present for v3.");
    }

    #[test]
    fn directed_evolution_datadog_roles_require_structured_evidence() {
        fn work_item(role: &str, prompt_ref: &str) -> DirectedEvolutionWorkItemState {
            DirectedEvolutionWorkItemState {
                id: format!("wi-{role}"),
                status: "Queued".to_string(),
                role: role.to_string(),
                target_entity_type: "StageResult".to_string(),
                target_entity_id: "sr-1".to_string(),
                prompt_ref: prompt_ref.to_string(),
                context_ref: "stage-result:sr-1".to_string(),
                output_schema_ref: "schema-1".to_string(),
                correlation_json: "{}".to_string(),
            }
        }
        let observer = work_item("observer", "literal:observe");
        let telemetry = work_item("telemetry_evaluator", "literal:evaluate telemetry");
        let plain_reviewer = work_item("reviewer", "literal:review without Datadog gate");
        let datadog_reviewer = work_item(
            "reviewer",
            "literal:RequiredEvidence: [\"datadog_evidence_scope\"]",
        );
        let datadog_viability = work_item(
            "viability_evaluator",
            "literal:RequiredEvidence: [\"datadog_evidence_scope\"]",
        );

        assert!(directed_evolution_work_item_requires_datadog_evidence(
            &observer
        ));
        assert!(directed_evolution_work_item_requires_datadog_evidence(
            &telemetry
        ));
        assert!(!directed_evolution_work_item_requires_datadog_evidence(
            &plain_reviewer
        ));
        assert!(directed_evolution_work_item_requires_datadog_evidence(
            &datadog_reviewer
        ));
        assert!(directed_evolution_work_item_requires_datadog_evidence(
            &datadog_viability
        ));

        assert!(
            ensure_directed_evolution_required_datadog_evidence(
                &observer,
                &json!({
                    "evidence_scope": [
                        {
                            "surface": "genesis",
                            "query": "app history",
                            "result_summary": "Found prior answer changes."
                        },
                        {
                            "surface": "logs",
                            "query": "service:temper-platform",
                            "time_window": "2026-06-02T14:00:00Z/2026-06-02T15:00:00Z",
                            "result_count": 12,
                            "interpretation": "Runtime request logs were present.",
                            "zero_result_meaning": "failure",
                            "datadog_url": "https://app.datadoghq.com/logs?query=service%3Atemper-platform"
                        }
                    ]
                })
            )
            .is_ok()
        );
        assert!(
            ensure_directed_evolution_required_datadog_evidence(
                &telemetry,
                &json!({
                    "evidence_scope": [{
                        "query": "service:temper-platform",
                        "time_window": "2026-06-02T14:00:00Z/2026-06-02T15:00:00Z",
                        "result_count": 0,
                        "interpretation": "No runtime logs were present.",
                        "zero_result_meaning": "failure"
                    }]
                })
            )
            .is_err()
        );
        assert!(
            ensure_directed_evolution_required_datadog_evidence(
                &plain_reviewer,
                &json!({"summary": "Reviewer evidence can be non-Datadog."})
            )
            .is_ok()
        );
        assert!(
            ensure_directed_evolution_required_datadog_evidence(
                &datadog_reviewer,
                &json!({"summary": "Reviewer prompt required Datadog but omitted it."})
            )
            .is_err()
        );
        assert!(
            ensure_directed_evolution_required_datadog_evidence(
                &datadog_viability,
                &json!({
                    "evidence_scope": [{
                        "query": "service:temper-platform @directed_evolution.variant_id:v2",
                        "time_window": "2026-06-02T14:00:00Z/2026-06-02T15:00:00Z",
                        "result_count": 4,
                        "interpretation": "Viability Datadog check returned runtime logs.",
                        "zero_result_meaning": "failure",
                        "datadog_url": "https://app.datadoghq.com/logs?query=v2"
                    }]
                })
            )
            .is_ok()
        );
    }

    #[test]
    fn directed_evolution_summary_uses_agent_summary_when_available() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-review".to_string(),
            status: "Queued".to_string(),
            role: "reviewer".to_string(),
            target_entity_type: "StageResult".to_string(),
            target_entity_id: "sr-1".to_string(),
            prompt_ref: String::new(),
            context_ref: String::new(),
            output_schema_ref: String::new(),
            correlation_json: String::new(),
        };

        let summary = directed_evolution_summary(
            &work_item,
            r#"{"summary":"Variant preserved baseline actions and showed no live OData errors."}"#,
        );

        assert!(summary.contains("reviewer"));
        assert!(summary.contains("Variant preserved baseline actions"));
    }

    #[test]
    fn directed_evolution_datadog_url_encodes_work_item_query() {
        let encoded = encode_url_component("service:temperpaw env:local @work_item_id:wi-1");

        assert_eq!(
            encoded,
            "service%3Atemperpaw%20env%3Alocal%20%40work_item_id%3Awi-1"
        );
    }

    #[test]
    fn directed_evolution_datadog_context_carries_join_fields() {
        let work_item = DirectedEvolutionWorkItemState {
            id: "wi-dd".to_string(),
            status: "Queued".to_string(),
            role: "simulated_user".to_string(),
            target_entity_type: "StageResult".to_string(),
            target_entity_id: "sr-1".to_string(),
            prompt_ref: String::new(),
            context_ref: String::new(),
            output_schema_ref: String::new(),
            correlation_json: String::new(),
        };

        let context = directed_evolution_datadog_context(&work_item);

        assert_eq!(context["work_item_id"], "wi-dd");
        assert_eq!(context["role"], "simulated_user");
        assert_eq!(context["target_entity_type"], "StageResult");
        assert_eq!(context["target_entity_id"], "sr-1");
        assert!(context["query"].as_str().unwrap().contains("@work_item_id:wi-dd"));
    }

    #[test]
    fn directed_evolution_claim_conflict_is_benign_race_signature() {
        let message =
            "WorkItems.ClaimWorkItem returned 409 Conflict: Action 'ClaimWorkItem' not valid from state 'Running'";

        assert!(directed_evolution_claim_conflict_message(message));
        assert!(!directed_evolution_claim_conflict_message(
            "WorkItems.StartWorkItem returned 409 Conflict: Action 'StartWorkItem' not valid from state 'Queued'"
        ));
    }

    #[test]
    fn directed_evolution_exclusive_key_conflict_detects_active_peer() {
        let rows = vec![
            json!({
                "entity_id": "wi-selector-current",
                "fields": {"Status": "Queued"}
            }),
            json!({
                "entity_id": "wi-selector-active",
                "fields": {"Status": "Running"}
            }),
            json!({
                "entity_id": "wi-selector-done",
                "fields": {"Status": "Succeeded"}
            }),
        ];

        assert_eq!(
            directed_evolution_exclusive_key_conflict_from_rows(
                "wi-selector-current",
                &rows,
                ExclusiveKeyConflictPolicy::AnyActivePeer,
            )
            .as_deref(),
            Some("wi-selector-active")
        );
        assert_eq!(
            directed_evolution_exclusive_key_conflict_from_rows(
                "wi-selector-active",
                &rows,
                ExclusiveKeyConflictPolicy::AnyActivePeer,
            ),
            None
        );
    }

    #[test]
    fn directed_evolution_post_claim_conflict_lets_lowest_active_work_item_win() {
        let rows = vec![
            json!({
                "entity_id": "wi-selector-003",
                "fields": {"Status": "Claimed"}
            }),
            json!({
                "entity_id": "wi-selector-001",
                "fields": {"Status": "Claimed"}
            }),
            json!({
                "entity_id": "wi-selector-004",
                "fields": {"Status": "Running"}
            }),
        ];

        assert_eq!(
            directed_evolution_exclusive_key_conflict_from_rows(
                "wi-selector-003",
                &rows,
                ExclusiveKeyConflictPolicy::LowerActivePeer,
            )
            .as_deref(),
            Some("wi-selector-001")
        );
        assert_eq!(
            directed_evolution_exclusive_key_conflict_from_rows(
                "wi-selector-001",
                &rows,
                ExclusiveKeyConflictPolicy::LowerActivePeer,
            ),
            None
        );
    }

    #[test]
    fn human_episode_contract_uses_direction_and_organism_defaults() {
        let input: DirectedEvolutionHumanEpisodeInput = serde_json::from_value(json!({
            "direction_id": "direction-growth",
            "selection_statement": "Prefer variants with clearer answer comparison.",
            "viability_constraints": [
                {"statement": "Answer creation still works.", "kind": "baseline"}
            ]
        }))
        .expect("contract input should parse");

        let plan = directed_evolution_episode_plan_from_input(
            input,
            &json!({
                "OrganismId": "org-agent-answers",
                "AutonomyLane": "growth-human-gated",
                "ProposedAdaptationGoal": "Humans compare candidate answers before acceptance.",
                "ProposedViabilityConstraintsJson": "[\"Do not regress answer acceptance.\"]"
            }),
            &json!({
                "OrganismVersionId": "ov-parent"
            }),
        )
        .expect("episode plan should resolve");

        assert_eq!(plan.direction_id, "direction-growth");
        assert_eq!(plan.organism_id, "org-agent-answers");
        assert_eq!(plan.parent_version_id, "ov-parent");
        assert_eq!(plan.autonomy_lane, "growth-human-gated");
        assert_eq!(
            plan.adaptation_goal,
            "Humans compare candidate answers before acceptance."
        );
        assert_eq!(plan.viability_constraints[0].statement, "Answer creation still works.");
        assert_eq!(plan.metrics.len(), 5);
        assert_eq!(plan.simulated_user_plan.users_per_variant, 3);
        assert_eq!(plan.simulated_user_plan.runs_per_persona, 2);
        assert!(
            plan.evaluation_stages
                .iter()
                .any(|stage| stage.kind == "simulated_user")
        );
    }

    #[test]
    fn human_episode_contract_falls_back_to_direction_constraints() {
        let input: DirectedEvolutionHumanEpisodeInput = serde_json::from_value(json!({
            "DirectionId": "direction-growth",
            "AdaptationGoal": "Improve citation memory."
        }))
        .expect("contract input should parse");

        let plan = directed_evolution_episode_plan_from_input(
            input,
            &json!({
                "OrganismId": "org-agent-answers",
                "ProposedViabilityConstraintsJson": "[\"Keep source fidelity.\",\"Do not increase answer latency.\"]"
            }),
            &json!({
                "ParentVersionId": "ov-parent"
            }),
        )
        .expect("episode plan should resolve");

        assert_eq!(plan.viability_constraints.len(), 2);
        assert_eq!(plan.viability_constraints[0].statement, "Keep source fidelity.");
        assert_eq!(plan.viability_constraints[1].statement, "Do not increase answer latency.");
    }

    #[test]
    fn human_episode_plan_authors_semantic_entities_directly() {
        let input: DirectedEvolutionHumanEpisodeInput = serde_json::from_value(json!({
            "DirectionId": "direction-growth",
            "AdaptationGoal": "Improve answer comparison.",
            "Metrics": [
                {"MetricName": "goal_score", "MetricKind": "goal", "Unit": "score"}
            ],
            "EliminationRules": [
                {"RuleStatement": "Eliminate failures.", "MetricNames": ["goal_score"]}
            ],
            "ScoringRules": [
                {"RuleStatement": "Prefer highest goal score.", "MetricNames": ["goal_score"]}
            ]
        }))
        .expect("contract input should parse");
        let plan = directed_evolution_episode_plan_from_input(
            input,
            &json!({
                "OrganismId": "org-agent-answers",
                "ProposedAdaptationGoal": "Direction goal",
            }),
            &json!({ "ParentVersionId": "ov-parent" }),
        )
        .expect("episode plan should resolve");

        assert_eq!(plan.direction_id, "direction-growth");
        assert_eq!(plan.organism_id, "org-agent-answers");
        assert_eq!(plan.parent_version_id, "ov-parent");
        assert_eq!(plan.metrics[0].name, "goal_score");
        assert_eq!(plan.metrics[0].provenance_kind, "brain-judged");
        assert_eq!(plan.elimination_rules[0].statement, "Eliminate failures.");
        assert_eq!(plan.scoring_rules[0].statement, "Prefer highest goal score.");
        assert_eq!(plan.simulated_user_plan.personas.len(), 3);
        assert!(!plan.evaluator_ref.trim().is_empty());
    }

    #[test]
    fn human_episode_contract_accepts_snake_case_semantics() {
        let input: DirectedEvolutionHumanEpisodeInput = serde_json::from_value(json!({
            "direction_id": "direction-growth",
            "adaptation_goal": "Improve answer comparison.",
            "metrics": [
                {
                    "metric_name": "required_changed_file_missing_count",
                    "metric_kind": "state",
                    "unit": "files",
                    "higher_is_better": false,
                    "provenance_kind": "state-verified",
                    "evaluator_ref": "genesis://nerdsane/agent-answers-evaluation@frozen",
                    "evaluator_module": "state-verifier",
                    "hard_constraint": true
                }
            ],
            "viability_constraints": [
                {
                    "constraint_statement": "Answer spec must change.",
                    "constraint_kind": "state-verified"
                }
            ],
            "elimination_rules": [
                {
                    "rule_statement": "Eliminate missing answer spec mutation.",
                    "metric_names": ["required_changed_file_missing_count"],
                    "threshold": { "max": 0 }
                }
            ],
            "scoring_rules": [
                {
                    "rule_statement": "Prefer exact answer-spec fit.",
                    "metric_names": ["required_changed_file_missing_count"],
                    "weight": "0.20"
                }
            ],
            "evaluation_stages": [
                {
                    "stage_name": "Deterministic state verification",
                    "stage_kind": "state_verification",
                    "executor_kind": "state_verifier",
                    "required_evidence": ["changed_file:specs/answer.ioa.toml"],
                    "measurement_provenance": "state-verified",
                    "evaluator_ref": "genesis://nerdsane/agent-answers-evaluation@frozen",
                    "evaluator_module": "state-verifier",
                    "decision_authority": "mechanical"
                }
            ]
        }))
        .expect("contract input should parse");

        let plan = directed_evolution_episode_plan_from_input(
            input,
            &json!({
                "OrganismId": "org-agent-answers",
                "ProposedAdaptationGoal": "Direction goal",
            }),
            &json!({ "ParentVersionId": "ov-parent" }),
        )
        .expect("episode plan should resolve");

        assert_eq!(plan.metrics[0].name, "required_changed_file_missing_count");
        assert!(!plan.metrics[0].higher_is_better);
        assert!(plan.metrics[0].hard_constraint);
        assert_eq!(plan.viability_constraints[0].statement, "Answer spec must change.");
        assert_eq!(plan.elimination_rules[0].metric_names[0], "required_changed_file_missing_count");
        assert_eq!(plan.scoring_rules[0].weight, "0.20");
        assert_eq!(plan.evaluation_stages[0].required_evidence[0], "changed_file:specs/answer.ioa.toml");
    }

    #[test]
    fn directed_evolution_start_episode_command_accepts_contract_path() {
        let command = parse_worker_command([
            "directed-evolution-start-episode".to_string(),
            "episode.json".to_string(),
        ]);

        assert_eq!(
            command,
            WorkerCommand::DirectedEvolutionStartEpisode {
                contract_path: Some("episode.json".to_string())
            }
        );
    }
}
