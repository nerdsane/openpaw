import type { EntityEvent } from './types';

export interface ToolCall {
  name: string;
  input: Record<string, unknown>;
}

export interface SessionTurn {
  number: number;
  timestamp: string;
  toolCalls: ToolCall[];
  /** Cumulative input tokens at this turn (from LLM call) */
  inputTokens: number;
  /** Cumulative output tokens at this turn (from LLM call) */
  outputTokens: number;
  authz?: { allowed: boolean; denied_resource?: string };
}

/** Accurate session metrics computed from events, not entity counters. */
export interface SessionMetrics {
  /** Number of LLM calls (ProcessToolCalls events) */
  turns: number;
  /** Total input tokens from the latest ProcessToolCalls (cumulative) */
  inputTokens: number;
  /** Total output tokens from the latest ProcessToolCalls (cumulative) */
  outputTokens: number;
  /** Sum of all per-turn output tokens (actual generation cost) */
  totalOutputTokens: number;
}

export function parsePendingToolCalls(raw: string): ToolCall[] {
  if (!raw || raw.startsWith('[stored')) return [];
  try {
    const arr = JSON.parse(raw);
    if (!Array.isArray(arr)) return [];
    return arr.map((tc: Record<string, unknown>) => ({
      name: (tc.name as string) ?? 'unknown',
      input: (tc.input as Record<string, unknown>) ?? {},
    }));
  } catch {
    return [];
  }
}

/**
 * Extract turns from session events.
 *
 * Each ProcessToolCalls event = one turn. Params carry:
 * - input_tokens: cumulative input tokens for the context
 * - output_tokens: cumulative output tokens generated
 * - pending_tool_calls: JSON array of tool calls
 */
export function eventsToTurns(events: EntityEvent[]): SessionTurn[] {
  const turns: SessionTurn[] = [];
  let turnNumber = 0;

  for (const event of events) {
    if (event.action === 'Heartbeat') continue;

    if (event.action === 'ProcessToolCalls') {
      turnNumber++;
      const toolCallsRaw = (event.params?.pending_tool_calls as string) ?? '';
      const toolCalls = parsePendingToolCalls(toolCallsRaw);
      const inputTokens = parseInt(String(event.params?.input_tokens ?? '0'), 10) || 0;
      const outputTokens = parseInt(String(event.params?.output_tokens ?? '0'), 10) || 0;

      turns.push({
        number: turnNumber,
        timestamp: event.timestamp ?? '',
        toolCalls,
        inputTokens,
        outputTokens,
      });
    }
  }

  return turns;
}

/**
 * Compute accurate session metrics from events.
 *
 * The entity's `input_tokens` and `output_tokens` fields are counter
 * increment counts (number of times the counter was bumped), NOT actual
 * token values. The real values live in ProcessToolCalls event params.
 */
export function computeMetrics(events: EntityEvent[]): SessionMetrics {
  let turns = 0;
  let inputTokens = 0;
  let outputTokens = 0;
  let totalOutputTokens = 0;
  let prevOutputTokens = 0;

  for (const event of events) {
    if (event.action === 'ProcessToolCalls') {
      turns++;
      const inTok = parseInt(String(event.params?.input_tokens ?? '0'), 10) || 0;
      const outTok = parseInt(String(event.params?.output_tokens ?? '0'), 10) || 0;
      inputTokens = inTok; // cumulative — last value is the total
      // Per-turn output = difference from previous cumulative
      const delta = outTok - prevOutputTokens;
      totalOutputTokens += delta > 0 ? delta : outTok;
      prevOutputTokens = outTok;
      outputTokens = outTok;
    }
  }

  return { turns, inputTokens, outputTokens, totalOutputTokens };
}

export function formatToolInput(name: string, input: Record<string, unknown>): string {
  switch (name) {
    case 'bash':
      return (input.command as string) ?? '';
    case 'read':
      return (input.file_path as string) ?? (input.path as string) ?? '';
    case 'write':
      return (input.file_path as string) ?? '';
    case 'edit':
      return (input.file_path as string) ?? '';
    case 'temper_action':
      return `${input.entity_set}('${(input.entity_id as string) ?? ''}').${input.action}`;
    case 'temper_get':
      return `${input.entity_set}('${(input.entity_id as string) ?? ''}')`;
    case 'temper_list':
      return `${input.entity_set}${input.filter ? `?$filter=${input.filter}` : ''}`;
    case 'temper_create':
      return `${input.entity_set}`;
    default: {
      const firstVal = Object.values(input).find((v) => typeof v === 'string');
      return (firstVal as string) ?? JSON.stringify(input).slice(0, 100);
    }
  }
}
