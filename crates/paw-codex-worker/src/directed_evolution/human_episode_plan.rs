#[derive(Clone, Debug, Deserialize)]
struct DirectedEvolutionHumanEpisodeInput {
    #[serde(default, alias = "DirectionId", alias = "directionId")]
    direction_id: String,
    #[serde(default, alias = "OrganismId", alias = "organismId")]
    organism_id: String,
    #[serde(default, alias = "ParentVersionId", alias = "parentVersionId")]
    parent_version_id: String,
    #[serde(default, alias = "AutonomyLane", alias = "autonomyLane")]
    autonomy_lane: String,
    #[serde(default, alias = "AdaptationGoal", alias = "adaptationGoal")]
    adaptation_goal: String,
    #[serde(default, alias = "HumanNotes", alias = "humanNotes")]
    human_notes: String,
    #[serde(default, alias = "CreatedByBrainRunId", alias = "createdByBrainRunId")]
    created_by_brain_run_id: String,
    #[serde(default, alias = "Metrics", alias = "metricDefinitions")]
    metrics: Vec<DirectedEvolutionMetricInput>,
    #[serde(default, alias = "ViabilityConstraints", alias = "viabilityConstraints")]
    viability_constraints: Vec<DirectedEvolutionConstraintInput>,
    #[serde(default, alias = "SelectionStatement", alias = "selectionStatement")]
    selection_statement: String,
    #[serde(default, alias = "EliminationRules", alias = "eliminationRules")]
    elimination_rules: Vec<DirectedEvolutionEliminationRuleInput>,
    #[serde(default, alias = "ScoringRules", alias = "scoringRules")]
    scoring_rules: Vec<DirectedEvolutionScoringRuleInput>,
    #[serde(default, alias = "EvaluationStages", alias = "evaluationStages")]
    evaluation_stages: Vec<DirectedEvolutionEvaluationStageInput>,
    #[serde(default, alias = "SelectedBy", alias = "selectedBy")]
    selected_by: String,
    #[serde(default, alias = "SelectionNotes", alias = "selectionNotes")]
    selection_notes: String,
    #[serde(default, alias = "StartedBy", alias = "startedBy")]
    started_by: String,
    #[serde(default, alias = "StartReason", alias = "startReason")]
    start_reason: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum DirectedEvolutionConstraintInput {
    Text(String),
    Object {
        #[serde(default, alias = "ConstraintStatement", alias = "constraintStatement")]
        statement: String,
        #[serde(default, alias = "ConstraintKind", alias = "constraintKind")]
        kind: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
struct DirectedEvolutionMetricInput {
    #[serde(default, alias = "MetricName", alias = "metricName")]
    name: String,
    #[serde(default, alias = "MetricKind", alias = "metricKind")]
    kind: String,
    #[serde(default, alias = "Unit")]
    unit: String,
    #[serde(default, alias = "HigherIsBetter", alias = "higherIsBetter")]
    higher_is_better: Option<bool>,
    #[serde(default, alias = "Description")]
    description: String,
}

#[derive(Clone, Debug, Deserialize)]
struct DirectedEvolutionEliminationRuleInput {
    #[serde(default, alias = "RuleStatement", alias = "ruleStatement")]
    statement: String,
    #[serde(default, alias = "MetricNames", alias = "metricNames")]
    metric_names: Vec<String>,
    #[serde(default, alias = "MetricIds", alias = "metricIds")]
    metric_ids: Vec<String>,
    #[serde(
        default = "empty_directed_evolution_json_object",
        alias = "Threshold",
        alias = "threshold"
    )]
    threshold: Value,
}

#[derive(Clone, Debug, Deserialize)]
struct DirectedEvolutionScoringRuleInput {
    #[serde(default, alias = "RuleStatement", alias = "ruleStatement")]
    statement: String,
    #[serde(default, alias = "MetricNames", alias = "metricNames")]
    metric_names: Vec<String>,
    #[serde(default, alias = "MetricIds", alias = "metricIds")]
    metric_ids: Vec<String>,
    #[serde(default, alias = "Weight")]
    weight: String,
}

#[derive(Clone, Debug, Deserialize)]
struct DirectedEvolutionEvaluationStageInput {
    #[serde(default, alias = "StageName", alias = "stageName")]
    name: String,
    #[serde(default, alias = "StageKind", alias = "stageKind")]
    kind: String,
    #[serde(default, alias = "ExecutorKind", alias = "executorKind")]
    executor: String,
    #[serde(default, alias = "RequiredEvidence", alias = "requiredEvidence")]
    required_evidence: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DirectedEvolutionEpisodePlan {
    direction_id: String,
    organism_id: String,
    parent_version_id: String,
    autonomy_lane: String,
    adaptation_goal: String,
    human_notes: String,
    created_by_brain_run_id: String,
    metrics: Vec<DirectedEvolutionMetricPlan>,
    viability_constraints: Vec<DirectedEvolutionConstraintPlan>,
    selection_statement: String,
    elimination_rules: Vec<DirectedEvolutionEliminationRulePlan>,
    scoring_rules: Vec<DirectedEvolutionScoringRulePlan>,
    evaluation_stages: Vec<DirectedEvolutionEvaluationStagePlan>,
    selected_by: String,
    selection_notes: String,
    started_by: String,
    start_reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DirectedEvolutionMetricPlan {
    name: String,
    kind: String,
    unit: String,
    higher_is_better: bool,
    description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DirectedEvolutionConstraintPlan {
    statement: String,
    kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DirectedEvolutionEliminationRulePlan {
    statement: String,
    metric_names: Vec<String>,
    metric_ids: Vec<String>,
    threshold: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DirectedEvolutionScoringRulePlan {
    statement: String,
    metric_names: Vec<String>,
    metric_ids: Vec<String>,
    weight: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DirectedEvolutionEvaluationStagePlan {
    name: String,
    kind: String,
    executor: String,
    required_evidence: Vec<String>,
}

struct DirectedEvolutionSelectionInputs<'a> {
    metric_ids: &'a [String],
    elimination_rule_ids: &'a [String],
    scoring_rule_ids: &'a [String],
}

fn directed_evolution_episode_plan_from_input(
    input: DirectedEvolutionHumanEpisodeInput,
    direction_fields: &Value,
    organism_fields: &Value,
) -> Result<DirectedEvolutionEpisodePlan> {
    if input.direction_id.trim().is_empty() {
        bail!("Directed Evolution episode contract requires direction_id");
    }
    let organism_id = nonempty(
        input.organism_id,
        value_field_string(direction_fields, &["OrganismId", "organism_id"]),
    );
    let parent_version_id = nonempty(
        input.parent_version_id,
        nonempty(
            value_field_string(organism_fields, &["OrganismVersionId", "organism_version_id"]),
            value_field_string(organism_fields, &["ParentVersionId", "parent_version_id"]),
        ),
    );
    if parent_version_id.trim().is_empty() {
        bail!("Directed Evolution episode contract needs parent_version_id or an Organism with OrganismVersionId/ParentVersionId");
    }

    let proposed_constraints = parse_json_string_array(&value_field_string(
        direction_fields,
        &[
            "ProposedViabilityConstraintsJson",
            "proposed_viability_constraints_json",
        ],
    ));
    let mut metrics = if input.metrics.is_empty() {
        default_human_episode_metrics()
    } else {
        input
            .metrics
            .into_iter()
            .filter_map(metric_plan_from_input)
            .collect()
    };
    if metrics.is_empty() {
        metrics = default_human_episode_metrics();
    }
    let mut viability_constraints = if input.viability_constraints.is_empty() {
        default_human_episode_constraints(proposed_constraints)
    } else {
        input
            .viability_constraints
            .into_iter()
            .filter_map(constraint_plan_from_input)
            .collect()
    };
    if viability_constraints.is_empty() {
        viability_constraints = default_human_episode_constraints(Vec::new());
    }
    let mut elimination_rules = if input.elimination_rules.is_empty() {
        default_human_episode_elimination_rules()
    } else {
        input
            .elimination_rules
            .into_iter()
            .filter_map(elimination_rule_plan_from_input)
            .collect()
    };
    if elimination_rules.is_empty() {
        elimination_rules = default_human_episode_elimination_rules();
    }
    let mut scoring_rules = if input.scoring_rules.is_empty() {
        default_human_episode_scoring_rules()
    } else {
        input
            .scoring_rules
            .into_iter()
            .filter_map(scoring_rule_plan_from_input)
            .collect()
    };
    if scoring_rules.is_empty() {
        scoring_rules = default_human_episode_scoring_rules();
    }
    let mut evaluation_stages = if input.evaluation_stages.is_empty() {
        default_human_episode_evaluation_stages()
    } else {
        input
            .evaluation_stages
            .into_iter()
            .filter_map(evaluation_stage_plan_from_input)
            .collect()
    };
    if evaluation_stages.is_empty() {
        evaluation_stages = default_human_episode_evaluation_stages();
    }
    let adaptation_goal = nonempty(
        input.adaptation_goal,
        value_field_string(
            direction_fields,
            &["ProposedAdaptationGoal", "proposed_adaptation_goal"],
        ),
    );
    if adaptation_goal.trim().is_empty() {
        bail!("Directed Evolution episode contract requires adaptation_goal or a Direction with ProposedAdaptationGoal");
    }

    Ok(DirectedEvolutionEpisodePlan {
        direction_id: input.direction_id,
        organism_id,
        parent_version_id,
        autonomy_lane: nonempty(
            input.autonomy_lane,
            nonempty(
                value_field_string(direction_fields, &["AutonomyLane", "autonomy_lane"]),
                "growth-human-gated".to_string(),
            ),
        ),
        adaptation_goal,
        human_notes: nonempty(
            input.human_notes,
            "Human and Codex agreed to this episode contract in chat.".to_string(),
        ),
        created_by_brain_run_id: nonempty(
            input.created_by_brain_run_id,
            env::var("CODEX_SESSION_ID").unwrap_or_else(|_| "chat-codex".to_string()),
        ),
        metrics,
        viability_constraints,
        selection_statement: nonempty(
            input.selection_statement,
            "Prefer variants that satisfy the Adaptation Goal while preserving every Viability Constraint.".to_string(),
        ),
        elimination_rules,
        scoring_rules,
        evaluation_stages,
        selected_by: nonempty(input.selected_by, "human+codex-chat".to_string()),
        selection_notes: nonempty(
            input.selection_notes,
            "Selected after human-brain negotiation in Codex chat.".to_string(),
        ),
        started_by: nonempty(input.started_by, "codex".to_string()),
        start_reason: nonempty(
            input.start_reason,
            "Start the human-directed growth episode using the negotiated contract.".to_string(),
        ),
    })
}
