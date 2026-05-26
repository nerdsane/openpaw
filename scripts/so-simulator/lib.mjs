// Small helpers for the stackoverflow-agents simulator.
//
// No npm deps; Node 18+ native fetch only. Everything here is pure
// helpers — no top-level effects — so it stays cheap to import from
// both the runtime entry point and any unit-style smoke tests.

// ─── Seeded RNG (mulberry32) ────────────────────────────────────────
// We deliberately do NOT use Math.random — the simulator must be
// bit-stable across runs so the demo is repeatable. Same seed → same
// agent decisions → same trajectory shape.

export function mulberry32(seed) {
  let state = seed >>> 0;
  return function next() {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function pick(rng, arr) {
  return arr[Math.floor(rng() * arr.length)];
}

// ─── Tiny argv parser (no dep) ──────────────────────────────────────

export function parseArgs(argv) {
  const flags = new Set();
  const opts = {};
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a.startsWith('--')) {
      const eq = a.indexOf('=');
      if (eq > -1) {
        opts[a.slice(2, eq)] = a.slice(eq + 1);
      } else {
        flags.add(a.slice(2));
      }
    }
  }
  return { flags, opts };
}

// ─── Stable ID helpers ──────────────────────────────────────────────
// We pick sim-prefixed deterministic IDs so the simulator's writes are
// idempotent on reruns (PUT-like POST behavior) and post-mortem
// queries against the journal can grep for them easily.

export function questionId(seed, idx) {
  return `sim-q-${seed}-${idx}`;
}

export function answerId(seed, q, idx) {
  return `sim-a-${seed}-${q}-${idx}`;
}

export function agentId(seed, idx) {
  return `sim-agent-${seed}-${idx}`;
}

// ─── HTTP helpers ───────────────────────────────────────────────────
// Each helper returns { ok, status, body, error } — never throws on
// non-2xx, because we *want* to observe non-2xx responses (the whole
// downvote attempt is supposed to fail).

export async function odataGet(base, tenant, path, headers = {}) {
  const url = `${base.replace(/\/$/, '')}${path}`;
  try {
    const resp = await fetch(url, {
      method: 'GET',
      headers: {
        Accept: 'application/json',
        'X-Tenant-Id': tenant,
        ...headers,
      },
    });
    const text = await resp.text();
    let body = text;
    try { body = JSON.parse(text); } catch (_) { /* leave as text */ }
    return { ok: resp.ok, status: resp.status, body };
  } catch (err) {
    return { ok: false, status: 0, error: err.message ?? String(err) };
  }
}

export async function odataPost(base, tenant, path, body, headers = {}) {
  const url = `${base.replace(/\/$/, '')}${path}`;
  try {
    const resp = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json',
        'X-Tenant-Id': tenant,
        ...headers,
      },
      body: JSON.stringify(body),
    });
    const text = await resp.text();
    let parsed = text;
    try { parsed = JSON.parse(text); } catch (_) { /* leave as text */ }
    return { ok: resp.ok, status: resp.status, body: parsed };
  } catch (err) {
    return { ok: false, status: 0, error: err.message ?? String(err) };
  }
}

// ─── Scripted agent decisions ───────────────────────────────────────
// In deterministic mode, "what should this agent do next" is a
// function of (rng, world state). In LLM mode we substitute a Claude
// call; the *shape* of the returned decision is identical so the
// downstream code never branches on the agent brain.

export function scriptedDecide(rng, world, agentIdx) {
  // Phase 1 only has one interesting decision: which answer to try to
  // downvote. We pick the one with the lowest upvote count, ties
  // broken by the seeded RNG.
  const answers = world.answers.slice();
  answers.sort((a, b) => {
    if (a.upvotes !== b.upvotes) return a.upvotes - b.upvotes;
    return rng() < 0.5 ? -1 : 1;
  });
  const target = answers[0];
  return {
    kind: 'downvote',
    answerId: target.id,
    reason: 'low-quality answer (scripted)',
  };
}

// ─── Tiny LLM bridge (optional) ─────────────────────────────────────
// Wraps Claude's messages API. We deliberately keep this as a single
// fetch call so the simulator stays dep-free; if LLM mode is not
// enabled, this is never imported.

export async function llmDecide({ apiKey, model, world, agentIdx }) {
  const url = 'https://api.anthropic.com/v1/messages';
  const system =
    'You are agent ' + agentIdx + ' on a Q&A site for AI agents. ' +
    'You can ONLY upvote, accept, delete, or (try to) downvote answers. ' +
    'Pick ONE answer to downvote because it is low-quality. ' +
    'Reply as compact JSON: {"kind":"downvote","answer_id":"...","reason":"..."}';
  const userBody = JSON.stringify({
    answers: world.answers.map((a) => ({ id: a.id, upvotes: a.upvotes })),
  });
  const body = {
    model,
    max_tokens: 200,
    system,
    messages: [{ role: 'user', content: userBody }],
  };
  try {
    const resp = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'x-api-key': apiKey,
        'anthropic-version': '2023-06-01',
      },
      body: JSON.stringify(body),
    });
    if (!resp.ok) {
      const t = await resp.text();
      return { error: `claude ${resp.status}: ${t.slice(0, 200)}` };
    }
    const json = await resp.json();
    const text = json?.content?.[0]?.text ?? '';
    try {
      const parsed = JSON.parse(text);
      return {
        kind: 'downvote',
        answerId: parsed.answer_id ?? parsed.answerId,
        reason: parsed.reason ?? 'llm-driven',
      };
    } catch (_) {
      return { error: 'claude returned non-JSON: ' + text.slice(0, 120) };
    }
  } catch (err) {
    return { error: err.message ?? String(err) };
  }
}

// ─── Pretty-printers ────────────────────────────────────────────────

export function fmtTrajectoryLine(entry) {
  const status = entry.success ? 'OK' : 'FAIL';
  const code = entry.status ?? '-';
  return `[${status} ${code}] ${entry.agent ?? '-'} ${entry.action} ${entry.target ?? ''} :: ${entry.note ?? ''}`;
}
