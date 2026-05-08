export function snakeCaseKey(key: string): string {
  return key
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/[\s-]+/g, '_')
    .toLowerCase();
}

export function lowerFirst(key: string): string {
  if (!key) return key;
  return `${key[0].toLowerCase()}${key.slice(1)}`;
}

export function readField(row: Record<string, unknown> | null | undefined, key: string): unknown {
  if (!row) return undefined;

  const variants = [key, snakeCaseKey(key), lowerFirst(key), key.toLowerCase()];
  for (const variant of variants) {
    if (Object.prototype.hasOwnProperty.call(row, variant)) {
      return row[variant];
    }
  }

  const wanted = snakeCaseKey(key);
  const match = Object.keys(row).find((candidate) => snakeCaseKey(candidate) === wanted);
  return match ? row[match] : undefined;
}

export function entityId(row: Record<string, unknown> | null | undefined): string {
  return String(readField(row, 'Id') ?? row?._entity_id ?? '');
}

export function entityStatus(row: Record<string, unknown> | null | undefined): string {
  return String(readField(row, 'Status') ?? '');
}

export function textValue(value: unknown): string {
  if (value === null || value === undefined || value === '') return '-';
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

export function parseJsonString(value: unknown): unknown {
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  if (!trimmed) return null;
  if (!((trimmed.startsWith('{') && trimmed.endsWith('}')) || (trimmed.startsWith('[') && trimmed.endsWith(']')))) {
    return null;
  }
  try {
    return JSON.parse(trimmed);
  } catch {
    return null;
  }
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function asRecord(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {};
}

export function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

export function fieldLabel(key: string): string {
  return key
    .replace(/^_/, '')
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

export function truncateMiddle(value: string, head = 12, tail = 8): string {
  if (value.length <= head + tail + 1) return value;
  return `${value.slice(0, head)}...${value.slice(-tail)}`;
}
