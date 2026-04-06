# SRE Monitoring — Deep Sci-Fi

This skill document is auto-injected into SRE agent prompts via the Harness. It covers Datadog monitoring patterns, alert triage, and health scan workflows for the deep-sci-fi platform.

## Datadog Monitoring Patterns

### Monitor types for deep-sci-fi

| Type | What it catches | Example |
|------|----------------|---------|
| Error rate | Spike in HTTP 5xx responses | `avg(last_5m):sum:trace.fastapi.request.errors{service:deep-sci-fi-api} > 5` |
| Latency | Slow endpoints (p95/p99) | `avg(last_5m):p95:trace.fastapi.request.duration{service:deep-sci-fi-api} > 2000` |
| Exception tracking | Unhandled exceptions | `logs("service:deep-sci-fi-api status:error").rollup("count").last("5m") > 10` |
| Endpoint health | Specific endpoint availability | `avg(last_5m):avg:http.status_code{service:deep-sci-fi-api,resource_name:GET /api/v1/worlds} > 399` |
| Database | Connection pool exhaustion, slow queries | `avg(last_5m):avg:postgresql.connections.active{service:deep-sci-fi-db} > 80` |
| Embedding pipeline | pgvector query latency | `avg(last_5m):p95:db.query.duration{service:deep-sci-fi-api,query_type:similarity_search} > 500` |
| Lockfile drift | package-lock.json out of sync | Synthetic monitor: `deep-sci-fi:npm-ci:lockfile-drift` |

### Monitor coverage target

Aim for approximately 1 monitor per 75 lines of critical code. "Critical code" means:
- API route handlers
- Database model operations
- Embedding pipeline functions
- Authentication/authorization logic
- Payment or billing logic (if applicable)
- Deployment scripts

Non-critical code (UI components, static pages, dev tooling) does not need monitoring.

## Using the datadog_query Tool

The `datadog_query` tool supports three query kinds:

### monitor_status
Check the current status of monitors.
```
query_kind: monitor_status
query: "deep-sci-fi"
```
Returns: list of monitors matching the query with their current status (OK, ALERT, WARN, NO DATA).

### recent_events
Fetch recent Datadog events.
```
query_kind: recent_events
query: "sources:deep-sci-fi priority:all"
time_range: "last_1h"
```
Returns: recent events with timestamps, titles, and descriptions.

### metrics_query
Query raw metrics data.
```
query_kind: metrics_query
query: "avg:trace.fastapi.request.duration{service:deep-sci-fi-api}"
time_range: "last_1h"
```
Returns: time series data points.

## Assessing Monitor Coverage

To assess whether monitoring is adequate:

1. **List existing monitors** — Use `datadog_query` with `query_kind: monitor_status` to get all deep-sci-fi monitors
2. **Map monitors to code** — For each API route, database operation, and critical function, check if a monitor exists
3. **Identify gaps** — Critical code paths without monitoring
4. **Check staleness** — Monitors that reference deleted or renamed endpoints, old service names, or deprecated metrics
5. **Verify thresholds** — Thresholds that are too loose (never fires) or too tight (constant noise)

## When to Create vs. Tune vs. Delete

### Create a new monitor when:
- A new API endpoint is deployed without monitoring
- A new critical code path is added (embedding pipeline, auth flow)
- A production incident reveals an unmonitored failure mode
- Monitor coverage drops below the 1:75 target for critical code

### Tune an existing monitor when:
- It fires frequently but the underlying issue is not actionable (threshold too tight)
- It never fires and you suspect the threshold is too loose
- The monitored service's normal behavior has changed (e.g., higher baseline latency after a feature launch)
- Dedup windows are too short or too long

### Delete a monitor when:
- The monitored endpoint or service no longer exists
- The monitor is fully redundant with another monitor
- The monitor has been in NO DATA state for over 30 days with no path to recovery

## Alert Triage Workflow

When a Datadog alert fires and creates an AlertCycle entity:

### Step 1: Classify
- **Real issue** — The alert indicates a genuine problem affecting users or system health
- **Noise** — The alert fired due to a transient spike, threshold misconfiguration, or non-issue
- **Unknown** — Need more data to determine

### Step 2: For real issues
1. Create an Issue entity describing the problem
2. Create a WorkCycle entity linked to the Issue
3. Assign to SWE with specific instructions (what's broken, where to look, what success looks like)
4. Monitor the WorkCycle progress
5. After fix is deployed, verify the monitor returns to OK

### Step 3: For noise
1. Tune the monitor (adjust threshold, add dedup, narrow scope)
2. If the monitor is fundamentally broken, delete and replace
3. Document why it was noise so the pattern is recognized next time

### Step 4: For unknown
1. Gather more data — check Logfire traces, query metrics history, review recent deploys
2. If still unclear after investigation, escalate to Ren with findings
3. Do not close the AlertCycle until classification is resolved

## Periodic Health Scan Workflow

The SRE cron job runs every 6 hours (`0 */6 * * *`). Each run should:

1. **Fetch all monitors** — `datadog_query` with `monitor_status` for deep-sci-fi
2. **Check monitor health** — Any monitors in ALERT, WARN, or NO DATA?
3. **Compare against codebase** — Read the current API routes and critical code paths. Are there new endpoints without monitors?
4. **Check for stale monitors** — Do all monitors reference code that still exists?
5. **Assess coverage ratio** — Calculate monitors per critical lines. Flag if below target.
6. **Report findings** — Update the CronJob entity with results. If action is needed, create an Issue.

### What to include in the scan report:
- Monitor count (total, OK, ALERT, WARN, NO DATA)
- Coverage assessment (monitored vs. unmonitored critical paths)
- Stale monitors found (if any)
- New endpoints needing monitors (if any)
- Recommended actions (create, tune, or delete specific monitors)
