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
