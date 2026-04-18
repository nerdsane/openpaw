// Deep Sci-Fi Autonomy Policies
// Defines what each role can do autonomously vs. what requires approval.

// ============================================================
// SWE Role — Autonomous Code Operations
// ============================================================

// SWE can create PRs without asking
permit(
    principal in TemperPaw::Role::"SWE",
    action == TemperPaw::Action::"CreatePR",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// SWE can create issues without asking
permit(
    principal in TemperPaw::Role::"SWE",
    action == TemperPaw::Action::"CreateIssue",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// SWE can push branches without asking
permit(
    principal in TemperPaw::Role::"SWE",
    action == TemperPaw::Action::"PushBranch",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// SWE can run tests autonomously
permit(
    principal in TemperPaw::Role::"SWE",
    action == TemperPaw::Action::"RunTests",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// SWE CANNOT merge PRs — requires lead approval
forbid(
    principal in TemperPaw::Role::"SWE",
    action == TemperPaw::Action::"MergePR",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// SWE CANNOT modify deployment configuration
forbid(
    principal in TemperPaw::Role::"SWE",
    action == TemperPaw::Action::"ModifyDeployConfig",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// ============================================================
// SRE Role — Autonomous Monitoring Operations
// ============================================================

// SRE can tune monitors without asking
permit(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"TuneMonitor",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// SRE can create monitors without asking
permit(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"CreateMonitor",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// SRE can delete stale monitors without asking
permit(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"DeleteMonitor",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// SRE can query deployment status
permit(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"QueryDeployStatus",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// SRE must notify Ren on alert escalation (escalation requires notification)
forbid(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"EscalateAlert",
    resource in TemperPaw::Project::"deep-sci-fi"
) unless { context.lead_notified == true };

// SRE CANNOT trigger redeployments
forbid(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"TriggerRedeploy",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// ============================================================
// Design Role — Autonomous Design Review
// ============================================================

// Design can propose UI changes (file issues, comment on PRs)
permit(
    principal in TemperPaw::Role::"Design",
    action == TemperPaw::Action::"ProposeUIChange",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// Design can review PRs and leave comments
permit(
    principal in TemperPaw::Role::"Design",
    action == TemperPaw::Action::"ReviewPR",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// Design CANNOT merge PRs
forbid(
    principal in TemperPaw::Role::"Design",
    action == TemperPaw::Action::"MergePR",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// ============================================================
// Librarian Role — Autonomous Content Analysis
// ============================================================

// Librarian can run content scans autonomously
permit(
    principal in TemperPaw::Role::"Librarian",
    action == TemperPaw::Action::"RunContentScan",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// Librarian can flag content issues
permit(
    principal in TemperPaw::Role::"Librarian",
    action == TemperPaw::Action::"FlagContentIssue",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// Librarian CANNOT modify content directly — reports to Ren
forbid(
    principal in TemperPaw::Role::"Librarian",
    action == TemperPaw::Action::"ModifyContent",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// ============================================================
// Global Restrictions — All Roles
// ============================================================

// No agent can modify deployment config without approval
forbid(
    principal in TemperPaw::Role::"Agent",
    action == TemperPaw::Action::"ModifyDeployConfig",
    resource in TemperPaw::Project::"deep-sci-fi"
) unless { context.human_approved == true };

// No agent can change AI model configuration without approval
forbid(
    principal in TemperPaw::Role::"Agent",
    action == TemperPaw::Action::"ChangeModelConfig",
    resource in TemperPaw::Project::"deep-sci-fi"
) unless { context.human_approved == true };

// No agent can delete production data
forbid(
    principal in TemperPaw::Role::"Agent",
    action == TemperPaw::Action::"DeleteProductionData",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// Only Lead (Ren) can merge PRs
permit(
    principal in TemperPaw::Role::"Lead",
    action == TemperPaw::Action::"MergePR",
    resource in TemperPaw::Project::"deep-sci-fi"
);

// Only Lead can trigger redeployments
permit(
    principal in TemperPaw::Role::"Lead",
    action == TemperPaw::Action::"TriggerRedeploy",
    resource in TemperPaw::Project::"deep-sci-fi"
);
