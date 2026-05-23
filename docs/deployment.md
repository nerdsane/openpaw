# Deployment

## Preferred path

Run `temperpaw deploy`. The CLI is responsible for:

1. Checking prerequisite CLIs.
2. Provisioning Turso and R2.
3. Creating the Railway project.
4. Seeding the admin account and dashboard-managed secrets.

## Railway fallback

If you prefer the one-click path, use the Railway button in the README and provide the infrastructure variables manually:

- `TURSO_URL`
- `TURSO_AUTH_TOKEN`
- `BLOB_ENDPOINT`
- `BLOB_BUCKET`
- `BLOB_ACCESS_KEY`
- `BLOB_SECRET_KEY`

## Genesis app source

Production TemperPaw app capabilities should come from Genesis, not from the
repo-local app catalog. Configure fresh-instance bootstrap with pinned refs:

- `TEMPERPAW_GENESIS_REGISTRY_URL=https://genesis-production-164d.up.railway.app`
- `TEMPERPAW_GENESIS_REGISTRY_TENANT=default`
- `TEMPERPAW_GENESIS_BOOTSTRAP_REFS=temperpaw/paw-fs@HASH,temperpaw/paw-agent@HASH`

On restart with the same database, TemperPaw restores installed Genesis app
metadata and skips unchanged bootstrap refs. Do not reset, wipe, replace, or
restore over a production database. Only a genuinely new empty database is a
fresh instance that installs the configured pinned refs once.

## Optional Datadog

Deploy the collector config in `scripts/otel-collector-railway.yaml` as a second Railway service and point `OTEL_EXPORTER_OTLP_ENDPOINT` at the collector's internal hostname.
