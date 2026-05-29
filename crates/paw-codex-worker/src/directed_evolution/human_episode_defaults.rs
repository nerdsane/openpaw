fn metric_plan_from_input(
    metric: DirectedEvolutionMetricInput,
) -> Option<DirectedEvolutionMetricPlan> {
    (!metric.name.trim().is_empty()).then(|| DirectedEvolutionMetricPlan {
        name: metric.name,
        kind: nonempty(metric.kind, "quality".to_string()),
        unit: nonempty(metric.unit, "score".to_string()),
        higher_is_better: metric.higher_is_better.unwrap_or(true),
        description: nonempty(metric.description, "Human-Codex negotiated metric.".to_string()),
        provenance_kind: nonempty(metric.provenance_kind, "brain-judged".to_string()),
        evaluator_ref: metric.evaluator_ref,
        evaluator_module: metric.evaluator_module,
        interpretation: metric.interpretation,
        hard_constraint: metric.hard_constraint.unwrap_or(false),
    })
}

fn constraint_plan_from_input(
    constraint: DirectedEvolutionConstraintInput,
) -> Option<DirectedEvolutionConstraintPlan> {
    match constraint {
        DirectedEvolutionConstraintInput::Text(statement) => {
            (!statement.trim().is_empty()).then(|| DirectedEvolutionConstraintPlan {
                statement,
                kind: "human-negotiated".to_string(),
            })
        }
        DirectedEvolutionConstraintInput::Object { statement, kind } => {
            (!statement.trim().is_empty()).then(|| DirectedEvolutionConstraintPlan {
                statement,
                kind: nonempty(kind, "human-negotiated".to_string()),
            })
        }
    }
}

fn elimination_rule_plan_from_input(
    rule: DirectedEvolutionEliminationRuleInput,
) -> Option<DirectedEvolutionEliminationRulePlan> {
    if rule.statement.trim().is_empty() {
        None
    } else {
        Some(DirectedEvolutionEliminationRulePlan {
            statement: rule.statement,
            metric_names: rule.metric_names,
            metric_ids: rule.metric_ids,
            threshold: rule.threshold,
        })
    }
}

fn scoring_rule_plan_from_input(
    rule: DirectedEvolutionScoringRuleInput,
) -> Option<DirectedEvolutionScoringRulePlan> {
    (!rule.statement.trim().is_empty()).then(|| DirectedEvolutionScoringRulePlan {
        statement: rule.statement,
        metric_names: rule.metric_names,
        metric_ids: rule.metric_ids,
        weight: nonempty(rule.weight, "1.0".to_string()),
    })
}

fn evaluation_stage_plan_from_input(
    stage: DirectedEvolutionEvaluationStageInput,
) -> Option<DirectedEvolutionEvaluationStagePlan> {
    (!stage.name.trim().is_empty()).then(|| DirectedEvolutionEvaluationStagePlan {
        name: stage.name,
        kind: nonempty(stage.kind, "reviewer".to_string()),
        executor: nonempty(stage.executor, "codex".to_string()),
        required_evidence: stage.required_evidence,
        measurement_provenance: nonempty(stage.measurement_provenance, "brain-judged".to_string()),
        evaluator_ref: stage.evaluator_ref,
        evaluator_module: stage.evaluator_module,
        decision_authority: nonempty(stage.decision_authority, "codex-evaluator".to_string()),
    })
}

fn default_human_episode_metrics() -> Vec<DirectedEvolutionMetricPlan> {
    vec![
        DirectedEvolutionMetricPlan {
            name: "adaptation_goal_satisfaction".to_string(),
            kind: "goal".to_string(),
            unit: "score".to_string(),
            higher_is_better: true,
            description: "How well the variant satisfies the negotiated Adaptation Goal.".to_string(),
            provenance_kind: "brain-judged".to_string(),
            evaluator_ref: String::new(),
            evaluator_module: String::new(),
            interpretation: "Codex evaluator judges against the Adaptation Goal from recorded evidence.".to_string(),
            hard_constraint: false,
        },
        DirectedEvolutionMetricPlan {
            name: "viability_regression_count".to_string(),
            kind: "regression".to_string(),
            unit: "count".to_string(),
            higher_is_better: false,
            description: "Number of negotiated Viability Constraints regressed by the variant.".to_string(),
            provenance_kind: "state-verified".to_string(),
            evaluator_ref: String::new(),
            evaluator_module: "state-verifier".to_string(),
            interpretation: "State/spec evaluator counts regressions against pinned Viability Constraints.".to_string(),
            hard_constraint: true,
        },
        DirectedEvolutionMetricPlan {
            name: "simulated_user_friction".to_string(),
            kind: "journey".to_string(),
            unit: "count".to_string(),
            higher_is_better: false,
            description: "Observed friction points across AI simulated-user journeys.".to_string(),
            provenance_kind: "agent-observed".to_string(),
            evaluator_ref: String::new(),
            evaluator_module: String::new(),
            interpretation: "Simulated users record observations; evaluator decides whether friction is acceptable.".to_string(),
            hard_constraint: false,
        },
        DirectedEvolutionMetricPlan {
            name: "simulated_user_app_blocker_count".to_string(),
            kind: "journey".to_string(),
            unit: "trials".to_string(),
            higher_is_better: false,
            description: "Simulated-user blockers classified as app behavior rather than runtime access.".to_string(),
            provenance_kind: "state-verified".to_string(),
            evaluator_ref: String::new(),
            evaluator_module: "trial-state-verifier".to_string(),
            interpretation: "The router counts terminal Trial entities and only hard-eliminates app-behavior blockers; runtime-access blockers remain visible evidence.".to_string(),
            hard_constraint: true,
        },
        DirectedEvolutionMetricPlan {
            name: "runtime_error_count".to_string(),
            kind: "telemetry".to_string(),
            unit: "count".to_string(),
            higher_is_better: false,
            description: "Runtime errors observed for the variant during evaluation.".to_string(),
            provenance_kind: "datadog-measured".to_string(),
            evaluator_ref: String::new(),
            evaluator_module: String::new(),
            interpretation: "Telemetry evaluator records the Datadog query/window/result count and interprets whether errors are acceptable.".to_string(),
            hard_constraint: true,
        },
    ]
}

fn default_human_episode_constraints(
    proposed_constraints: Vec<String>,
) -> Vec<DirectedEvolutionConstraintPlan> {
    let constraints = if proposed_constraints.is_empty() {
        vec![
            "Preserve existing Agent Answers question and answer workflows.".to_string(),
            "Do not let variants modify evaluators, selection pressure, or viability constraints."
                .to_string(),
        ]
    } else {
        proposed_constraints
    };
    constraints
        .into_iter()
        .enumerate()
        .map(|(index, statement)| DirectedEvolutionConstraintPlan {
            statement,
            kind: if index == 0 {
                "baseline"
            } else {
                "human-negotiated"
            }
            .to_string(),
        })
        .collect()
}

fn default_human_episode_elimination_rules() -> Vec<DirectedEvolutionEliminationRulePlan> {
    vec![DirectedEvolutionEliminationRulePlan {
        statement: "Eliminate variants that fail code/spec review, regress a Viability Constraint, or fail the AI simulated-user trial.".to_string(),
        metric_names: Vec::new(),
        metric_ids: Vec::new(),
        threshold: json!({
            "viability_regression_count": 0,
            "simulated_user_app_blocker_count": 0,
            "runtime_error_count": 0,
        }),
    }]
}

fn default_human_episode_scoring_rules() -> Vec<DirectedEvolutionScoringRulePlan> {
    vec![DirectedEvolutionScoringRulePlan {
        statement: "Prefer the surviving variant with strongest Adaptation Goal satisfaction and no viability regression.".to_string(),
        metric_names: Vec::new(),
        metric_ids: Vec::new(),
        weight: "1.0".to_string(),
    }]
}

fn default_human_episode_evaluation_stages() -> Vec<DirectedEvolutionEvaluationStagePlan> {
    vec![
        DirectedEvolutionEvaluationStagePlan {
            name: "Code and spec review".to_string(),
            kind: "reviewer".to_string(),
            executor: "codex".to_string(),
            required_evidence: vec![
                "changed_files".to_string(),
                "verification_notes".to_string(),
                "viability_constraints".to_string(),
            ],
            measurement_provenance: "brain-judged".to_string(),
            evaluator_ref: String::new(),
            evaluator_module: String::new(),
            decision_authority: "codex-reviewer".to_string(),
        },
        DirectedEvolutionEvaluationStagePlan {
            name: "AI simulated user growth trial".to_string(),
            kind: "simulated_user".to_string(),
            executor: "codex".to_string(),
            required_evidence: vec![
                "simulated_user_trace".to_string(),
                "unmet_intent_observations".to_string(),
                "journey_observations".to_string(),
            ],
            measurement_provenance: "agent-observed".to_string(),
            evaluator_ref: String::new(),
            evaluator_module: String::new(),
            decision_authority: "codex-viability-evaluator-after-trials".to_string(),
        },
        DirectedEvolutionEvaluationStagePlan {
            name: "Runtime telemetry evidence".to_string(),
            kind: "telemetry".to_string(),
            executor: "telemetry_evaluator".to_string(),
            required_evidence: vec![
                "datadog_query_summary".to_string(),
                "runtime_logs_or_metrics".to_string(),
                "zero_result_meaning".to_string(),
            ],
            measurement_provenance: "datadog-measured".to_string(),
            evaluator_ref: String::new(),
            evaluator_module: String::new(),
            decision_authority: "codex-telemetry-evaluator".to_string(),
        },
        DirectedEvolutionEvaluationStagePlan {
            name: "State and spec invariant verification".to_string(),
            kind: "state_verified".to_string(),
            executor: "state_verifier".to_string(),
            required_evidence: vec![
                "ioa_transition_check".to_string(),
                "csdl_field_alignment".to_string(),
            ],
            measurement_provenance: "state-verified".to_string(),
            evaluator_ref: "genesis://nerdsane/agent-answers-evaluation@frozen".to_string(),
            evaluator_module: "state-verifier".to_string(),
            decision_authority: "deterministic-state-verifier".to_string(),
        },
    ]
}

fn default_simulated_user_personas() -> Vec<Value> {
    vec![
        json!({
            "name": "answer seeker",
            "goal_style": "asks a practical question and needs a usable accepted answer",
        }),
        json!({
            "name": "careful reviewer",
            "goal_style": "checks whether citations and constraints remain understandable",
        }),
        json!({
            "name": "returning maintainer",
            "goal_style": "uses prior context and expects answer state to remain consistent",
        }),
    ]
}

fn default_simulated_user_goals() -> Vec<Value> {
    vec![
        "Ask a question, inspect generated answers, and identify whether one can be accepted with understandable evidence.",
        "Review an answer for citation readability and decide whether the app gives enough context to trust it.",
        "Return to an existing question and verify the accepted answer remains visible and usable.",
    ]
    .into_iter()
    .map(Value::from)
    .collect()
}

fn parse_json_string_array(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
}

fn empty_directed_evolution_json_object() -> Value {
    json!({})
}

fn nonempty(value: String, fallback: String) -> String {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}
