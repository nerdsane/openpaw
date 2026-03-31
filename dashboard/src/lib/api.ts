const BASE = ''; // relative — proxied by Vite in dev, served by tower-http in prod

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

  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`OData query failed: ${res.status} ${res.statusText}`);
  }
  const data = await res.json();
  return data.value || [];
}

export async function getEntity(
  entitySet: string,
  id: string
): Promise<Record<string, unknown>> {
  const res = await fetch(`${BASE}/tdata/${entitySet}('${id}')`);
  if (!res.ok) {
    throw new Error(`OData get failed: ${res.status} ${res.statusText}`);
  }
  return res.json();
}
