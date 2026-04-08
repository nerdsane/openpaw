/**
 * Mock data representing a fully populated OpenPaw instance.
 *
 * GENERIC — not app-specific. Every entity is the same shape:
 * { Id, Status, _entity_type, _app, fields, _events, _sequence_nr }
 *
 * The UI renders entities generically by type, status, and transitions.
 * New apps with new entity types appear automatically.
 */

export interface MockEntity {
  Id: string;
  Status: string;
  _entity_type: string;
  _app: string; // which os-app owns this entity type
  fields: Record<string, unknown>;
  _events: Array<{
    action: string;
    from_status: string;
    to_status: string;
    timestamp: string;
    params?: Record<string, unknown>;
  }>;
  _sequence_nr: number;
}

export interface MockDecision {
  id: string;
  tenant: string;
  agent_id: string;
  action: string;
  resource_type: string;
  resource_id: string;
  denial_reason: string;
  module_name: string | null;
  agent_type: string;
  created_at: string;
  status: 'pending' | 'approved' | 'denied';
  decided_by: string | null;
  decided_at: string | null;
  generated_policy: string | null;
}

export interface MockPolicy {
  policy_id: string;
  tenant: string;
  cedar_text: string;
  enabled: boolean;
  source: string;
  created_by: string;
}

export interface MockAuthzEvent {
  timestamp: string;
  entity_type: string;
  entity_id: string;
  action: string;
  success: boolean;
  from_status: string;
  to_status: string;
  authz_denied: boolean;
  denied_resource: string | null;
  matched_policy_ids: string[] | null;
}

export interface MockOsApp {
  name: string;
  description: string;
  entity_types: string[];
  version: string;
  installed: boolean;
}

const now = Date.now();
const ago = (ms: number) => new Date(now - ms).toISOString();

// ═══════════════════════════════════════════════════════════════
// ENTITIES — all generic, grouped by app for readability only
// ═══════════════════════════════════════════════════════════════

export const MOCK_ENTITIES: MockEntity[] = [

  // ──── paw-agent: Projects ────
  { Id: 'proj-dsf', Status: 'Active', _entity_type: 'Project', _app: 'paw-agent', fields: { name: 'Deep Sci-Fi', description: 'AI agent team for the Deep Sci-Fi social platform', owner_agent_id: 'agent-ren', app_ids: 'paw-harness,paw-heal,paw-pm,paw-foresight' }, _events: [], _sequence_nr: 1 },
  { Id: 'proj-koto', Status: 'Active', _entity_type: 'Project', _app: 'paw-agent', fields: { name: 'Kotowari', description: 'AI teaching platform — adaptive learning', owner_agent_id: 'agent-sensei', app_ids: 'paw-pm' }, _events: [], _sequence_nr: 1 },

  // ──── paw-agent: Souls ────
  { Id: 'soul-paw', Status: 'Active', _entity_type: 'Soul', _app: 'paw-agent', fields: { Name: 'Paw', Description: 'Chief of staff — creates project leads, coordinates teams' }, _events: [], _sequence_nr: 2 },
  { Id: 'soul-swe', Status: 'Active', _entity_type: 'Soul', _app: 'paw-agent', fields: { Name: 'SWE', Description: 'Software engineer agent' }, _events: [], _sequence_nr: 2 },
  { Id: 'soul-sre', Status: 'Active', _entity_type: 'Soul', _app: 'paw-agent', fields: { Name: 'SRE', Description: 'Site reliability engineering agent' }, _events: [], _sequence_nr: 2 },
  { Id: 'soul-probe', Status: 'Active', _entity_type: 'Soul', _app: 'paw-agent', fields: { Name: 'Probe', Description: 'Foresight probe — projects product futures' }, _events: [], _sequence_nr: 2 },
  { Id: 'soul-convergence', Status: 'Active', _entity_type: 'Soul', _app: 'paw-agent', fields: { Name: 'Convergence Analyst', Description: 'Synthesizes observations into directions' }, _events: [], _sequence_nr: 2 },
  { Id: 'soul-ren', Status: 'Active', _entity_type: 'Soul', _app: 'paw-agent', fields: { Name: 'Ren', Description: 'Product lead for Deep Sci-Fi (INTP)' }, _events: [], _sequence_nr: 2 },

  // ──── paw-agent: Skills ────
  { Id: 'skill-openpaw-agent', Status: 'Active', _entity_type: 'Skill', _app: 'paw-agent', fields: { name: 'openpaw-agent', description: 'Platform operating manual', scope: 'global', project_id: '' }, _events: [], _sequence_nr: 2 },
  { Id: 'skill-platform-awareness', Status: 'Active', _entity_type: 'Skill', _app: 'paw-agent', fields: { name: 'platform-awareness', description: 'Entity type discovery', scope: 'global', project_id: '' }, _events: [], _sequence_nr: 2 },
  { Id: 'skill-swe-conv', Status: 'Active', _entity_type: 'Skill', _app: 'paw-agent', fields: { name: 'swe-conventions', description: 'SWE coding conventions', scope: 'SWE', project_id: 'proj-dsf' }, _events: [], _sequence_nr: 2 },
  { Id: 'skill-sre-mon', Status: 'Active', _entity_type: 'Skill', _app: 'paw-agent', fields: { name: 'sre-monitoring', description: 'Datadog monitoring and alert triage', scope: 'SRE', project_id: 'proj-dsf' }, _events: [], _sequence_nr: 2 },
  { Id: 'skill-design', Status: 'Active', _entity_type: 'Skill', _app: 'paw-agent', fields: { name: 'design-system', description: 'Neo-editorial design tokens', scope: 'Design', project_id: 'proj-dsf' }, _events: [], _sequence_nr: 2 },

  // ──── paw-agent: Team ────
  { Id: 'team-dsf', Status: 'Active', _entity_type: 'Team', _app: 'paw-agent', fields: { name: 'Deep Sci-Fi Team', description: '7-role AI agent team', harness_id: 'harness-dsf', project_id: 'proj-dsf' }, _events: [], _sequence_nr: 2 },

  // ──── paw-agent: Agents ────
  { Id: 'agent-ren', Status: 'Active', _entity_type: 'Agent', _app: 'paw-agent', fields: { name: 'Ren', role: 'Lead', soul_id: 'soul-ren', team_id: 'team-dsf', model: 'claude-sonnet-4-20250514', provider: 'anthropic', tools_enabled: 'bash,edit,write,read,temper_action' }, _events: [], _sequence_nr: 3 },
  { Id: 'agent-swe', Status: 'Active', _entity_type: 'Agent', _app: 'paw-agent', fields: { name: 'SWE', role: 'SWE', soul_id: 'soul-swe', team_id: 'team-dsf', model: 'claude-sonnet-4-20250514', provider: 'anthropic', tools_enabled: 'bash,edit,write,read,temper_action' }, _events: [], _sequence_nr: 3 },
  { Id: 'agent-sre', Status: 'Active', _entity_type: 'Agent', _app: 'paw-agent', fields: { name: 'SRE', role: 'SRE', soul_id: 'soul-sre', team_id: 'team-dsf', model: 'claude-sonnet-4-20250514', provider: 'anthropic', tools_enabled: 'bash,temper_action' }, _events: [], _sequence_nr: 3 },
  { Id: 'agent-design', Status: 'Active', _entity_type: 'Agent', _app: 'paw-agent', fields: { name: 'Design', role: 'Design', soul_id: '', team_id: 'team-dsf', model: 'claude-sonnet-4-20250514', provider: 'anthropic' }, _events: [], _sequence_nr: 3 },
  { Id: 'agent-librarian', Status: 'Active', _entity_type: 'Agent', _app: 'paw-agent', fields: { name: 'Librarian', role: 'Librarian', soul_id: '', team_id: 'team-dsf', model: 'claude-sonnet-4-20250514', provider: 'anthropic' }, _events: [], _sequence_nr: 3 },
  { Id: 'agent-codereview', Status: 'Active', _entity_type: 'Agent', _app: 'paw-agent', fields: { name: 'CodeReviewer', role: 'CodeReviewer', soul_id: '', team_id: 'team-dsf', model: 'claude-sonnet-4-20250514', provider: 'anthropic' }, _events: [], _sequence_nr: 3 },
  { Id: 'agent-dstreview', Status: 'Active', _entity_type: 'Agent', _app: 'paw-agent', fields: { name: 'DSTReviewer', role: 'DSTReviewer', soul_id: '', team_id: 'team-dsf', model: 'claude-sonnet-4-20250514', provider: 'anthropic' }, _events: [], _sequence_nr: 3 },

  // ──── paw-agent: Sessions (ACTIVE WORK) ────
  { Id: 'sess-swe-1', Status: 'Executing', _entity_type: 'Session', _app: 'paw-agent', fields: { agent_id: 'agent-swe', soul_id: 'soul-swe', project_id: 'proj-dsf', model: 'claude-sonnet-4-20250514', turn_count: 14, last_heartbeat_at: ago(30000), user_message: 'Implement the world proposal voting endpoint. Follow SWE conventions. Create migration, endpoint, tests. Report gates.', pending_decision_id: '' }, _events: [
    { action: 'ProcessToolCalls', from_status: 'Thinking', to_status: 'Executing', timestamp: ago(500000), params: { input_tokens: 4200, output_tokens: 180, pending_tool_calls: '[{"name":"bash","input":{"command":"alembic revision --autogenerate -m \\"add world_proposals\\""}}]' } },
    { action: 'ProcessToolCalls', from_status: 'Thinking', to_status: 'Executing', timestamp: ago(400000), params: { input_tokens: 6800, output_tokens: 320, pending_tool_calls: '[{"name":"write","input":{"file_path":"platform/backend/app/api/proposals.py"}}]' } },
    { action: 'ProcessToolCalls', from_status: 'Thinking', to_status: 'Executing', timestamp: ago(300000), params: { input_tokens: 8900, output_tokens: 250, pending_tool_calls: '[{"name":"write","input":{"file_path":"platform/backend/tests/test_proposals.py"}}]' } },
    { action: 'ProcessToolCalls', from_status: 'Thinking', to_status: 'Executing', timestamp: ago(200000), params: { input_tokens: 11200, output_tokens: 150, pending_tool_calls: '[{"name":"bash","input":{"command":"pytest tests/test_proposals.py -x -q"}}]' } },
    { action: 'ProcessToolCalls', from_status: 'Thinking', to_status: 'Executing', timestamp: ago(100000), params: { input_tokens: 13500, output_tokens: 200, pending_tool_calls: '[{"name":"temper_action","input":{"entity_set":"WorkCycles","entity_id":"wc-1","action":"ReportUnitTests"}}]' } },
    { action: 'ProcessToolCalls', from_status: 'Thinking', to_status: 'Executing', timestamp: ago(30000), params: { input_tokens: 15800, output_tokens: 180, pending_tool_calls: '[{"name":"bash","input":{"command":"bun run typecheck"}}]' } },
  ], _sequence_nr: 29 },

  { Id: 'sess-sre-1', Status: 'Thinking', _entity_type: 'Session', _app: 'paw-agent', fields: { agent_id: 'agent-sre', soul_id: 'soul-sre', project_id: 'proj-dsf', model: 'claude-sonnet-4-20250514', turn_count: 6, last_heartbeat_at: ago(15000), user_message: 'Alert: error_rate_5xx crossed 5% threshold. Triage and fix.', pending_decision_id: '' }, _events: [
    { action: 'ProcessToolCalls', from_status: 'Thinking', to_status: 'Executing', timestamp: ago(200000), params: { input_tokens: 3800, output_tokens: 120, pending_tool_calls: '[{"name":"bash","input":{"command":"curl -s https://api.datadoghq.com/api/v1/query..."}}]' } },
    { action: 'ProcessToolCalls', from_status: 'Thinking', to_status: 'Executing', timestamp: ago(100000), params: { input_tokens: 5200, output_tokens: 90, pending_tool_calls: '[{"name":"bash","input":{"command":"railway logs --project deep-sci-fi --last 50"}}]' } },
    { action: 'ProcessToolCalls', from_status: 'Thinking', to_status: 'Executing', timestamp: ago(15000), params: { input_tokens: 7600, output_tokens: 200, pending_tool_calls: '[{"name":"temper_action","input":{"entity_set":"AlertCycles","entity_id":"alert-1","action":"RecordDiagnosis"}}]' } },
  ], _sequence_nr: 13 },

  { Id: 'sess-ren-1', Status: 'WaitingForApproval', _entity_type: 'Session', _app: 'paw-agent', fields: { agent_id: 'agent-ren', soul_id: 'soul-ren', project_id: 'proj-dsf', model: 'claude-sonnet-4-20250514', turn_count: 8, last_heartbeat_at: ago(60000), user_message: 'Review work cycle WC-42. Check tests, review PR, approve or request changes.', pending_decision_id: 'pd-123', pending_tool_context: 'Awaiting Cedar approval to merge PR #47' }, _events: [], _sequence_nr: 17 },

  { Id: 'sess-probe-1', Status: 'Completed', _entity_type: 'Session', _app: 'paw-agent', fields: { agent_id: '', soul_id: 'Probe', turn_count: 9, last_heartbeat_at: ago(3600000), result: '5 observations about content moderation scaling', user_message: 'Project forward: What happens to world coherence as proposals scale beyond 10K?', parent_session_id: 'sess-projection-1' }, _events: [], _sequence_nr: 19 },
  { Id: 'sess-probe-2', Status: 'Completed', _entity_type: 'Session', _app: 'paw-agent', fields: { agent_id: '', soul_id: 'Probe', turn_count: 7, last_heartbeat_at: ago(3500000), result: '4 observations about real-time collaboration bottlenecks', user_message: 'Project forward: How does foresight viz perform under 50+ concurrent users?', parent_session_id: 'sess-projection-1' }, _events: [], _sequence_nr: 15 },

  // ──── Kotowari project ────
  { Id: 'team-koto', Status: 'Active', _entity_type: 'Team', _app: 'paw-agent', fields: { name: 'Kotowari Team', description: 'AI teaching platform — adaptive learning', harness_id: '', project_id: 'proj-koto' }, _events: [], _sequence_nr: 2 },
  { Id: 'agent-sensei', Status: 'Active', _entity_type: 'Agent', _app: 'paw-agent', fields: { name: 'Sensei', role: 'Instructor', soul_id: '', team_id: 'team-koto', model: 'claude-sonnet-4-20250514', provider: 'anthropic', tools_enabled: 'bash,write,read,temper_action' }, _events: [], _sequence_nr: 3 },
  { Id: 'agent-curator', Status: 'Active', _entity_type: 'Agent', _app: 'paw-agent', fields: { name: 'Curator', role: 'Content', soul_id: '', team_id: 'team-koto', model: 'claude-sonnet-4-20250514', provider: 'anthropic', tools_enabled: 'bash,read,temper_action' }, _events: [], _sequence_nr: 3 },
  { Id: 'agent-assessor', Status: 'Active', _entity_type: 'Agent', _app: 'paw-agent', fields: { name: 'Assessor', role: 'Testing', soul_id: '', team_id: 'team-koto', model: 'claude-sonnet-4-20250514', provider: 'anthropic', tools_enabled: 'bash,read,temper_action' }, _events: [], _sequence_nr: 3 },
  { Id: 'sess-sensei-1', Status: 'Executing', _entity_type: 'Session', _app: 'paw-agent', fields: { agent_id: 'agent-sensei', soul_id: '', project_id: 'proj-koto', model: 'claude-sonnet-4-20250514', turn_count: 5, last_heartbeat_at: ago(20000), user_message: 'Build lesson 3: introduction to recursion with visual tree diagrams', pending_decision_id: '' }, _events: [
    { action: 'ProcessToolCalls', from_status: 'Thinking', to_status: 'Executing', timestamp: ago(100000), params: { input_tokens: 3200, output_tokens: 280, pending_tool_calls: '[{"name":"write","input":{"file_path":"lessons/03-recursion/content.md"}}]' } },
    { action: 'ProcessToolCalls', from_status: 'Thinking', to_status: 'Executing', timestamp: ago(60000), params: { input_tokens: 4800, output_tokens: 190, pending_tool_calls: '[{"name":"temper_action","input":{"entity_set":"Concepts","entity_id":"concept-recursion","action":"LinkPrerequisite"}}]' } },
    { action: 'ProcessToolCalls', from_status: 'Thinking', to_status: 'Executing', timestamp: ago(20000), params: { input_tokens: 6100, output_tokens: 150, pending_tool_calls: '[{"name":"write","input":{"file_path":"lessons/03-recursion/exercises.md"}}]' } },
  ], _sequence_nr: 11 },

  // ──── paw-agent: Memories ────
  { Id: 'mem-swe-1', Status: 'Active', _entity_type: 'Memory', _app: 'paw-agent', fields: { agent_id: 'agent-swe', key: 'alembic-pattern', content: 'Always use create_type=False for PostgreSQL ENUMs', memory_type: 'feedback' }, _events: [], _sequence_nr: 1 },
  { Id: 'mem-ren-1', Status: 'Active', _entity_type: 'Memory', _app: 'paw-agent', fields: { agent_id: 'agent-ren', key: 'pr-process', content: 'Only Ren or the human can merge PRs', memory_type: 'feedback' }, _events: [], _sequence_nr: 1 },

  // ──── paw-agent: CronJobs ────
  { Id: 'cron-heartbeat', Status: 'Active', _entity_type: 'CronJob', _app: 'paw-agent', fields: { schedule: '*/5 * * * *', soul_id: '', user_message_template: 'Scan for stale sessions', next_run_at: new Date(now + 120000).toISOString(), run_count: 288 }, _events: [], _sequence_nr: 289 },
  { Id: 'cron-daily-report', Status: 'Active', _entity_type: 'CronJob', _app: 'paw-agent', fields: { schedule: '0 9 * * *', soul_id: 'soul-ren', user_message_template: 'Daily status report for Deep Sci-Fi', next_run_at: new Date(now + 43200000).toISOString(), run_count: 14 }, _events: [], _sequence_nr: 15 },

  // ──── paw-harness: Harness + WorkCycles ────
  { Id: 'harness-dsf', Status: 'Active', _entity_type: 'ProjectHarness', _app: 'paw-harness', fields: { repo_url: 'https://github.com/arni-labs/deep-sci-fi.git', tech_stack: 'Next.js 14, TypeScript, Tailwind, Supabase, Python', project_name: 'Deep Sci-Fi', work_cycle_type: 'WorkCycles' }, _events: [], _sequence_nr: 2 },

  { Id: 'wc-1', Status: 'Testing', _entity_type: 'WorkCycle', _app: 'paw-harness', fields: { task_summary: 'World proposal voting endpoint', planner_id: 'agent-ren', pr_url: 'https://github.com/arni-labs/deep-sci-fi/pull/47', has_plan: true, tests_passed: false }, _events: [
    { action: 'StartPlanning', from_status: 'Planning', to_status: 'Planned', timestamp: ago(800000), params: {} },
    { action: 'StartWork', from_status: 'Planned', to_status: 'InProgress', timestamp: ago(700000), params: {} },
    { action: 'ReportUnitTests', from_status: 'InProgress', to_status: 'InProgress', timestamp: ago(100000), params: { ok: true, summary: '5 passed, 0 failed' } },
    { action: 'BeginTesting', from_status: 'InProgress', to_status: 'Testing', timestamp: ago(90000), params: {} },
  ], _sequence_nr: 8 },
  { Id: 'wc-2', Status: 'Completed', _entity_type: 'WorkCycle', _app: 'paw-harness', fields: { task_summary: 'Foresight timeline visualization', planner_id: 'agent-ren', pr_url: 'https://github.com/arni-labs/deep-sci-fi/pull/44', has_plan: true, tests_passed: true }, _events: [], _sequence_nr: 12 },
  { Id: 'wc-3', Status: 'InProgress', _entity_type: 'WorkCycle', _app: 'paw-harness', fields: { task_summary: 'Content moderation pipeline', planner_id: 'agent-ren', has_plan: false, tests_passed: false }, _events: [], _sequence_nr: 3 },

  // ──── paw-foresight: Observations, Projections, Directions ────
  { Id: 'obs-1', Status: 'Confirmed', _entity_type: 'Observation', _app: 'paw-foresight', fields: { content: 'All deep-sci-fi monitors (error rate, latency, throughput) showing green.', source_session_id: 'sess-probe-1' }, _events: [
    { action: 'Record', from_status: 'Created', to_status: 'Created', timestamp: ago(3600000), params: {} },
    { action: 'Confirm', from_status: 'Created', to_status: 'Confirmed', timestamp: ago(3500000), params: {} },
  ], _sequence_nr: 3 },
  { Id: 'obs-2', Status: 'Confirmed', _entity_type: 'Observation', _app: 'paw-foresight', fields: { content: 'Observability approach thrashing over 24 hours — 4 different monitoring strategies.', source_session_id: 'sess-probe-1' }, _events: [], _sequence_nr: 3 },
  { Id: 'obs-3', Status: 'Created', _entity_type: 'Observation', _app: 'paw-foresight', fields: { content: 'Proposal voting at scale (>10K) needs materialized views for sub-200ms response.', source_session_id: 'sess-probe-1' }, _events: [], _sequence_nr: 1 },
  { Id: 'obs-4', Status: 'Created', _entity_type: 'Observation', _app: 'paw-foresight', fields: { content: 'Scientific grounding pipeline has no automated regression testing.', source_session_id: 'sess-probe-2' }, _events: [], _sequence_nr: 1 },
  { Id: 'obs-5', Status: 'Confirmed', _entity_type: 'Observation', _app: 'paw-foresight', fields: { content: '50+ concurrent users will hit Supabase real-time limits. Need CRDT approach.', source_session_id: 'sess-probe-2' }, _events: [], _sequence_nr: 3 },

  { Id: 'proj-1', Status: 'Complete', _entity_type: 'Projection', _app: 'paw-foresight', fields: { name: 'Q3 Scaling Projection', step_count: 3, probe_count: 3, observation_count: 15 }, _events: [], _sequence_nr: 7 },

  { Id: 'dir-1', Status: 'Proposed', _entity_type: 'Direction', _app: 'paw-foresight', fields: { title: 'Implement materialized views for proposal aggregation', confidence: 0.85 }, _events: [], _sequence_nr: 1 },
  { Id: 'dir-2', Status: 'UnderReview', _entity_type: 'Direction', _app: 'paw-foresight', fields: { title: 'Add CRDT concurrent editing for world proposals', confidence: 0.72 }, _events: [], _sequence_nr: 2 },

  // ──── paw-heal: Monitors, AlertCycles ────
  { Id: 'mon-1', Status: 'Active', _entity_type: 'Monitor', _app: 'paw-heal', fields: { name: 'error_rate_5xx', dd_query: 'avg:trace.error_rate{service:deep-sci-fi}', threshold: 5, alert_count: 2 }, _events: [], _sequence_nr: 5 },
  { Id: 'mon-2', Status: 'Active', _entity_type: 'Monitor', _app: 'paw-heal', fields: { name: 'p99_latency', dd_query: 'avg:trace.latency.p99{service:deep-sci-fi}', threshold: 2000, alert_count: 0 }, _events: [], _sequence_nr: 2 },
  { Id: 'mon-3', Status: 'Active', _entity_type: 'Monitor', _app: 'paw-heal', fields: { name: 'db_connection_pool', threshold: 80, alert_count: 1 }, _events: [], _sequence_nr: 3 },

  { Id: 'alert-1', Status: 'Triaging', _entity_type: 'AlertCycle', _app: 'paw-heal', fields: { monitor_id: 'mon-1', diagnosis: 'Connection pool exhaustion from unindexed query', sre_session_id: 'sess-sre-1' }, _events: [
    { action: 'Open', from_status: 'Created', to_status: 'Triaging', timestamp: ago(300000), params: {} },
  ], _sequence_nr: 2 },

  // ──── paw-pm: Issues ────
  { Id: 'issue-42', Status: 'InProgress', _entity_type: 'Issue', _app: 'paw-pm', fields: { title: 'World proposal voting endpoint', assignee_id: 'agent-swe', planner_id: 'agent-ren', priority: 'high', labels: ['feature', 'backend'] }, _events: [], _sequence_nr: 6 },
  { Id: 'issue-43', Status: 'Todo', _entity_type: 'Issue', _app: 'paw-pm', fields: { title: 'Content moderation pipeline', planner_id: 'agent-ren', priority: 'high', labels: ['feature', 'ai'] }, _events: [], _sequence_nr: 2 },
  { Id: 'issue-44', Status: 'Done', _entity_type: 'Issue', _app: 'paw-pm', fields: { title: 'Foresight timeline visualization', assignee_id: 'agent-swe', priority: 'medium' }, _events: [], _sequence_nr: 10 },
  { Id: 'issue-45', Status: 'InReview', _entity_type: 'Issue', _app: 'paw-pm', fields: { title: 'Fix p99 latency regression on /api/worlds', assignee_id: 'agent-swe', priority: 'critical', labels: ['bug'] }, _events: [], _sequence_nr: 8 },

  // ──── paw-channels ────
  { Id: 'chan-discord', Status: 'Connected', _entity_type: 'Channel', _app: 'paw-channels', fields: { type: 'discord', guild_id: '1234567890' }, _events: [], _sequence_nr: 3 },

  // ──── paw-compute ────
  { Id: 'comp-swe-1', Status: 'Ready', _entity_type: 'Computer', _app: 'paw-compute', fields: { machine_id: 'fly-sprite-a1b2c3', sandbox_url: 'https://swe-sandbox.fly.dev', cpu_cores: 2, memory_gb: 4, provisioned_at: ago(86400000) }, _events: [], _sequence_nr: 3 },
  { Id: 'comp-sre-1', Status: 'Ready', _entity_type: 'Computer', _app: 'paw-compute', fields: { machine_id: 'fly-sprite-d4e5f6', sandbox_url: 'https://sre-sandbox.fly.dev', cpu_cores: 1, memory_gb: 2, provisioned_at: ago(43200000) }, _events: [], _sequence_nr: 3 },

  // ──── paw-fs ────
  { Id: 'file-soul-paw', Status: 'Ready', _entity_type: 'File', _app: 'paw-fs', fields: { Name: 'paw-soul.md', MimeType: 'text/markdown', size_bytes: 4200 }, _events: [], _sequence_nr: 2 },
  { Id: 'file-session-tree-1', Status: 'Ready', _entity_type: 'File', _app: 'paw-fs', fields: { Name: 'session-tree.jsonl', MimeType: 'application/jsonl', size_bytes: 89000 }, _events: [], _sequence_nr: 2 },

  // ──── paw-ingest ────
  { Id: 'wh-1', Status: 'Processed', _entity_type: 'WebhookEvent', _app: 'paw-ingest', fields: { source: 'github', event_type: 'pull_request', payload_summary: 'PR #47 opened by SWE' }, _events: [], _sequence_nr: 3 },
  { Id: 'wh-2', Status: 'Processed', _entity_type: 'WebhookEvent', _app: 'paw-ingest', fields: { source: 'datadog', event_type: 'alert', payload_summary: 'Monitor error_rate_5xx triggered' }, _events: [], _sequence_nr: 3 },
  { Id: 'wh-3', Status: 'Processing', _entity_type: 'WebhookEvent', _app: 'paw-ingest', fields: { source: 'github', event_type: 'push', payload_summary: 'Push to feat/world-proposals' }, _events: [], _sequence_nr: 2 },
];

// ═══════════════════════════════════════════════════════════════
// AUTHORIZATION — generic, applies to any entity type
// ═══════════════════════════════════════════════════════════════

export const MOCK_DECISIONS: MockDecision[] = [
  { id: 'pd-123', tenant: 'default', agent_id: 'agent-ren', action: 'MergePR', resource_type: 'WorkCycle', resource_id: 'wc-1', denial_reason: 'Requires supervisor approval', module_name: 'coding_agent_runner', agent_type: 'agent', created_at: ago(60000), status: 'pending', decided_by: null, decided_at: null, generated_policy: 'permit(\n  principal == Agent::"agent-ren",\n  action == Action::"MergePR",\n  resource == WorkCycle::"wc-1"\n);' },
  { id: 'pd-124', tenant: 'default', agent_id: 'agent-sre', action: 'DeleteMonitor', resource_type: 'Monitor', resource_id: 'mon-3', denial_reason: 'Requires admin approval', module_name: null, agent_type: 'agent', created_at: ago(120000), status: 'pending', decided_by: null, decided_at: null, generated_policy: 'permit(\n  principal == Agent::"agent-sre",\n  action == Action::"DeleteMonitor",\n  resource == Monitor::"mon-3"\n);' },
  { id: 'pd-120', tenant: 'default', agent_id: 'agent-swe', action: 'ProcessToolCalls', resource_type: 'Session', resource_id: 'sess-swe-1', denial_reason: '', module_name: 'llm_caller', agent_type: 'system', created_at: ago(3600000), status: 'approved', decided_by: 'human:sesh', decided_at: ago(3500000), generated_policy: null },
  { id: 'pd-119', tenant: 'default', agent_id: 'agent-ren', action: 'SpawnSession', resource_type: 'Agent', resource_id: 'agent-swe', denial_reason: 'Non-supervisor cannot spawn', module_name: null, agent_type: 'agent', created_at: ago(7200000), status: 'denied', decided_by: 'human:sesh', decided_at: ago(7100000), generated_policy: null },
];

export const MOCK_POLICIES: MockPolicy[] = [
  { policy_id: 'session-admin', tenant: 'default', cedar_text: 'permit(\n  principal is Admin,\n  action,\n  resource is Session\n);', enabled: true, source: 'paw-agent', created_by: 'startup' },
  { policy_id: 'session-system-callbacks', tenant: 'default', cedar_text: 'permit(\n  principal,\n  action in [\n    Action::"ProcessToolCalls",\n    Action::"HandleToolResults",\n    Action::"Heartbeat",\n    Action::"TimeoutFail"\n  ],\n  resource is Session\n) when {\n  principal.agent_type == "system"\n};', enabled: true, source: 'paw-agent', created_by: 'startup' },
  { policy_id: 'session-steering', tenant: 'default', cedar_text: 'permit(\n  principal,\n  action in [Action::"Steer"],\n  resource is Session\n) when {\n  ["supervisor","human","system"].contains(principal.agent_type)\n};', enabled: true, source: 'paw-agent', created_by: 'startup' },
  { policy_id: 'memory-own-only', tenant: 'default', cedar_text: 'permit(\n  principal,\n  action in [Action::"Save",Action::"Update",Action::"Recall"],\n  resource is Memory\n) when {\n  resource.agent_id == principal.id\n};', enabled: true, source: 'paw-agent', created_by: 'startup' },
  { policy_id: 'workcycle-no-self-approve', tenant: 'default', cedar_text: 'forbid(\n  principal is Agent,\n  action == Action::"Approve",\n  resource is WorkCycle\n) when {\n  resource.planner_id == principal.id\n};', enabled: true, source: 'paw-harness', created_by: 'startup' },
  { policy_id: 'monitor-delete-admin', tenant: 'default', cedar_text: 'forbid(\n  principal is Agent,\n  action == Action::"DeleteMonitor",\n  resource is Monitor\n);', enabled: true, source: 'paw-heal', created_by: 'startup' },
  { policy_id: 'observation-confirm-diff', tenant: 'default', cedar_text: 'forbid(\n  principal,\n  action == Action::"Confirm",\n  resource is Observation\n) when {\n  resource.source_session_id == context.session_id\n};', enabled: true, source: 'paw-foresight', created_by: 'startup' },
  { policy_id: 'dsf-autonomy', tenant: 'default', cedar_text: 'permit(\n  principal,\n  action in [Action::"create",Action::"read",Action::"list"],\n  resource is Issue\n);', enabled: true, source: 'dsf-team', created_by: 'install_app' },
];

export const MOCK_AUTHZ_HISTORY: MockAuthzEvent[] = [
  { timestamp: ago(30000), entity_type: 'Session', entity_id: 'sess-swe-1', action: 'ProcessToolCalls', success: true, from_status: 'Thinking', to_status: 'Executing', authz_denied: false, denied_resource: null, matched_policy_ids: ['default:session-system-callbacks:0'] },
  { timestamp: ago(60000), entity_type: 'WorkCycle', entity_id: 'wc-1', action: 'MergePR', success: false, from_status: 'Testing', to_status: 'Testing', authz_denied: true, denied_resource: 'WorkCycle:wc-1', matched_policy_ids: null },
  { timestamp: ago(90000), entity_type: 'Session', entity_id: 'sess-sre-1', action: 'ProcessToolCalls', success: true, from_status: 'Thinking', to_status: 'Executing', authz_denied: false, denied_resource: null, matched_policy_ids: ['default:session-system-callbacks:0'] },
  { timestamp: ago(120000), entity_type: 'Monitor', entity_id: 'mon-3', action: 'DeleteMonitor', success: false, from_status: 'Active', to_status: 'Active', authz_denied: true, denied_resource: 'Monitor:mon-3', matched_policy_ids: ['default:monitor-delete-admin:0'] },
  { timestamp: ago(200000), entity_type: 'Session', entity_id: 'sess-swe-1', action: 'ProcessToolCalls', success: true, from_status: 'Thinking', to_status: 'Executing', authz_denied: false, denied_resource: null, matched_policy_ids: ['default:session-system-callbacks:0'] },
  { timestamp: ago(300000), entity_type: 'Memory', entity_id: 'mem-swe-1', action: 'Save', success: true, from_status: 'Active', to_status: 'Active', authz_denied: false, denied_resource: null, matched_policy_ids: ['default:memory-own-only:0'] },
  { timestamp: ago(400000), entity_type: 'Observation', entity_id: 'obs-3', action: 'Confirm', success: false, from_status: 'Created', to_status: 'Created', authz_denied: true, denied_resource: 'Observation:obs-3', matched_policy_ids: ['default:observation-confirm-diff:0'] },
  { timestamp: ago(500000), entity_type: 'Issue', entity_id: 'issue-42', action: 'create', success: true, from_status: '', to_status: 'Backlog', authz_denied: false, denied_resource: null, matched_policy_ids: ['default:dsf-autonomy:0'] },
];

// ═══════════════════════════════════════════════════════════════
// OS APPS — the capability catalog
// ═══════════════════════════════════════════════════════════════

export const MOCK_OS_APPS: MockOsApp[] = [
  { name: 'paw-agent', description: 'Core agent lifecycle — souls, skills, sessions, memory, teams', entity_types: ['Agent', 'Soul', 'Skill', 'Session', 'Memory', 'Team', 'CronJob', 'HeartbeatMonitor', 'CapabilityRequest', 'ToolHook'], version: '0.1.0', installed: true },
  { name: 'paw-harness', description: 'Development workflow governance', entity_types: ['ProjectHarness', 'WorkCycle'], version: '0.1.0', installed: true },
  { name: 'paw-heal', description: 'Self-healing monitoring with Datadog', entity_types: ['Monitor', 'AlertCycle', 'MonitorScan'], version: '0.1.0', installed: true },
  { name: 'paw-foresight', description: 'Product direction engine', entity_types: ['Observation', 'Projection', 'Direction', 'ProductModel', 'DirectionFeedback'], version: '0.1.0', installed: true },
  { name: 'paw-pm', description: 'Project management', entity_types: ['Issue', 'Project', 'Cycle', 'Label', 'Comment'], version: '0.1.0', installed: true },
  { name: 'paw-channels', description: 'Multi-channel agent communication', entity_types: ['Channel', 'ChannelSession', 'AgentRoute'], version: '0.1.0', installed: true },
  { name: 'paw-compute', description: 'Compute provisioning', entity_types: ['Computer'], version: '0.1.0', installed: true },
  { name: 'paw-fs', description: 'File, workspace, directory management', entity_types: ['File', 'FileVersion', 'Directory', 'Workspace'], version: '0.1.0', installed: true },
  { name: 'paw-research', description: 'Governed web search', entity_types: ['WebQuery'], version: '0.1.0', installed: true },
  { name: 'paw-ingest', description: 'Webhook ingress and routing', entity_types: ['WebhookEvent', 'WebhookRoute'], version: '0.1.0', installed: true },
];

// ═══════════════════════════════════════════════════════════════
// HELPERS — derive views from generic entities
// ═══════════════════════════════════════════════════════════════

/** Get all entities of a given type */
export function entitiesOfType(type: string): MockEntity[] {
  return MOCK_ENTITIES.filter(e => e._entity_type === type);
}

/** Get all entities from a given app */
export function entitiesFromApp(app: string): MockEntity[] {
  return MOCK_ENTITIES.filter(e => e._app === app);
}

/** Get all entity types present in the data */
export function allEntityTypes(): string[] {
  return [...new Set(MOCK_ENTITIES.map(e => e._entity_type))].sort();
}

/** Get all apps that have entities */
export function allApps(): string[] {
  return [...new Set(MOCK_ENTITIES.map(e => e._app))].sort();
}

/** Get active (non-terminal) sessions */
export function activeSessions(): MockEntity[] {
  return MOCK_ENTITIES.filter(e =>
    e._entity_type === 'Session' &&
    !['Completed', 'Failed', 'Cancelled'].includes(e.Status)
  );
}

/** Get entities with non-terminal status (something happening) */
export function activeEntities(): MockEntity[] {
  const terminal = ['Completed', 'Failed', 'Cancelled', 'Archived', 'Done', 'Processed', 'Faded'];
  return MOCK_ENTITIES.filter(e => !terminal.includes(e.Status));
}

/** Summary counts by entity type */
export function entityCounts(): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const e of MOCK_ENTITIES) {
    counts[e._entity_type] = (counts[e._entity_type] || 0) + 1;
  }
  return counts;
}

/** Summary counts by app */
export function appCounts(): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const e of MOCK_ENTITIES) {
    counts[e._app] = (counts[e._app] || 0) + 1;
  }
  return counts;
}
