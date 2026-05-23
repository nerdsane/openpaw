# ADR-0051: Genesis app repair without app-install request queues

## Status

Accepted.

## Context

TemperPaw agents install and repair Temper apps through Genesis. The older
self-provisioning design included an app-install approval queue and installer
WASM. That created two app-install stories: one based on local requests and one
based on pinned Genesis refs. It also made human UX confusing because an app fix
could appear to be "staged" without actually becoming a Genesis version.

Production databases may contain historical request rows. Those rows are
production data and must not be reset, wiped, truncated, deleted, or migrated
destructively.

## Decision

Remove the app-install request queue from the active `paw-agent` app surface.
Agents use the native Genesis tool path:

- `temper.search_apps(...)`
- `temper.publish_app(...)`
- `temper.update_app(...)`
- `temper.install_app({"app_ref":"owner/name@hash", ...})`

When an installed app, sensor, entity action, policy, WASM module, agent
definition, or seed data is wrong, the agent repairs the app package and
publishes the next Genesis version. It then installs the returned pinned ref and
verifies the broken behavior. Normal repairs are version updates of the same
Genesis app; forks/imports are lineage changes in Genesis.

The active model no longer exposes the old request entity, policy, or installer
WASM. Existing production rows remain inert historical data.

## Consequences

- Agents have one normal app workflow: Genesis pinned refs.
- Human UX no longer has a separate app-install queue to reconcile.
- Warm restarts preserve existing installed app state from the database and skip
  unchanged bootstrap refs.
- Future approval UX must wrap Genesis pinned-ref publish/install semantics
  instead of reintroducing a parallel app-install mechanism.

## Verification

- Contract tests assert the old request entity, policy, and installer module are
  absent from active app surfaces.
- Skills and docs teach Genesis search/publish/update/install as the default
  repair flow.
- Live deployment verification must not reset, wipe, truncate, replace, restore,
  or manually clean production database rows.
