/** An entity event from the OData events array. */
export interface EntityEvent {
  action: string;
  from_status: string;
  to_status: string;
  timestamp: string;
  params?: Record<string, unknown>;
}

export interface Agent {
  Id: string;
  Status: string;
  soul_id: string;
  model: string;
  provider: string;
  turn_count: number;
  max_turns: string;
  input_tokens: number;
  output_tokens: number;
  cost_cents: string;
  parent_agent_id: string;
  child_agent_ids: string;
  agent_depth: number;
  last_heartbeat_at: string;
  pending_decision_id: string;
  pending_tool_calls: string;
  pending_tool_context: string;
  result: string;
  error_message: string;
  user_message: string;
  tools_enabled: string;
  _entity_type?: string;
  _entity_id?: string;
  _events?: EntityEvent[];
  _sequence_nr?: number;
  _total_event_count?: number;
}

export interface WorkCycle {
  Id: string;
  Status: string;
  task_summary: string;
  planner_id: string;
  approver_id: string;
  plan_summary: string;
  test_summary: string;
  pr_url: string;
  has_plan: boolean;
  tests_passed: boolean;
  project_harness_id: string;
}

export interface Soul {
  Id: string;
  Status: string;
  name: string;
  description: string;
}

export interface ProjectHarness {
  Id: string;
  Status: string;
  repo_url: string;
  tech_stack: string;
  project_name: string;
}

export interface Skill {
  Id: string;
  Status: string;
  name: string;
  description: string;
}
