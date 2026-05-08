import { entityId, parseJsonString, readField, textValue } from '$lib/entity-format';
import type { EntityEvent } from '$lib/types';

export const TERMINAL_STATUSES = new Set(['Completed', 'Complete', 'Failed', 'Cancelled', 'Archived', 'Rejected']);

export function entitySetToEntityType(entitySet: string): string {
  if (entitySet.endsWith('ies')) return `${entitySet.slice(0, -3)}y`;
  if (entitySet.endsWith('s')) return entitySet.slice(0, -1);
  return entitySet;
}

export function field(row: Record<string, unknown> | null | undefined, keys: string[]): unknown {
  for (const key of keys) {
    const value = readField(row, key);
    if (value !== undefined && value !== null && value !== '') return value;
  }
  return undefined;
}

export function asEventArray(row: Record<string, unknown> | null | undefined): EntityEvent[] {
  const events = readField(row, '_events');
  return Array.isArray(events) ? events as EntityEvent[] : [];
}

export function timestampMs(value: unknown): number {
  if (typeof value !== 'string' || !value.trim()) return 0;
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

export function idTimeMs(id: unknown): number {
  const value = String(id ?? '');
  const match = value.match(/^en-([0-9a-f]{8})-([0-9a-f]{4})/i);
  if (!match) return 0;
  const high = Number.parseInt(match[1], 16);
  const low = Number.parseInt(match[2], 16);
  if (!Number.isFinite(high) || !Number.isFinite(low)) return 0;
  return high * 0x10000 + low;
}

export function lastActivityMs(row: Record<string, unknown> | null | undefined): number {
  const explicit = [
    'last_heartbeat_at',
    'completed_at',
    'updated_at',
    'started_at',
    'created_at'
  ]
    .map((key) => timestampMs(readField(row, key)))
    .filter((value) => value > 0);
  const events = asEventArray(row).map((event) => timestampMs(event.timestamp)).filter((value) => value > 0);
  return Math.max(...explicit, ...events, idTimeMs(entityId(row)), 0);
}

export function firstActivityMs(row: Record<string, unknown> | null | undefined): number {
  const explicit = [
    'created_at',
    'started_at',
    'last_heartbeat_at',
    'completed_at'
  ]
    .map((key) => timestampMs(readField(row, key)))
    .filter((value) => value > 0);
  const events = asEventArray(row).map((event) => timestampMs(event.timestamp)).filter((value) => value > 0);
  const idMs = idTimeMs(entityId(row));
  const all = [...explicit, ...events, idMs].filter((value) => value > 0);
  return all.length ? Math.min(...all) : 0;
}

export function formatDateTimeMs(ms: number): string {
  if (!ms) return '-';
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  }).format(new Date(ms));
}

export function formatDateTime(value: unknown): string {
  return formatDateTimeMs(timestampMs(value));
}

export function formatCount(value: unknown): string {
  const num = Number(value ?? 0);
  return Number.isFinite(num) ? num.toLocaleString() : textValue(value);
}

export function jsonArrayCount(value: unknown): number {
  const parsed = parseJsonString(value);
  return Array.isArray(parsed) ? parsed.length : 0;
}

export function sessionTitle(row: Record<string, unknown>): string {
  return textValue(
    field(row, [
      'user_message',
      'result',
      'error_message',
      'task_summary',
      'summary',
      'Id'
    ])
  );
}

export function shortText(value: unknown, max = 120): string {
  const text = textValue(value);
  if (text === '-') return text;
  return text.length > max ? `${text.slice(0, max - 1)}...` : text;
}
