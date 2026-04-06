# paw-ingest

Webhook ingress pipeline. Receives external webhooks, validates signatures, routes to target entities, and processes the dispatched action.

## Entity Types

### WebhookEvent
One incoming webhook flowing through validation, routing, and processing.

- **States**: Created -> Validating -> Routing -> Processing -> Processed / Rejected
- **Key actions**: `Received` (raw_payload, raw_headers, route_key), `Validated` (source_type, hmac_verified), `Routed` (target_entity_type, target_entity_id, target_action), `Processed`
- **Failure actions**: `ValidationFailed`, `RouteFailed`, `ProcessFailed` — all transition to Rejected
- **WASM**: `validate_webhook` (HMAC verification, normalization), `route_webhook` (match route key to target), `process_webhook` (dispatch action on target entity)

### WebhookRoute
Configuration entity mapping route keys to target entities and actions.

- **States**: Active <-> Disabled
- **Key actions**: `Register` (route_key, source_type, event_filter, target_entity_type, target_action, webhook_secret), `Update`, `Disable`, `Enable`
- **Options**: `monitor_resolution_enabled`, `dedup_enabled`, `dedup_window_minutes`

## Setup

No dependencies. Register WebhookRoute entities for each external source, then POST webhooks to the ingress endpoint with the route key.
