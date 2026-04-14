# Deployment

## Preferred path

Run `openpaw deploy`. The CLI is responsible for:

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

## Optional Datadog

Deploy the collector config in `scripts/otel-collector-railway.yaml` as a second Railway service and point `OTEL_EXPORTER_OTLP_ENDPOINT` at the collector's internal hostname.
