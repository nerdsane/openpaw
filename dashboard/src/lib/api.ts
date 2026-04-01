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
  let url = `${BASE}/api/tenants/default/decisions`;
  if (status) url += `?status=${status}`;
  const res = await fetch(url, { headers: HEADERS });
  if (!res.ok) return { decisions: [], total: 0, pending_count: 0, approved_count: 0, denied_count: 0 };
  return res.json();
}

export async function fetchPolicies(): Promise<PolicyEntry[]> {
  const res = await fetch(`${BASE}/api/tenants/default/policies`, { headers: HEADERS });
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

export interface AgentHistoryEntry {
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
}

export async function fetchAgentHistory(
  entityId: string,
  entityType: string = 'Agent',
  limit: number = 200
): Promise<AgentHistoryEntry[]> {
  // Try multiple possible endpoints — the observe history endpoint may not exist.
  const params = new URLSearchParams();
  if (entityType) params.set('entity_type', entityType);
  params.set('limit', limit.toString());

  const endpoints = [
    `${BASE}/observe/agents/${entityId}/history?${params.toString()}`,
    `${BASE}/observe/agents/system/history?${params.toString()}`,
    `${BASE}/observe/history?${params.toString()}`,
  ];

  for (const url of endpoints) {
    try {
      const res = await fetch(url, { headers: HEADERS });
      if (!res.ok) continue;
      const data = await res.json();
      const history = data.history ?? data ?? [];
      const filtered = (Array.isArray(history) ? history : []).filter(
        (e: AgentHistoryEntry) => e.entity_id === entityId
      );
      return filtered;
    } catch {
      // endpoint unavailable, try next
    }
  }
  // All endpoints failed — gracefully return empty
  return [];
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
