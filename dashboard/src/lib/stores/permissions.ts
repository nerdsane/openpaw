import { writable } from 'svelte/store';
import { fetchDecisions, fetchPolicies, type PendingDecision, type PolicyEntry, type SessionHistoryEntry } from '$lib/api';

export const decisions = writable<PendingDecision[]>([]);
export const policies = writable<PolicyEntry[]>([]);
export const authzHistory = writable<SessionHistoryEntry[]>([]);

export async function loadDecisions(status?: string): Promise<void> {
  const res = await fetchDecisions(status);
  decisions.set(res.decisions);
}

export async function loadPolicies(): Promise<void> {
  const data = await fetchPolicies();
  policies.set(data);
}

/**
 * Fetch global authorization history (all entities).
 * Uses the history endpoint without entity_id filtering.
 */
export async function loadAuthzHistory(limit = 500): Promise<void> {
  const params = new URLSearchParams();
  params.set('limit', limit.toString());
  const res = await fetch(`/observe/agents/system/history?${params.toString()}`, {
    headers: { 'x-tenant-id': 'default', 'x-temper-principal-kind': 'admin' },
  });
  if (!res.ok) { authzHistory.set([]); return; }
  const data = await res.json();
  const history = data.history ?? data ?? [];
  authzHistory.set(Array.isArray(history) ? history : []);
}
