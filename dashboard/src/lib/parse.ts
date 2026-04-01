import type { EntityEvent, ToolCall, AgentTurn } from './types';

export function parsePendingToolCalls(raw: string): ToolCall[] {
  if (!raw || raw.startsWith('[stored')) return [];
  try {
    const arr = JSON.parse(raw);
    if (!Array.isArray(arr)) return [];
    return arr.map((tc: Record<string, unknown>) => ({
      name: (tc.name as string) ?? 'unknown',
      input: (tc.input as Record<string, unknown>) ?? {},
    }));
  } catch { return []; }
}

export function eventsToTurns(events: EntityEvent[]): AgentTurn[] {
  const turns: AgentTurn[] = [];
  let turnNumber = 0;

  for (const event of events) {
    if (event.action === 'Heartbeat') continue;

    if (event.action === 'ProcessToolCalls') {
      turnNumber++;
      const toolCallsRaw = event.params?.pending_tool_calls as string ?? '';
      const toolCalls = parsePendingToolCalls(toolCallsRaw);

      // Format timestamp
      const ts = event.timestamp ?? '';

      turns.push({
        number: turnNumber,
        timestamp: ts,
        toolCalls,
      });
    }
  }

  return turns;
}

export function formatToolInput(name: string, input: Record<string, unknown>): string {
  // Format tool input as terminal-like command
  switch (name) {
    case 'bash':
      return input.command as string ?? '';
    case 'read':
      return input.file_path as string ?? input.path as string ?? '';
    case 'write':
      return input.file_path as string ?? '';
    case 'edit':
      return input.file_path as string ?? '';
    case 'temper_action':
      return `${input.entity_set}('${(input.entity_id as string)?.slice(0, 12)}').${input.action}`;
    case 'temper_get':
      return `${input.entity_set}('${(input.entity_id as string)?.slice(0, 12)}')`;
    case 'temper_list':
      return `${input.entity_set}${input.filter ? `?$filter=${input.filter}` : ''}`;
    case 'temper_create':
      return `${input.entity_set}`;
    default: {
      // Generic: show first string value
      const firstVal = Object.values(input).find(v => typeof v === 'string');
      return (firstVal as string) ?? JSON.stringify(input).slice(0, 100);
    }
  }
}
