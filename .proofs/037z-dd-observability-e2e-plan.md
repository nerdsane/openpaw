# Proof Report: 037z — End-to-End Tracing Verification (Execution Plan)

## Date
2026-04-20 (plan written); executed after merges land

## Branch / Commit
openpaw `dd-observability` and temper `feat/dd-obs-host-spans` (both need to merge to respective `main` before this verification can run)

## What This Report Covers

This is the **final-step verification plan** for ADR-0037. It cannot run until two prerequisite merges ship to production:

1. **temper `nerdsane/temper` main** — receives both:
   - `codex/discord-trace-context` (remote parent extraction + proper `traceparent` injection in `ProductionWasmHost::http_call`) — work by another agent, not touched by this branch.
   - `feat/dd-obs-host-spans` (`split_span_hint_headers` + `apply_span_hints` applied to `http_call` / `http_call_binary`) — this branch.
2. **openpaw `main`** — receives `dd-observability` which already has:
   - DD pipeline severity fix (already live, commit `df031d34`)
   - llm_caller per-attempt timing (commit `699885e1`)
   - llm_caller span hint headers (commit `639899d7`)

Once both merge and Railway auto-deploys, this report's verification steps can run.

## Expected Trace Tree (from DD APM UI)

Click any katagami job's root span and you should see:

```
temper.action  [CurationJob.StartResearch]                            ~0.5s
 ├─ wasm.invoke  [module_name=research_agent_spawn, ...]              ~0.2s
 │
 └─ temper.action  [Session.Submit]                                    ~10s
     └─ wasm.invoke  [module_name=llm_caller, trigger_action=call_llm] ~8s
         └─ tool.llm_call.anthropic                                    ~7.5s
             • gen_ai.system = anthropic
             • gen_ai.request.model = claude-sonnet-4.6
             • http.method = POST
             • http.url = api.anthropic.com/v1/messages
             • status_code = 200
             • response_bytes = 4096
             • logs (via trace_id correlation):
                 – "llm_caller: anthropic attempt 1/5 start..."
                 – "llm_caller: anthropic attempt 1 end elapsed_ms=7485 http_status=200..."
                 – "llm_caller: anthropic complete attempts=1 total_elapsed_ms=7485..."

temper.action  [Session.ProcessToolCalls]                              ~90s ← the slow one
 └─ wasm.invoke  [module_name=monty_repl]
     ├─ wasm.host.http_call (temper_get)                               ~10ms
     ├─ wasm.host.http_call (temper_web_fetch)                         ~3s
     ├─ wasm.host.http_call (temper_write)                             ~30s ← BOTTLENECK
     │   └─ temper.action  [File.CreateFile]
     │       └─ wasm.invoke  [module_name=workspace_fs]                ~10s ← TRUE COST
     │           (child spans depend on future workspace_fs work)
     └─ wasm.host.http_call (temper_action)                            ~200ms
```

## Verification Steps

### Pre-deploy check

```bash
# Confirm Fix A still firing clean (run from openpaw root, .env has DD keys)
source .env
curl -s -X POST -H "DD-API-KEY: $DD_API_KEY" -H "DD-APPLICATION-KEY: $DD_APP_KEY" \
  -H "Content-Type: application/json" \
  "https://api.${DD_SITE:-datadoghq.com}/api/v2/logs/analytics/aggregate" \
  -d '{"compute":[{"aggregation":"count","type":"total"}],
       "group_by":[{"facet":"status","limit":10}],
       "filter":{"query":"service:openpaw \"entity actor stopped\"","from":"now-1h","to":"now"}}'
# Expected: only `info` and `debug`. No critical/alert/emergency.
```

### Trigger verification

1. **Merge `feat/dd-obs-host-spans` and `codex/discord-trace-context` to `nerdsane/temper` main** (external to this repo). Bump temper dep commit in `openpaw/crates/temperpaw/Cargo.toml` if pinned, or just `cargo update -p temper-platform -p temper-server -p temper-wasm` on the openpaw side.
2. **Merge `dd-observability` to `openpaw` main.** Railway picks up, redeploys.
3. **Wait for Railway deploy to reach steady state.** Watch `temper_up` metric or `service:openpaw` boot logs.
4. **Re-spawn the two failing katagami research jobs:**
   - Job: `en-019d9f1c-6802-7061-99e3-2efb6f254145` (2026 UI UX trends)
   - Job: `en-019d9f1c-de7c-7621-bf4f-7dbd3ea1b693` (Chinese calligraphy)
   - These can be kicked by calling the katagami "reseed research" action, OR by creating equivalent new jobs.

### Post-deploy queries

**Query 1: Verify `tool.llm_call.anthropic` spans now exist**

```bash
curl -s -G -H "DD-API-KEY: $DD_API_KEY" -H "DD-APPLICATION-KEY: $DD_APP_KEY" \
  --data-urlencode "from=$(($(date +%s) - 3600))" --data-urlencode "to=$(date +%s)" \
  --data-urlencode "query=sum:trace.temper.openpaw.hits{resource_name:tool.llm_call.anthropic}.as_count()" \
  "https://api.${DD_SITE:-datadoghq.com}/api/v1/query"
# Expected: > 0 over any 1h window with traffic.
```

**Query 2: Verify `gen_ai.request.model` is queryable as a span attribute**

In DD APM UI, query `service:openpaw @resource_name:tool.llm_call.anthropic @gen_ai.request.model:*` — expected: returns spans with the model name visible.

**Query 3: Verify Fix B timing logs appear and correlate to the span**

In DD APM, click any `tool.llm_call.anthropic` span → Logs tab. Expected: three structured log lines per call (attempt start / attempt end / complete) with `trace_id` matching the span.

**Query 4: Verify the 13-min black hole is gone**

Filter DD logs to `service:openpaw @session_id:<new-session-id>`. Expected: continuous log activity throughout the agent's LLM call, no more than 15-second silence windows. If the agent gets stuck, `llm_caller: HANG HINT` warn logs fire at 60 s.

### Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| `trace.temper.openpaw.hits{resource_name:tool.llm_call.anthropic}` in last 1h | > 0 | TBD after deploy | PENDING |
| Session entity has continuous log activity | yes | TBD | PENDING |
| `HANG HINT` warn log fires on >60s stall | yes (tested via forced slow endpoint if needed) | TBD | PENDING |
| DD APM flame graph for a katagami session | flat tree with session → action → wasm.invoke → tool.llm_call | TBD | PENDING |
| `gen_ai.system` and `gen_ai.request.model` visible as span attrs | yes | TBD | PENDING |

## What to Do If Verification Fails

**If no `tool.llm_call.anthropic` spans appear:**
- Confirm `feat/dd-obs-host-spans` is merged and deployed (`cargo tree -p temper-wasm | head` from an openpaw rebuild shows the right commit).
- Check `scripts/otel-collector-datadog.yaml` deploy status — see `.proofs/037b-otel-export-investigation.md` for mitigation (resourcedetection + resource processors).
- DD API key scope: `/api/v2/spans/events/search` requires APM read; use the DD UI if the API returns `Unauthorized`.

**If hint headers leak to Anthropic / OpenRouter:**
- Anthropic's API is permissive about unknown headers (returns 200 even with `X-Temper-Span-*` present), so no immediate outage. Confirm `codex/discord-trace-context` + `feat/dd-obs-host-spans` are BOTH merged before considering this a long-term solution — without the host stripping, they pass through.

**If Fix B logs don't appear:**
- Verify WASM `llm_caller.wasm` was rebuilt and the new artifact shipped. Compare `monty_repl/target/wasm32-wasip1/release/monty_repl.wasm` mtime against deploy time.

## Artifacts to Capture

1. DD APM trace URL for one successful katagami research session.
2. Screenshot of the flame graph (save to `.proofs/037z-flame-graph-success.png`).
3. Screenshot of a Fix B hang-hint firing (reproduce by pointing `anthropic_api_url` secret at a deliberately slow mock if prod doesn't hang naturally).
4. Before/after `service:openpaw status:critical` log count over 24 h window.

## Dependencies This Verifies Transitively

- ADR-0037 Decision item 1 (W3C Trace Context) — proved by step "gen_ai attrs visible on span".
- ADR-0037 Decision item 2 (root span per action) — proved by trace tree shape.
- ADR-0037 Decision item 3 (host↔WASM invisible) — proved by connected flame graph.
- ADR-0037 Decision item 4 (gen_ai semconv) — proved by attribute query.
- ADR-0037 Decision item 5 (DD pipeline severity rename) — already verified in `037a`.

Once all steps land green, this ADR moves from `Status: Proposed` → `Status: Accepted`, the deprecated per-action metrics list from the ADR's "Deprecated Metrics" section goes into a follow-up issue, and the dashboards migrate to APM trace metrics.
