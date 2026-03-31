# Datadog Webhook Setup

This document describes the OpenPaw-facing Datadog webhook contract used by the current self-heal loop.

## Endpoint

Point the Datadog webhook integration at:

```text
https://<your-openpaw-host>/webhooks/ingest
```

For local development, expose the daemon with your preferred tunnel and use the public URL.

## Recommended OpenPaw Env Vars

```bash
DD_API_KEY=...
DD_APP_KEY=...
DD_SITE=datadoghq.com
WEBHOOK_SECRET=...
```

- `DD_API_KEY` and `DD_APP_KEY` are used by the Datadog query tool.
- `DD_SITE` defaults to `datadoghq.com`.
- `WEBHOOK_SECRET` is optional but recommended so incoming requests can be HMAC-verified.

## Signature Header

If `WEBHOOK_SECRET` is configured, OpenPaw expects one of these headers:

- `x-webhook-signature-256`
- `x-webhook-signature`

The value should be the hex-encoded `sha256` HMAC of the raw JSON payload, optionally prefixed with `sha256=`.

## Payload Shape

OpenPaw accepts two Datadog styles:

1. An OpenPaw envelope:

```json
{
  "source": "datadog",
  "event_type": "alert_fired",
  "payload": {
    "id": "123456789",
    "title": "deep-sci-fi npm ci failure",
    "alert_transition": "Triggered",
    "priority": "P1",
    "project_harness_id": "project-harness-id",
    "repo_url": "https://github.com/arni-labs/deep-sci-fi.git",
    "query": "synthetic:deep-sci-fi:npm-ci-lockfile-drift"
  }
}
```

2. A native Datadog-style body, which OpenPaw will normalize automatically:

```json
{
  "org": { "id": 12345, "name": "example" },
  "id": "123456789",
  "title": "deep-sci-fi npm ci failure",
  "text": "package-lock drift detected",
  "alert_type": "error",
  "alert_transition": "Triggered",
  "priority": "P1",
  "project_harness_id": "project-harness-id",
  "repo_url": "https://github.com/arni-labs/deep-sci-fi.git",
  "query": "synthetic:deep-sci-fi:npm-ci-lockfile-drift"
}
```

## Recovery Events

To mark a fixed alert as recovered, send the same monitor identity with:

```json
{
  "org": { "id": 12345 },
  "id": "123456789",
  "title": "deep-sci-fi npm ci failure",
  "alert_transition": "Recovered",
  "text": "monitor returned to normal"
}
```

When the matching `AlertCycle` is already in `Fixed` or `Verifying`, OpenPaw will dispatch `AlertResolved`.

## Useful Fields

OpenPaw currently reads these fields when present:

- monitor identity: `id`, `monitor_id`, `dd_monitor_id`, `monitor.id`, `monitor.slug`, `monitor.name`
- severity: `severity`, `priority`
- summary: `summary`, `title`, `name`, `event_title`
- failure detail: `reproduction.failure`, `failure`, `error.message`, `body`, `message`, `text`
- project context: `project_harness_id`, `repo_url`
- reply routing: `reply_channel_entity_id`, `reply_thread_id`, `reply_channel_id`

## Verification

Use these local proof drivers after the daemon is running:

- `python3 scripts/prove_datadog_webhook.py`
- `python3 scripts/prove_webhook_to_sre.py`
- `python3 scripts/prove_autonomous_alert.py`
