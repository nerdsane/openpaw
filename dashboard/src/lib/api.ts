const BASE = ''; // relative — proxied by Vite in dev, served by tower-http in prod

// Default headers for all OData requests.
const HEADERS: Record<string, string> = {
  'x-tenant-id': 'default',
  'x-temper-principal-kind': 'admin',
};

/**
 * Flatten a Temper OData entity response.
 * Temper returns { entity_type, entity_id, status, fields: { Id, Status, ... }, counters, booleans, ... }.
 * We merge `fields`, `counters`, and `booleans` into a single flat object for simpler consumption.
 */
function flattenEntity(raw: Record<string, unknown>): Record<string, unknown> {
  const fields = (raw.fields ?? {}) as Record<string, unknown>;
  const counters = (raw.counters ?? {}) as Record<string, unknown>;
  const booleans = (raw.booleans ?? {}) as Record<string, unknown>;
  return {
    _entity_type: raw.entity_type,
    _entity_id: raw.entity_id,
    _events: raw.events ?? [],
    _sequence_nr: raw.sequence_nr,
    _total_event_count: raw.total_event_count,
    ...fields,
    ...counters,
    ...booleans,
  };
}

export async function queryEntities(
  entitySet: string,
  filter?: string,
  orderby?: string,
  top?: number
): Promise<Record<string, unknown>[]> {
  let url = `${BASE}/tdata/${entitySet}`;
  const params = new URLSearchParams();
  if (filter) params.set('$filter', filter);
  if (orderby) params.set('$orderby', orderby);
  if (top) params.set('$top', top.toString());
  const qs = params.toString();
  if (qs) url += `?${qs}`;

  const res = await fetch(url, { headers: HEADERS });
  if (!res.ok) {
    throw new Error(`OData query failed: ${res.status} ${res.statusText}`);
  }
  const data = await res.json();
  const raw = (data.value || []) as Record<string, unknown>[];
  return raw.map(flattenEntity);
}

export async function fetchDecisions(status?: string): Promise<DecisionsResponse> {
  // Temper platform serves decisions at /api/decisions (global, not tenant-scoped)
  let url = `${BASE}/api/decisions`;
  if (status) url += `?status=${status}`;
  const res = await fetch(url, { headers: HEADERS });
  if (!res.ok) return { decisions: [], total: 0, pending_count: 0, approved_count: 0, denied_count: 0 };
  return res.json();
}

export async function fetchPolicies(): Promise<PolicyEntry[]> {
  // Temper platform uses /policies/list and returns { policies: [...] }
  const res = await fetch(`${BASE}/api/tenants/default/policies/list`, { headers: HEADERS });
  if (!res.ok) return [];
  const data = await res.json();
  return Array.isArray(data) ? data : (data.policies ?? data.value ?? []);
}

export interface DecisionsResponse {
  decisions: PendingDecision[];
  total: number;
  pending_count: number;
  approved_count: number;
  denied_count: number;
}

export interface PendingDecision {
  id: string;
  tenant: string;
  agent_id: string;
  action: string;
  resource_type: string;
  resource_id: string;
  resource_attrs?: Record<string, unknown>;
  denial_reason: string;
  module_name?: string;
  agent_type?: string;
  created_at: string;
  status: string;
  decided_by?: string;
  decided_at?: string;
  generated_policy?: string;
}

export interface PolicyEntry {
  policy_id: string;
  tenant: string;
  cedar_text: string;
  enabled: boolean;
  created_at?: string;
  created_by?: string;
  source?: string;
}

export interface SessionHistoryEntry {
  timestamp: string;
  tenant: string;
  entity_type: string;
  entity_id: string;
  action: string;
  success: boolean;
  from_status: string;
  to_status: string;
  error: string | null;
  authz_denied: boolean;
  denied_resource: string | null;
  /** Cedar policy IDs that contributed to the authorization decision (allow or deny).
   *  Available after Temper ADR-0039 (authz policy traceability). */
  matched_policy_ids: string[] | null;
}

export async function fetchSessionHistory(
  entityId: string,
  entityType: string = 'Session',
  limit: number = 200
): Promise<SessionHistoryEntry[]> {
  const params = new URLSearchParams();
  if (entityType) params.set('entity_type', entityType);
  params.set('limit', limit.toString());
  const url = `${BASE}/observe/agents/system/history?${params.toString()}`;
  const res = await fetch(url, { headers: HEADERS });
  if (!res.ok) return [];
  const data = await res.json();
  const history = data.history ?? data ?? [];
  // Filter to only events for this specific entity
  return (Array.isArray(history) ? history : []).filter(
    (e: SessionHistoryEntry) => e.entity_id === entityId
  );
}

export async function queryTeams(): Promise<Record<string, unknown>[]> {
  return queryEntities('Teams');
}

export async function queryAgentsForTeam(teamId: string): Promise<Record<string, unknown>[]> {
  return queryEntities('Agents', `team_id eq '${teamId}'`);
}

export async function getEntity(
  entitySet: string,
  id: string
): Promise<Record<string, unknown>> {
  const res = await fetch(`${BASE}/tdata/${entitySet}('${id}')`, { headers: HEADERS });
  if (!res.ok) {
    throw new Error(`OData get failed: ${res.status} ${res.statusText}`);
  }
  const raw = await res.json();
  return flattenEntity(raw);
}

/**
 * Fetch raw file content by file ID (e.g. soul markdown, skill markdown).
 */
export async function fetchFileContent(fileId: string): Promise<string> {
  const res = await fetch(`${BASE}/tdata/Files('${fileId}')/$value`, { headers: HEADERS });
  if (!res.ok) return '';
  return res.text();
}

// ──────────────────── Setup & Transport API ────────────────────

export interface SetupStatus {
  has_anthropic_key: boolean;
  has_discord: boolean;
  has_slack: boolean;
  has_agents: boolean;
  agent_count: number;
  discord_connected: boolean;
  slack_connected: boolean;
}

export async function fetchSetupStatus(): Promise<SetupStatus> {
  const res = await fetch(`${BASE}/paw/setup/status`, { headers: HEADERS });
  if (!res.ok) throw new Error(`Setup status failed: ${res.status}`);
  return res.json();
}

export async function saveSecret(key: string, value: string): Promise<void> {
  const res = await fetch(`${BASE}/paw/setup/secrets`, {
    method: 'POST',
    headers: { ...HEADERS, 'content-type': 'application/json' },
    body: JSON.stringify({ key, value }),
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `Save secret failed: ${res.status}`);
  }
}

export async function listSecretKeys(): Promise<string[]> {
  const res = await fetch(`${BASE}/paw/setup/secrets`, { headers: HEADERS });
  if (!res.ok) return [];
  const data = await res.json();
  return data.keys ?? [];
}

export async function deleteSecret(key: string): Promise<void> {
  await fetch(`${BASE}/paw/setup/secrets/${key}`, {
    method: 'DELETE',
    headers: HEADERS,
  });
}

export interface SoulTemplate {
  name: string;
  description: string;
  path: string;
}

export async function getSoulTemplates(): Promise<SoulTemplate[]> {
  const res = await fetch(`${BASE}/paw/souls/templates`, { headers: HEADERS });
  if (!res.ok) return [];
  const data = await res.json();
  return data.templates ?? [];
}

export interface CreateAgentParams {
  name: string;
  role?: string;
  soul_template?: string;
  model?: string;
  tools_enabled?: string;
  max_turns?: string;
}

export async function createAgent(params: CreateAgentParams): Promise<{ agent_id: string }> {
  const res = await fetch(`${BASE}/paw/agents/create`, {
    method: 'POST',
    headers: { ...HEADERS, 'content-type': 'application/json' },
    body: JSON.stringify(params),
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error || `Create agent failed: ${res.status}`);
  }
  return res.json();
}

export interface TransportStatusResponse {
  discord: { status: string; guild_id?: string; message?: string };
  slack: { status: string; message?: string };
}

export async function getTransportStatus(): Promise<TransportStatusResponse> {
  const res = await fetch(`${BASE}/paw/transports/status`, { headers: HEADERS });
  if (!res.ok) throw new Error(`Transport status failed: ${res.status}`);
  return res.json();
}

export async function connectDiscord(params: {
  bot_token: string;
  public_key?: string;
  guild_id?: string;
  feed_channel_id?: string;
  forum_channel_id?: string;
}): Promise<void> {
  const res = await fetch(`${BASE}/paw/transports/discord/connect`, {
    method: 'POST',
    headers: { ...HEADERS, 'content-type': 'application/json' },
    body: JSON.stringify(params),
  });
  if (!res.ok) throw new Error(`Connect Discord failed: ${res.status}`);
}

export async function disconnectDiscord(): Promise<void> {
  await fetch(`${BASE}/paw/transports/discord/disconnect`, {
    method: 'POST',
    headers: HEADERS,
  });
}

export async function connectSlack(params: {
  app_token: string;
  bot_token: string;
  signing_secret?: string;
}): Promise<void> {
  const res = await fetch(`${BASE}/paw/transports/slack/connect`, {
    method: 'POST',
    headers: { ...HEADERS, 'content-type': 'application/json' },
    body: JSON.stringify(params),
  });
  if (!res.ok) throw new Error(`Connect Slack failed: ${res.status}`);
}

export async function disconnectSlack(): Promise<void> {
  await fetch(`${BASE}/paw/transports/slack/disconnect`, {
    method: 'POST',
    headers: HEADERS,
  });
}
