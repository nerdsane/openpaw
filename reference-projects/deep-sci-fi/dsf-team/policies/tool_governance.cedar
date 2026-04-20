// Deep Sci-Fi Tool Governance Policies
// Controls which CLI tools and API endpoints each role can use.
// Evaluated by Cedar-backed ToolHooks before tool execution.

// ============================================================
// SWE Role — GitHub CLI Tools
// ============================================================

// SWE can create PRs
permit(
    principal in TemperPaw::Role::"SWE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"gh"
) when { context.subcommand == "pr create" };

// SWE can create issues
permit(
    principal in TemperPaw::Role::"SWE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"gh"
) when { context.subcommand == "issue create" };

// SWE can view workflow runs
permit(
    principal in TemperPaw::Role::"SWE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"gh"
) when { context.subcommand == "run view" };

// SWE can view PR status
permit(
    principal in TemperPaw::Role::"SWE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"gh"
) when { context.subcommand == "pr view" };

// SWE can list PRs
permit(
    principal in TemperPaw::Role::"SWE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"gh"
) when { context.subcommand == "pr list" };

// SWE CANNOT merge PRs via gh
forbid(
    principal in TemperPaw::Role::"SWE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"gh"
) when { context.subcommand == "pr merge" };

// SWE CANNOT delete repos
forbid(
    principal in TemperPaw::Role::"SWE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"gh"
) when { context.subcommand == "repo delete" };

// ============================================================
// SRE Role — GitHub CLI Tools
// ============================================================

// SRE can view workflow runs
permit(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"gh"
) when { context.subcommand == "run view" };

// SRE can view PRs
permit(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"gh"
) when { context.subcommand == "pr view" };

// SRE can list workflow runs
permit(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"gh"
) when { context.subcommand == "run list" };

// SRE CANNOT merge PRs
forbid(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"gh"
) when { context.subcommand == "pr merge" };

// ============================================================
// All Roles — Dangerous Command Blocks
// ============================================================

// Block rm -rf for all agents
forbid(
    principal in TemperPaw::Role::"Agent",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"bash"
) when { context.command like "*rm -rf*" };

// Block git push --force to main for all agents
forbid(
    principal in TemperPaw::Role::"Agent",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"git"
) when { context.command like "*push --force*main*" };

// Block git push -f to main for all agents
forbid(
    principal in TemperPaw::Role::"Agent",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"git"
) when { context.command like "*push -f*main*" };

// Block dropping databases
forbid(
    principal in TemperPaw::Role::"Agent",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"bash"
) when { context.command like "*DROP DATABASE*" };

// Block dropping all tables
forbid(
    principal in TemperPaw::Role::"Agent",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"bash"
) when { context.command like "*DROP TABLE*" };

// ============================================================
// Railway API — Infrastructure Tools
// ============================================================

// SRE can query Railway deployment status
permit(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"railway_api"
) when { context.operation == "query_deployment_status" };

// SRE can view Railway service logs
permit(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"railway_api"
) when { context.operation == "view_logs" };

// Only Lead can trigger Railway redeployment
permit(
    principal in TemperPaw::Role::"Lead",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"railway_api"
) when { context.operation == "trigger_redeploy" };

// Block all other Railway operations for non-Lead roles
forbid(
    principal in TemperPaw::Role::"Agent",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"railway_api"
) when { context.operation == "trigger_redeploy" }
unless { principal in TemperPaw::Role::"Lead" };

// ============================================================
// Vercel API — Frontend Deployment Tools
// ============================================================

// SRE can query Vercel deployment status
permit(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"vercel_api"
) when { context.operation == "query_deployment_status" };

// SRE can view Vercel deployment logs
permit(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"vercel_api"
) when { context.operation == "view_logs" };

// Only Lead can trigger Vercel redeployment
permit(
    principal in TemperPaw::Role::"Lead",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"vercel_api"
) when { context.operation == "trigger_redeploy" };

// Block Vercel redeploy for non-Lead roles
forbid(
    principal in TemperPaw::Role::"Agent",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"vercel_api"
) when { context.operation == "trigger_redeploy" }
unless { principal in TemperPaw::Role::"Lead" };

// ============================================================
// Datadog API — Monitoring Tools
// ============================================================

// SRE can use all datadog_query operations
permit(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"datadog_query"
);

// Lead can use all datadog_query operations
permit(
    principal in TemperPaw::Role::"Lead",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"datadog_query"
);

// SRE can create and modify Datadog monitors
permit(
    principal in TemperPaw::Role::"SRE",
    action == TemperPaw::Action::"ExecuteTool",
    resource == TemperPaw::Tool::"datadog_monitor"
);
