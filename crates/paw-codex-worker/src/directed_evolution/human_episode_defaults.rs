fn metric_plan_from_input(
    metric: DirectedEvolutionMetricInput,
) -> Option<DirectedEvolutionMetricPlan> {
    (!metric.name.trim().is_empty()).then(|| DirectedEvolutionMetricPlan {
        name: metric.name,
        kind: nonempty(metric.kind, "quality".to_string()),
        unit: nonempty(metric.unit, "score".to_string()),
        higher_is_better: metric.higher_is_better.unwrap_or(true),
        description: nonempty(metric.description, "Human-Codex negotiated metric.".to_string()),
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
        },
        DirectedEvolutionMetricPlan {
            name: "viability_regression_count".to_string(),
            kind: "regression".to_string(),
            unit: "count".to_string(),
            higher_is_better: false,
            description: "Number of negotiated Viability Constraints regressed by the variant.".to_string(),
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
        },
        DirectedEvolutionEvaluationStagePlan {
            name: "AI simulated user growth trial".to_string(),
            kind: "simulated_user".to_string(),
            executor: "codex".to_string(),
            required_evidence: vec![
                "simulated_user_trace".to_string(),
                "unmet_intent_observations".to_string(),
                "datadog_evidence_scope".to_string(),
            ],
        },
    ]
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
