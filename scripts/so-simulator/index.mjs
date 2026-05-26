#!/usr/bin/env node
//
// so-simulator — synthetic agent-user driver for stackoverflow-agents.
//
// Phase 1.5 of the directed-evolution build. Exercises the running
// stackoverflow-agents tenant over OData, intentionally bumps into the
// absent `Downvote` action, and emits the resulting unmet intent so
// the evolver (in genesis) picks it up and grows the missing feature.
//
// See README.md for the full mode matrix.
//
// Determinism is the default. The whole point is that the
// "downvote-attempt-fails" trajectory is bit-stable across runs so the
// demo (and any reproducibility report under .proofs/) doesn't drift.

import {
  mulberry32,
  parseArgs,
  questionId,
  answerId,
  agentId,
  odataGet,
  odataPost,
  scriptedDecide,
  llmDecide,
  fmtTrajectoryLine,
} from './lib.mjs';

// ─── Configuration ──────────────────────────────────────────────────

const argv = parseArgs(process.argv.slice(2));
const FLAGS = argv.flags;
const OPTS = argv.opts;

const cfg = {
  dryRun: FLAGS.has('dry-run'),
  llm: FLAGS.has('llm'),
  noEvolution: FLAGS.has('no-evolution'),
  targetOnly: FLAGS.has('target-only'),

  soApiBase: process.env.SO_API_BASE ?? 'http://127.0.0.1:3000',
  soTenant: process.env.SO_TENANT_ID ?? 'stackoverflow-agents',
  genesisApiBase: process.env.GENESIS_API_BASE ?? 'http://127.0.0.1:3000',
  genesisTenant: process.env.GENESIS_TENANT_ID ?? 'default',

  seed: Number(process.env.SO_SIM_SEED ?? OPTS.seed ?? 42),
  questions: Number(process.env.SO_SIM_QUESTIONS ?? OPTS.questions ?? 3),
  answersPerQuestion: Number(
    process.env.SO_SIM_ANSWERS ?? OPTS.answers ?? 3,
  ),
  agents: Number(process.env.SO_SIM_AGENTS ?? OPTS.agents ?? 4),
  intentAutonomy: Number(process.env.SO_SIM_INTENT_AUTONOMY ?? 0),

  anthropicApiKey: process.env.ANTHROPIC_API_KEY ?? '',
  anthropicModel: process.env.ANTHROPIC_MODEL ?? 'claude-sonnet-4-6',
};

if (cfg.llm && !cfg.anthropicApiKey && !cfg.dryRun) {
  log('!! --llm requires ANTHROPIC_API_KEY; falling back to scripted mode');
  cfg.llm = false;
}

const rng = mulberry32(cfg.seed);
const trajectory = [];

function log(msg) {
  // Single sink so the proof report can grep ::SIM:: lines.
  process.stdout.write(`::SIM:: ${msg}\n`);
}

function record(entry) {
  trajectory.push(entry);
  log(fmtTrajectoryLine(entry));
}

// ─── Seed corpus ────────────────────────────────────────────────────

const sampleTitles = [
  'How do I stream WASM logs to ClickHouse?',
  'Why does my Cedar policy deny everything?',
  'What is the canonical hash for an empty tree?',
  'Best way to fan out OData reads in Rust?',
  'How do agents reliably emit unmet intents?',
];

const sampleAnswerBodies = [
  'You should just disable Cedar entirely.', // intentionally bad
  'Use a separate file watcher per partition.',
  'Read the spec, then call DispatchAction.',
  'I have no idea — please update the docs!', // intentionally bad
  'Try `temper verify --specs-dir specs`.',
];

// ─── Seed phase: questions + answers + upvotes ──────────────────────

async function seedWorld() {
  log(`seeding ${cfg.questions} questions × ${cfg.answersPerQuestion} answers @ tenant=${cfg.soTenant}`);
  const questions = [];

  for (let q = 0; q < cfg.questions; q += 1) {
    const qid = questionId(cfg.seed, q);
    const title = sampleTitles[q % sampleTitles.length];
    const body = {
      Id: qid,
      Title: title,
      Body: `seeded by so-simulator (seed=${cfg.seed})`,
      AuthorId: `sim-author-${q}`,
      Status: 'Open',
      HasAccepted: false,
      AcceptedAnswerId: null,
      CreatedAt: new Date(0).toISOString(),
      UpdatedAt: new Date(0).toISOString(),
    };

    if (cfg.dryRun) {
      record({
        action: 'POST Questions',
        target: qid,
        success: true,
        status: 200,
        note: 'dry-run',
      });
    } else {
      const r = await odataPost(
        cfg.soApiBase,
        cfg.soTenant,
        '/tdata/Questions',
        body,
      );
      record({
        action: 'POST Questions',
        target: qid,
        success: r.ok,
        status: r.status,
        note: r.ok ? '' : excerpt(r.body),
      });
    }

    const answers = [];
    for (let a = 0; a < cfg.answersPerQuestion; a += 1) {
      const aid = answerId(cfg.seed, q, a);
      const abody = {
        Id: aid,
        QuestionId: qid,
        Body: sampleAnswerBodies[(q + a) % sampleAnswerBodies.length],
        AuthorId: `sim-author-${q}-${a}`,
        Status: 'Active',
        Upvotes: 0,
        CreatedAt: new Date(0).toISOString(),
      };

      if (cfg.dryRun) {
        record({
          action: 'POST Answers',
          target: aid,
          success: true,
          status: 200,
          note: 'dry-run',
        });
        answers.push({ id: aid, upvotes: 0 });
      } else {
        const r = await odataPost(
          cfg.soApiBase,
          cfg.soTenant,
          '/tdata/Answers',
          abody,
        );
        record({
          action: 'POST Answers',
          target: aid,
          success: r.ok,
          status: r.status,
          note: r.ok ? '' : excerpt(r.body),
        });
        answers.push({ id: aid, upvotes: 0 });
      }
    }
    questions.push({ id: qid, title, answers });
  }

  // Cast a handful of Upvotes — deterministically distributed so one
  // answer ends up clearly low-quality (which is the "bad answer"
  // every agent then tries to downvote).
  log(`casting upvotes (deterministic distribution)`);
  for (const q of questions) {
    for (let i = 0; i < q.answers.length; i += 1) {
      // Answer 0: many upvotes; answer 1: a few; answer 2: zero (the
      // intended downvote target).
      const upvotes = Math.max(0, q.answers.length - 1 - i) * 2;
      for (let u = 0; u < upvotes; u += 1) {
        const voter = `sim-voter-${u}`;
        const path = `/tdata/Answers('${q.answers[i].id}')/Soa.QA.Upvote`;
        if (cfg.dryRun) {
          record({
            action: 'Upvote',
            target: q.answers[i].id,
            success: true,
            status: 200,
            note: `dry-run voter=${voter}`,
          });
        } else {
          const r = await odataPost(
            cfg.soApiBase,
            cfg.soTenant,
            path,
            { VoterId: voter },
          );
          record({
            action: 'Upvote',
            target: q.answers[i].id,
            success: r.ok,
            status: r.status,
            agent: voter,
            note: r.ok ? '' : excerpt(r.body),
          });
        }
        q.answers[i].upvotes += 1;
      }
    }
  }
  return questions;
}

function excerpt(v) {
  if (v == null) return '';
  const s = typeof v === 'string' ? v : JSON.stringify(v);
  return s.slice(0, 140).replace(/\s+/g, ' ');
}

// ─── Downvote phase: each agent attempts the absent action ──────────

async function downvoteAttempts(world) {
  log(`${cfg.agents} synthetic user-agents attempting Downvote (this MUST fail in Phase 1)`);
  const failures = [];

  // We flatten the answers across all questions, then each agent
  // picks the lowest-upvote one (scripted) or asks Claude (llm).
  const allAnswers = world.flatMap((q) => q.answers);
  const worldView = { answers: allAnswers };

  for (let i = 0; i < cfg.agents; i += 1) {
    const id = agentId(cfg.seed, i);
    let decision;
    if (cfg.llm) {
      decision = await llmDecide({
        apiKey: cfg.anthropicApiKey,
        model: cfg.anthropicModel,
        world: worldView,
        agentIdx: i,
      });
      if (decision.error) {
        record({
          agent: id,
          action: 'llm-decide',
          target: '',
          success: false,
          status: 0,
          note: decision.error,
        });
        decision = scriptedDecide(rng, worldView, i);
      }
    } else {
      decision = scriptedDecide(rng, worldView, i);
    }

    const target = decision.answerId;
    const path = `/tdata/Answers('${target}')/Soa.QA.Downvote`;
    if (cfg.dryRun) {
      // We mark it as a deliberate failure for symmetry with the live
      // path. The expected real-world response is 404/400 because the
      // CSDL has no `Downvote` action declared on Soa.QA.Answer.
      const entry = {
        agent: id,
        action: 'Downvote (absent)',
        target,
        success: false,
        status: 404,
        note: 'dry-run (CSDL has no Downvote action — expected)',
      };
      record(entry);
      failures.push(entry);
    } else {
      const r = await odataPost(
        cfg.soApiBase,
        cfg.soTenant,
        path,
        { VoterId: id, reason: decision.reason },
      );
      const entry = {
        agent: id,
        action: 'Downvote (absent)',
        target,
        success: r.ok,
        status: r.status,
        note: r.ok
          ? '!! UNEXPECTED — Downvote already exists?'
          : excerpt(r.body),
      };
      record(entry);
      if (!r.ok) failures.push(entry);
    }
  }
  return failures;
}

// ─── Intent emission phase ──────────────────────────────────────────

async function emitUnmetIntent(failures) {
  if (!failures.length) {
    log('no failed downvotes recorded — skipping unmet intent emission');
    return;
  }
  const sample = failures[0];
  const intentBody = {
    action: 'Downvote',
    intent: 'agents want to downvote low-quality answers',
    tenant: cfg.soTenant,
    entity_type: 'Answer',
    error: `${sample.status} on /tdata/Answers('${sample.target}')/Soa.QA.Downvote`,
    source: 'platform',
    metadata: {
      sim_seed: cfg.seed,
      sim_agents: cfg.agents,
      sim_failures: failures.length,
    },
  };

  if (cfg.dryRun) {
    record({
      action: 'POST /api/evolution/trajectories/unmet',
      target: cfg.soTenant,
      success: true,
      status: 201,
      note: `dry-run body=${excerpt(intentBody)}`,
    });
  } else {
    const r = await odataPost(
      cfg.soApiBase,
      cfg.soTenant,
      '/api/evolution/trajectories/unmet',
      intentBody,
    );
    record({
      action: 'POST /api/evolution/trajectories/unmet',
      target: cfg.soTenant,
      success: r.ok,
      status: r.status,
      note: r.ok ? '' : excerpt(r.body),
    });
  }
}

async function createEvolutionRow(failures) {
  if (cfg.noEvolution || cfg.targetOnly) {
    log('skipping Evolution creation (--no-evolution / --target-only)');
    return;
  }
  if (!failures.length) {
    log('no failed downvotes — skipping Evolution creation');
    return;
  }

  // Stable, deterministic UUID-shaped id. We deliberately do not use
  // crypto.randomUUID() so reruns produce the same Evolution row id
  // (the OData store will treat the POST as an upsert via key).
  const stableHex = (n) => n.toString(16).padStart(8, '0');
  const evolutionId =
    `${stableHex(cfg.seed)}-0000-4000-8000-${stableHex(0)}0000`;
  const body = {
    Id: evolutionId,
    TargetApp: 'stackoverflow-agents',
    TargetTenant: cfg.soTenant,
    Intent: 'agents want to downvote low-quality answers',
    ProblemStatement:
      'Add a Downvote action and a downvotes counter to Answer; ' +
      'maintain ScoreConsistent across upvotes and downvotes.',
    Autonomy: cfg.intentAutonomy,
    VariantCount: 0,
    Status: 'IntentObserved',
    CreatedAt: new Date(0).toISOString(),
  };

  if (cfg.dryRun) {
    record({
      action: 'POST Evolutions',
      target: evolutionId,
      success: true,
      status: 201,
      note: `dry-run body=${excerpt(body)}`,
    });
    return;
  }
  const r = await odataPost(
    cfg.genesisApiBase,
    cfg.genesisTenant,
    '/tdata/Evolutions',
    body,
  );
  record({
    action: 'POST Evolutions',
    target: evolutionId,
    success: r.ok,
    status: r.status,
    note: r.ok ? '' : excerpt(r.body),
  });
}

// ─── Pre-flight: discover the running tenant ────────────────────────

async function preflight() {
  if (cfg.dryRun) {
    log('dry-run mode; skipping CSDL discovery');
    return { ok: true, dryRun: true };
  }
  const r = await odataGet(
    cfg.soApiBase,
    cfg.soTenant,
    '/tdata/$metadata',
  );
  if (!r.ok) {
    log(`!! CSDL fetch failed (${r.status}) at ${cfg.soApiBase} — server unreachable?`);
    return { ok: false };
  }
  const body = typeof r.body === 'string' ? r.body : JSON.stringify(r.body);
  const hasQuestion = body.includes('EntityType Name="Question"');
  const hasAnswer = body.includes('EntityType Name="Answer"');
  const hasDownvote = /Action Name="Downvote"/.test(body);
  log(`preflight: Question=${hasQuestion} Answer=${hasAnswer} Downvote=${hasDownvote}`);
  if (hasDownvote) {
    log('!! Downvote action ALREADY present in CSDL — the seed app has already evolved? Aborting.');
    return { ok: false, alreadyEvolved: true };
  }
  if (!hasQuestion || !hasAnswer) {
    log('!! tenant does not look like stackoverflow-agents');
    return { ok: false };
  }
  return { ok: true };
}

// ─── Main ───────────────────────────────────────────────────────────

async function main() {
  log(`so-simulator starting; mode=${cfg.dryRun ? 'dry-run' : cfg.llm ? 'llm' : 'scripted'} seed=${cfg.seed}`);
  log(`config: questions=${cfg.questions} answers/q=${cfg.answersPerQuestion} agents=${cfg.agents}`);
  log(`endpoints: so=${cfg.soApiBase} (tenant=${cfg.soTenant}); genesis=${cfg.genesisApiBase} (tenant=${cfg.genesisTenant})`);

  const pf = await preflight();
  if (!pf.ok && !cfg.dryRun) {
    log('!! pre-flight failed — exit 2');
    process.exit(2);
  }

  const world = await seedWorld();
  const failures = await downvoteAttempts(world);
  await emitUnmetIntent(failures);
  await createEvolutionRow(failures);

  const totalActions = trajectory.length;
  const successCount = trajectory.filter((t) => t.success).length;
  const downvoteFails = trajectory.filter(
    (t) => t.action === 'Downvote (absent)' && !t.success,
  ).length;
  log(`done: ${totalActions} actions, ${successCount} ok, ${downvoteFails} downvote-absent (the point)`);

  // Single-line summary suitable for ::PROOF:: redirection
  const summary = JSON.stringify({
    sim_seed: cfg.seed,
    total_actions: totalActions,
    successes: successCount,
    downvote_failures: downvoteFails,
    unmet_intent_emitted: failures.length > 0,
    evolution_row_created: failures.length > 0 && !cfg.noEvolution && !cfg.targetOnly,
    mode: cfg.dryRun ? 'dry-run' : cfg.llm ? 'llm' : 'scripted',
  });
  process.stdout.write(`::SIM-SUMMARY:: ${summary}\n`);

  // Exit 0 if we got the expected failure signature; exit 3 if the
  // seed app already has Downvote (no unmet intent to emit).
  if (failures.length === 0 && !cfg.dryRun) {
    process.exit(3);
  }
}

main().catch((err) => {
  log(`!! uncaught error: ${err?.stack ?? err}`);
  process.exit(1);
});
