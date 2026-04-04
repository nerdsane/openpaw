/**
 * Client-side Cedar policy text → human-readable permission summary.
 *
 * Handles real-world Cedar patterns including multi-line statements with
 * nested brackets (action in [...]) and when clauses.
 *
 * Limitation: This is best-effort regex parsing. The Temper platform does not
 * currently return which specific policy rule matched a given authorization
 * decision. When Temper source is available, the proper fix is to have the
 * Cedar evaluation path return matched_policy_id on every allow/deny result.
 */

export interface ParsedPermission {
  verdict: 'permit' | 'forbid';
  action: string;
  actionDisplay: string;
  resource: string;
  principal: string;
  condition: string | null;
  conditionRaw: string | null;
  rawLine: string;
}

/**
 * Convert a Cedar action name like "ProcessToolCalls" into "Process Tool Calls".
 */
function humanizeAction(action: string): string {
  if (action === '*') return 'Any action';
  return action
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .trim();
}

/**
 * Parse a block of Cedar policy text into an array of human-readable permissions.
 *
 * Real Cedar examples this must handle:
 *
 *   permit(
 *     principal is Admin,
 *     action,
 *     resource is Session
 *   );
 *
 *   permit(
 *     principal,
 *     action in [Action::"create", Action::"read"],
 *     resource is Issue
 *   ) when {
 *     resource.AssigneeId == principal.id
 *   };
 *
 *   permit(
 *     principal,
 *     action in [
 *       Action::"ProcessToolCalls",
 *       Action::"HandleToolResults"
 *     ],
 *     resource is Session
 *   ) when {
 *     principal.agent_type == "system"
 *   };
 */
export function parseCedarPolicies(cedarText: string): ParsedPermission[] {
  const results: ParsedPermission[] = [];

  // Strip comments
  const stripped = cedarText.replace(/\/\/[^\n]*/g, '');

  // Match permit/forbid blocks — use a stateful approach to handle nested brackets
  const blocks = extractBlocks(stripped);

  for (const block of blocks) {
    const verdict = block.verdict;
    const body = block.body;
    const whenClause = block.when;

    // Extract principal
    const principalMatch = body.match(/principal\s+is\s+(\w+)/);
    const principal = principalMatch ? principalMatch[1] : '*';

    // Extract actions — handle: bare `action`, `action == Action::"X"`, `action in [...]`
    let actions: string[] = [];
    const actionEqMatch = body.match(/action\s*==\s*Action::"([^"]+)"/);
    const actionInMatch = body.match(/action\s+in\s*\[([^\]]*)\]/s);

    if (actionEqMatch) {
      actions = [actionEqMatch[1]];
    } else if (actionInMatch) {
      const inner = actionInMatch[1];
      const items = inner.match(/Action::"([^"]+)"/g) || [];
      actions = items.map(a => a.replace(/Action::"|"/g, ''));
    } else if (/action\s*[,)]/.test(body) || /action\s*$/.test(body.trim())) {
      // Bare `action` (no constraint) = any action
      actions = ['*'];
    }

    if (actions.length === 0) actions = ['*'];

    // Extract resource type
    const resourceMatch = body.match(/resource\s+is\s+(\w+)/);
    const resource = resourceMatch ? resourceMatch[1] : '*';

    const conditionRaw = whenClause?.trim() || null;
    const condition = conditionRaw ? humanizeCondition(conditionRaw) : null;

    for (const action of actions) {
      results.push({
        verdict,
        action,
        actionDisplay: humanizeAction(action),
        resource,
        principal,
        condition,
        conditionRaw,
        rawLine: block.raw,
      });
    }
  }

  return results;
}

interface CedarBlock {
  verdict: 'permit' | 'forbid';
  body: string;
  when: string | null;
  raw: string;
}

/**
 * Extract permit/forbid blocks handling nested parentheses and brackets.
 */
function extractBlocks(text: string): CedarBlock[] {
  const blocks: CedarBlock[] = [];
  const pattern = /\b(permit|forbid)\s*\(/g;
  let m: RegExpExecArray | null;

  while ((m = pattern.exec(text)) !== null) {
    const verdict = m[1] as 'permit' | 'forbid';
    const start = m.index;
    let pos = m.index + m[0].length;

    // Find matching closing paren, tracking nesting
    let depth = 1;
    while (pos < text.length && depth > 0) {
      if (text[pos] === '(') depth++;
      else if (text[pos] === ')') depth--;
      pos++;
    }

    const body = text.slice(m.index + m[0].length, pos - 1);

    // Check for when clause
    let when: string | null = null;
    const afterParen = text.slice(pos).trimStart();
    if (afterParen.startsWith('when')) {
      const whenStart = text.indexOf('{', pos);
      if (whenStart !== -1) {
        let wDepth = 1;
        let wPos = whenStart + 1;
        while (wPos < text.length && wDepth > 0) {
          if (text[wPos] === '{') wDepth++;
          else if (text[wPos] === '}') wDepth--;
          wPos++;
        }
        when = text.slice(whenStart + 1, wPos - 1);
        pos = wPos;
      }
    }

    // Skip to semicolon
    const semiPos = text.indexOf(';', pos);
    const end = semiPos !== -1 ? semiPos + 1 : pos;

    blocks.push({
      verdict,
      body,
      when,
      raw: text.slice(start, end).trim(),
    });
  }

  return blocks;
}

/**
 * Attempt to make a Cedar condition clause human-readable.
 */
function humanizeCondition(condition: string): string {
  return condition
    .replace(/\["supervisor",\s*"human",\s*"system",\s*"agent"\]\.contains\(principal\.agent_type\)/g,
      'any authenticated principal')
    .replace(/\["supervisor",\s*"human",\s*"system"\]\.contains\(principal\.agent_type\)/g,
      'supervisors, humans, or system')
    .replace(/\["supervisor",\s*"human",\s*"system",\s*"admin"\]\.contains\(principal\.agent_type\)/g,
      'any privileged principal')
    .replace(/principal\.agent_type\s*==\s*"system"/g, 'system callbacks only')
    .replace(/principal\.agent_type\s*==\s*"agent"/g, 'agents only')
    .replace(/principal\s*==\s*resource/g, 'self only')
    .replace(/context\.parent_session_id\s*==\s*principal\.id/g, 'parent session')
    .replace(/resource\.planner_id\s*==\s*principal\.id/g, 'when agent is the planner')
    .replace(/resource\.AssigneeId\s*==\s*principal\.id/g, 'own assignments only')
    .replace(/resource\.agent_id\s*==\s*principal\.id/g, 'own resources only')
    .replace(/\[([^\]]+)\]\.contains\(context\.module\)/g, (_, modules) => {
      const names = modules.match(/"([^"]+)"/g)?.map((s: string) => s.replace(/"/g, '')) ?? [];
      if (names.length > 3) return `modules: ${names.slice(0, 3).join(', ')}...`;
      return `modules: ${names.join(', ')}`;
    })
    // Clean up leftover operators
    .replace(/\s*\|\|\s*/g, ' or ')
    .replace(/\s*&&\s*/g, ' and ')
    .replace(/\s+/g, ' ')
    .trim();
}

/**
 * Merge multiple policy files into a single permissions summary.
 * Groups by action+resource, resolving permit/forbid conflicts (forbid wins).
 */
export function summarizePermissions(allPolicies: string[]): ParsedPermission[] {
  const all: ParsedPermission[] = [];
  for (const text of allPolicies) {
    all.push(...parseCedarPolicies(text));
  }

  // Deduplicate: group by action+resource, forbid overrides permit
  const map = new Map<string, ParsedPermission>();
  for (const p of all) {
    const key = `${p.action}:${p.resource}`;
    const existing = map.get(key);
    if (!existing || p.verdict === 'forbid') {
      map.set(key, p);
    }
  }

  return Array.from(map.values()).sort((a, b) => {
    if (a.verdict !== b.verdict) return a.verdict === 'forbid' ? -1 : 1;
    if (a.resource !== b.resource) return a.resource.localeCompare(b.resource);
    return a.action.localeCompare(b.action);
  });
}
