# Ramp-Style Monitoring Setup — Outcome Report

## Goal
Agents set up Ramp-style self-maintaining monitoring for deep-sci-fi, as described in memory.

## What agents accomplished

### Ren (Lead Agent)
- Retrieved Datadog API key from Temper vault
- Spawned SWE for instrumentation work
- Created **13 real Datadog monitors** on the production Datadog account
- Set up WebhookRoute entities for Datadog alert → AlertCycle routing
- Configured Datadog webhook integration
- Steered SWE during execution
- Completed sessions at 79 and 138 turns (past old 26-turn limit)

### SWE (Software Engineer Agent)
- Cloned deep-sci-fi repo in sandbox
- PR #90: Fixed OTLP intake endpoint + added DD resource headers (merged)
- PR #91: Replaced broken OTLP with ddtrace-run agentless mode (merged)
- Tests ran in sandbox

### Datadog Monitors Created (verified on DD account)
1. High Error Rate (5xx)
2. High P99 Latency
3. High P95 Latency
4. DB Query Latency High
5. DB Connection Errors
6. 4xx Client Error Spike
7. High CPU Usage
8. High Memory Usage
9. Service Metrics Missing
10. /worlds Endpoint Errors
11. pgvector Query Latency
12. Dweller Interaction Errors
13. Request Volume Anomaly

## What I (human/Claude) did

### Human actions required
1. Granted Cedar policy `agent-session-management` (one-time)
2. Merged PR #90 (should have been agent — agent couldn't merge due to no review flow)
3. Merged PR #91 (same)
4. Set Railway DD env vars via CLI (DD_API_KEY, DD_SITE, DD_ENV, DD_SERVICE, etc.)
5. Approved Railway deployment from dashboard
6. Provided fresh Railway token (old one expired)
7. Provided fresh Anthropic API key (old one rate-limited)

### Platform fixes made during this mission
| Fix | Commit | What |
|-----|--------|------|
| Coroutine bug | fde8767e | All tool calls return real values, not coroutines |
| Sandbox auth | b47500c5 | sandbox.bash() works with Tensorlake auth |
| Sandbox API format | b47500c5 | Correct /api/v1/processes format |
| REPL memory | 86620ca9 | Sessions run 200+ turns (was 26 limit) |
| temper.done() | a6044746 | Agents can signal completion |
| Railway/Vercel tokens | a6044746 | Tokens wired into integration config |
| Self-provisioning | 1d334ec6 | Agents create specs, WASM, policies |
| Cedar restart | 196164f6 | Daemon survives restarts |
| temper.get_secret() | 85d9ba15 | Agents read secrets from vault |
| Dashboard sort | 86620ca9 | Newest sessions shown first |

## What's NOT working yet

### Traces not flowing to Datadog
The ddtrace-run instrumentation is deployed but traces are not arriving in Datadog APM.
- Both PRs are merged and deployed on Railway
- ddtrace>=2.0.0 is installed
- DD_API_KEY and DD_SITE are set
- ddtrace-run is in start.sh
- The service is healthy (200 on /health)
- But zero spans show in Datadog APM

Likely cause: ddtrace agentless mode configuration issue. May need:
- DD_TRACE_AGENT_URL pointed at correct intake
- Or a Datadog Agent sidecar (Railway doesn't support this natively)
- Or switch back to OTLP approach with correct endpoint

### Full loop not closed
- Traces not flowing → monitors stay "No Data" → webhooks never fire → SRE never auto-spawns
- The infrastructure is all in place but the data pipeline isn't connected

### Agent limitations discovered
- SWE ran 230 turns without calling temper.done() (fixed with done signal)
- SWE hit Anthropic rate limit at turn 103
- Ren spent 100+ turns fighting Railway API
- Agents can't approve Railway deployments (NEEDS_APPROVAL state)
- Agents can't set empty env vars on Railway

## Architecture that works
```
Human → Ren (lead) → spawns SWE/SRE sessions → sandbox work → PRs → merge
                    → Datadog API calls → monitors created
                    → WebhookRoutes → AlertCycle routing
                    → temper.done() → clean completion
```

## What would make this complete
1. Fix ddtrace → Datadog trace flow (likely a config issue)
2. Agent reviews and merges PRs (currently human does this)
3. Agent monitors Railway deploy status via API
4. SRE auto-spawns on AlertCycle and triages
