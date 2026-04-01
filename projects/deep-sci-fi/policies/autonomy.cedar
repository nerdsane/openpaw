// Deep Sci-Fi Autonomy Policies
// Defines what each role can do autonomously vs. what requires approval.

// ============================================================
// SWE Role — Autonomous Code Operations
// ============================================================

// SWE can create PRs without asking
permit(
    principal in OpenPaw::Role::"SWE",
    action == OpenPaw::Action::"CreatePR",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// SWE can create issues without asking
permit(
    principal in OpenPaw::Role::"SWE",
    action == OpenPaw::Action::"CreateIssue",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// SWE can push branches without asking
permit(
    principal in OpenPaw::Role::"SWE",
    action == OpenPaw::Action::"PushBranch",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// SWE can run tests autonomously
permit(
    principal in OpenPaw::Role::"SWE",
    action == OpenPaw::Action::"RunTests",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// SWE CANNOT merge PRs — requires lead approval
forbid(
    principal in OpenPaw::Role::"SWE",
    action == OpenPaw::Action::"MergePR",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// SWE CANNOT modify deployment configuration
forbid(
    principal in OpenPaw::Role::"SWE",
    action == OpenPaw::Action::"ModifyDeployConfig",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// ============================================================
// SRE Role — Autonomous Monitoring Operations
// ============================================================

// SRE can tune monitors without asking
permit(
    principal in OpenPaw::Role::"SRE",
    action == OpenPaw::Action::"TuneMonitor",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// SRE can create monitors without asking
permit(
    principal in OpenPaw::Role::"SRE",
    action == OpenPaw::Action::"CreateMonitor",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// SRE can delete stale monitors without asking
permit(
    principal in OpenPaw::Role::"SRE",
    action == OpenPaw::Action::"DeleteMonitor",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// SRE can query deployment status
permit(
    principal in OpenPaw::Role::"SRE",
    action == OpenPaw::Action::"QueryDeployStatus",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// SRE must notify Ren on alert escalation (escalation requires notification)
forbid(
    principal in OpenPaw::Role::"SRE",
    action == OpenPaw::Action::"EscalateAlert",
    resource in OpenPaw::Project::"deep-sci-fi"
) unless { context.lead_notified == true };

// SRE CANNOT trigger redeployments
forbid(
    principal in OpenPaw::Role::"SRE",
    action == OpenPaw::Action::"TriggerRedeploy",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// ============================================================
// Design Role — Autonomous Design Review
// ============================================================

// Design can propose UI changes (file issues, comment on PRs)
permit(
    principal in OpenPaw::Role::"Design",
    action == OpenPaw::Action::"ProposeUIChange",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// Design can review PRs and leave comments
permit(
    principal in OpenPaw::Role::"Design",
    action == OpenPaw::Action::"ReviewPR",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// Design CANNOT merge PRs
forbid(
    principal in OpenPaw::Role::"Design",
    action == OpenPaw::Action::"MergePR",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// ============================================================
// Librarian Role — Autonomous Content Analysis
// ============================================================

// Librarian can run content scans autonomously
permit(
    principal in OpenPaw::Role::"Librarian",
    action == OpenPaw::Action::"RunContentScan",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// Librarian can flag content issues
permit(
    principal in OpenPaw::Role::"Librarian",
    action == OpenPaw::Action::"FlagContentIssue",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// Librarian CANNOT modify content directly — reports to Ren
forbid(
    principal in OpenPaw::Role::"Librarian",
    action == OpenPaw::Action::"ModifyContent",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// ============================================================
// Global Restrictions — All Roles
// ============================================================

// No agent can modify deployment config without approval
forbid(
    principal in OpenPaw::Role::"Agent",
    action == OpenPaw::Action::"ModifyDeployConfig",
    resource in OpenPaw::Project::"deep-sci-fi"
) unless { context.human_approved == true };

// No agent can change AI model configuration without approval
forbid(
    principal in OpenPaw::Role::"Agent",
    action == OpenPaw::Action::"ChangeModelConfig",
    resource in OpenPaw::Project::"deep-sci-fi"
) unless { context.human_approved == true };

// No agent can delete production data
forbid(
    principal in OpenPaw::Role::"Agent",
    action == OpenPaw::Action::"DeleteProductionData",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// Only Lead (Ren) can merge PRs
permit(
    principal in OpenPaw::Role::"Lead",
    action == OpenPaw::Action::"MergePR",
    resource in OpenPaw::Project::"deep-sci-fi"
);

// Only Lead can trigger redeployments
permit(
    principal in OpenPaw::Role::"Lead",
    action == OpenPaw::Action::"TriggerRedeploy",
    resource in OpenPaw::Project::"deep-sci-fi"
);
