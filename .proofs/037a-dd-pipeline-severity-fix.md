# Proof Report: 037a — DD Pipeline Severity Collision Fix

## Date
2026-04-20

## Branch / Commit
`dd-observability` @ `df031d34` (fix(dd-pipeline): rename entity status attribute to avoid DD severity collision)

## What Was Done

Deployed a DD log pipeline change that:

1. Adds an `attribute-remapper` processor that renames incoming `status` attribute → `entity_status` before the status-remapper runs. This decouples the entity state-machine state from DD's log severity classification.
2. Points the status-remapper at `otel.severity_text` (with `level` as fallback) so INFO logs from `tracing::info!` are classified as info, not critical.

Related ADR: `docs/adrs/0037-end-to-end-tracing-and-traceparent-propagation.md` (Decision item 5).

## Verification Flow

1. Captured pre-deploy baseline: counted log status distribution for `service:openpaw "entity actor stopped"` over 6h and 15 min windows.
2. Committed the JSON change; ran `python3 scripts/deploy_pipelines.py --dry-run`; confirmed only one pipeline update was planned.
3. Ran `python3 scripts/deploy_pipelines.py` (prod). Pipeline update returned 200; script later failed on an unrelated pre-existing 404 for `/api/v2/logs/config/facets` (that endpoint isn't exposed on this DD tier — not caused by our change).
4. Fetched the deployed pipeline definition via the DD API and confirmed processor order includes the new `entity_status` attribute-remapper and updated status-remapper.
5. Waited 30 s for new logs to flow through the updated pipeline.
6. Queried DD for post-deploy status distribution over 2-min window.
7. Sampled one recent `entity actor stopped` log and confirmed: `log status = info`, `entity_status = Ready`, `status attribute = absent (renamed)`.

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Pipeline processor list | Contains new "Rename entity status…" remapper at position [4] | Confirmed at position [4] | PASS |
| Pipeline processor list | Status-remapper sources = `otel.severity_text, level` | Confirmed | PASS |
| `entity actor stopped` critical logs (last 2 min post-deploy) | 0 | 0 | PASS |
| `entity actor stopped` alert logs (last 2 min post-deploy) | 0 | 0 | PASS |
| `entity actor stopped` emergency logs (last 2 min post-deploy) | 0 | 0 | PASS |
| `entity actor stopped` info logs (last 2 min post-deploy) | >0 (classified correctly) | 110 | PASS |
| Sample log `attributes.status` field | absent (renamed) | absent | PASS |
| Sample log `attributes.entity_status` field | preserves entity state (`Ready`, `CallingProvider`, etc.) | `Ready` | PASS |
| Baseline severity noise (last 6h pre-deploy) | N/A (for reference) | 106 alert + 276 critical + 30 emergency = **412 misclassified of 2174** (~19%) | recorded |

## What Worked
- Deploy script's idempotency: matched pipeline by name, PUT to existing ID `sGSl7pPaRX28rLdS1BnEvA`.
- Attribute rename preserves the information (entity state still queryable as `@entity_status`).
- Status-remapper fallback chain (`otel.severity_text, level`) covers both OTLP-delivered logs and any direct `tracing` emissions.

## What Didn't Work
- `scripts/deploy_pipelines.py` crashes after pipeline update on `/api/v2/logs/config/facets` 404. Pre-existing issue (not caused by this change). Facets are already registered manually via DD UI from earlier work. Script should be guarded with `try/except` or an explicit tier check — tracked as a follow-up, not blocking.

## Limitations
- Only 2 min of post-deploy data collected in this proof. Longer-horizon confirmation (24h) that the critical/alert/emergency count stays at 0 will be included in the final E2E proof (`037z-dd-observability-e2e.md`).
- The fix addresses the severity *classification* issue only. It does not change the volume of `entity actor stopped` logs, which remains high (~10–60/min). Downgrading to debug at the `tracing::info!` call site is an upstream Temper repo concern, separate from this pipeline fix.

## What Still Doesn't Work
- Nothing from the Fix A scope. This proof covers A only; B (llm_caller timing), C1–C4 (trace propagation), and the metric deprecation are in subsequent commits on the same branch.

## Artifacts
- Deployed pipeline ID: `sGSl7pPaRX28rLdS1BnEvA` (unchanged; updated in place).
- Pipeline definition in repo: `dd-pipelines/temper-temperpaw.json` @ commit `df031d34`.
- Pre-deploy baseline query: `service:openpaw "entity actor stopped"`, time range `now-6h..now`, grouped by `status`.
- Post-deploy verification query: same filter, time range `now-2m..now`.

## Architecture Diagram

```text
BEFORE (broken):
  otlp log {status: "CallingProvider", ...} ──▶ DD ingests
                                                  │
                                                  ▼
                                          attribute "status" = "CallingProvider"
                                                  │
                                                  ▼
                                          DD auto-classifies top-level "status"
                                          field as log severity → unknown enum
                                          maps to "critical" / "alert" / "emergency"

AFTER (fixed):
  otlp log {status: "CallingProvider", ...} ──▶ DD ingests
                                                  │
                                                  ▼
                                          [4] remap status → entity_status
                                                  │
                                                  ▼
                                          attribute "entity_status" = "CallingProvider"
                                          attribute "status" absent
                                                  │
                                                  ▼
                                          [5] status-remapper reads otel.severity_text
                                                  │
                                                  ▼
                                          DD log status = "info" ✓
```
