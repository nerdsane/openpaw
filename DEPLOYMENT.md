# TemperPaw Deployment Guide

TemperPaw is a **single-user, self-hosted** platform. Each deployment serves one operator (person or team) with one Turso database, one blob storage bucket, and one compute instance. There is no multi-tenant SaaS mode.

---

## Architecture Overview

```
                   GitHub Actions (CI)
                         |
                    Docker build
                         |
                         v
                  GHCR (ghcr.io/nerdsane/temperpaw)
                    :edge  :latest  :sha-*  :semver
                         |
                         v
         +----- Railway (compute) ------+
         |                              |
         |  temperpaw (main service)      |
         |    port 3467                 |
         |    /readyz readiness check   |
         |                              |
         |  otel-collector (sidecar)    |
         |    port 4318 (internal)      |
         |    -> Datadog (if DD_API_KEY)|
         +------------------------------+
                   |           |
                   v           v
           Turso (DB)    Cloudflare R2 (blobs)
```

**Current providers:**

| Component | Provider | Free Tier |
|-----------|----------|-----------|
| Compute | Railway | 512 MB RAM, 1 vCPU |
| Database | Turso (libSQL) | 9 GB, 500M rows |
| Blob storage | Cloudflare R2 | 10 GB |
| Container registry | GitHub Container Registry (GHCR) | Unlimited for public repos |
| Observability | Datadog (via OTEL) | Optional |

**Future direction:** The deployment CLI (`temperpaw deploy`) is designed to be extended to other cloud providers. The core app is a single static binary + dashboard assets + WASM modules, so it can run anywhere that supports Docker or bare metal. Turso and R2 could be swapped for any libSQL-compatible DB and S3-compatible storage.

---

## Quick Start: Automated Deploy

The fastest path from zero to a running instance:

```bash
# Install the CLI
cargo install temperpaw-cli

# Run the interactive deploy wizard
temperpaw deploy
```

This command:
1. Installs Railway CLI, Turso CLI, and Wrangler if not present
2. Authenticates with each service (interactive browser flows)
3. Creates a Turso database named `temperpaw-<username>`
4. Creates an R2 bucket named `temperpaw-fs-<username>`
5. Creates a Railway project named `temperpaw-<username>` with two services
6. Seeds all environment variables (DB credentials, blob keys, LLM keys, OTEL config)
7. Deploys the OTEL collector sidecar
8. Deploys the pre-built Docker image from GHCR
9. Assigns a public domain and polls `/readyz` until the new deployment is ready
10. Prints the dashboard URL

### LLM credential auto-detection

The deploy wizard checks environment variables in this order:
- `ANTHROPIC_API_KEY` -> sets `LLM_PROVIDER=anthropic`
- `OPENROUTER_API_KEY` -> sets `LLM_PROVIDER=openrouter`
- `OPENAI_API_KEY` -> sets `LLM_PROVIDER=openai`
- OpenAI Codex subscription auth is configured in the dashboard via device-code login.

If none are found, you can configure the LLM provider later via the dashboard Settings page.

---

## Docker Image

### Build pipeline

GitHub Actions (`.github/workflows/docker.yml`) builds the Docker image on:
- **Push to `main`** -> tagged `edge` and `sha-<short-hash>`
- **Published release** -> tagged `latest`, semver (`v1.2.3`, `v1.2`)

The image is pushed to `ghcr.io/nerdsane/temperpaw`.

### Multi-stage Dockerfile

The image is built in three stages:

1. **`dashboard-build`** (Node 22): Installs npm dependencies and builds the SvelteKit dashboard (`npm run build`)
2. **`rust-build`** (Rust 1.94): Compiles the `temperpaw-server` binary, embedding the pre-built dashboard assets
3. **Runtime** (Debian Bookworm slim): Copies the binary, dashboard build, and `os-apps/` (entity specs, WASM modules, Cedar policies)

Build args:
- `BUILD_VERSION` — version string (git tag or `sha-<hash>`)
- `BUILD_SHA` — full git commit SHA

The final image exposes **port 3467** and runs `./temperpaw`.

### Building locally

```bash
docker build -t temperpaw:local \
  --build-arg BUILD_VERSION=dev \
  --build-arg BUILD_SHA=$(git rev-parse HEAD) \
  .
```

---

## Railway Configuration

### `railway.toml`

```toml
[build]
builder = "dockerfile"
dockerfilePath = "Dockerfile"

[deploy]
healthcheckPath = "/readyz"
healthcheckTimeout = 3600
restartPolicyType = "ON_FAILURE"
restartPolicyMaxRetries = 3
```

Railway builds the Docker image from the repo's Dockerfile. The deployment health check hits `/readyz` on the service's assigned port, while `/healthz` remains process liveness. TemperPaw cold boots can take a long time while the server restores state and reconciles OS apps, so the health window is intentionally set to one hour and traffic should not move to a new container until readiness succeeds.

### Two-service architecture

1. **`temperpaw`** — The main application. Serves the dashboard, OData API, and agent runtime.
2. **`otel-collector`** — OpenTelemetry Collector sidecar. Receives traces/metrics from temperpaw via Railway's private network (`otel-collector.railway.internal:4318`) and exports to Datadog (or debug logs if no DD_API_KEY).

### Pre-built image deploy

The `temperpaw deploy` CLI does **not** build from source on Railway. Instead, it creates a thin Dockerfile that pulls the pre-built image from GHCR:

```dockerfile
ARG IMAGE_TAG=latest
FROM ghcr.io/nerdsane/temperpaw:${IMAGE_TAG}
```

This means deploys are fast — Railway just pulls the image instead of compiling Rust from scratch.

---

## Environment Variables

These are set on the `temperpaw` Railway service:

### Required

| Variable | Description |
|----------|-------------|
| `TURSO_URL` | libSQL database URL (e.g., `libsql://temperpaw-xxx.turso.io`) |
| `TURSO_AUTH_TOKEN` | Turso authentication token |
| `BLOB_ENDPOINT` | R2/S3-compatible endpoint URL |
| `BLOB_BUCKET` | Bucket name |
| `BLOB_ACCESS_KEY` | S3 access key ID |
| `BLOB_SECRET_KEY` | S3 secret access key |

### LLM Configuration

| Variable | Description |
|----------|-------------|
| `LLM_PROVIDER` | Provider name: `anthropic`, `openai`, `openai_codex`, `openrouter` |
| `ANTHROPIC_API_KEY` | Anthropic API key (if using Anthropic) |
| `OPENAI_API_KEY` | OpenAI API key (if using OpenAI) |
| `OPENAI_CODEX_TOKEN` | Legacy Codex OAuth token fallback. Prefer dashboard device-code login. |
| `OPENROUTER_API_KEY` | OpenRouter API key (if using OpenRouter) |

### Observability

| Variable | Description |
|----------|-------------|
| `OTEL_ENABLED` | Set to `true` to enable OTEL export |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | Collector endpoint (set automatically: `http://otel-collector.railway.internal:4318`) |
| `DD_API_KEY` | Datadog API key (set on `otel-collector` service) |
| `DD_SITE` | Datadog site (e.g., `datadoghq.com`) |

### Railway Integration (for dashboard redeploy)

| Variable | Description |
|----------|-------------|
| `RAILWAY_TOKEN` | Project-scoped Railway API token |
| `RAILWAY_PROJECT_ID` | Railway project UUID |
| `RAILWAY_ENVIRONMENT_ID` | Railway environment UUID |
| `RAILWAY_SERVICE_ID` | Railway service UUID for the temperpaw service |
| `RAILWAY_OTEL_SERVICE_ID` | Railway service UUID for the otel-collector |

These enable the dashboard's "Deploy latest build" and "Update" buttons to trigger redeployments without leaving the browser.

### Build metadata

| Variable | Description |
|----------|-------------|
| `BUILD_VERSION` | Set at build time (git tag or sha) |
| `BUILD_SHA` | Set at build time (full commit SHA) |

---

## Dashboard Redeploy Integration

The running TemperPaw server exposes Railway management endpoints:

- **`POST /paw/infra/railway/redeploy`** — Triggers a Railway service redeployment via the Railway GraphQL API (`serviceInstanceRedeploy` mutation). Accepts an optional `image_tag` parameter (`"latest"` for stable releases, `"edge"` for latest main build).
- **`GET /paw/infra/railway/status`** — Returns current deployment status.
- **`POST /paw/infra/railway/set-var`** — Sets an environment variable on the Railway service.

These endpoints require the four `RAILWAY_*` secrets to be configured in the vault.

The dashboard sidebar shows:
- **"Update"** button — when a new release is available, redeploys with `image_tag=latest`
- **"Deploy latest build"** button — redeploys with `image_tag=edge` (latest `main` commit)

---

## Manual Deployment

### Using Railway CLI directly

If you have the Railway CLI linked to your project:

```bash
# Trigger a redeploy of the current image
railway redeploy -y

# Or deploy from the repo (builds from Dockerfile on Railway)
railway up -s temperpaw
```

### Using Docker directly

```bash
docker run -d \
  -p 3467:3467 \
  -e TURSO_URL=libsql://your-db.turso.io \
  -e TURSO_AUTH_TOKEN=your-token \
  -e BLOB_ENDPOINT=https://your-r2-endpoint \
  -e BLOB_BUCKET=your-bucket \
  -e BLOB_ACCESS_KEY=your-key \
  -e BLOB_SECRET_KEY=your-secret \
  -e LLM_PROVIDER=anthropic \
  -e ANTHROPIC_API_KEY=sk-ant-... \
  ghcr.io/nerdsane/temperpaw:edge
```

### First-time setup

After the server starts:

1. Navigate to `http://<host>:3467/dashboard`
2. The login page detects no accounts exist and shows "Create your account"
3. Create the admin account (email + password)
4. Configure LLM provider and API key in Settings (if not set via env vars)
5. The Paw agent is automatically bootstrapped on first boot

---

## OTEL Collector

The collector runs as a separate Railway service with two modes:

- **Datadog mode** (when `DD_API_KEY` is set): Exports traces, metrics, and logs to Datadog
- **Debug mode** (no `DD_API_KEY`): Logs traces to stdout for troubleshooting

The collector is reachable from the temperpaw service via Railway's private network at `otel-collector.railway.internal:4318`. To enable Datadog later, add `DD_API_KEY` to the `otel-collector` service in the Railway dashboard — the collector auto-detects it on restart.

---

## Development

### Running locally

```bash
# Start the server (uses .env for credentials)
cargo run -p temperpaw --bin temperpaw-server

# Dashboard dev server (hot reload)
cd dashboard && npm run dev
```

The local server runs on `http://localhost:3467`. The dashboard dev server proxies API calls to the Rust server.

### Required local env vars

Create a `.env` file in the repo root:

```
TURSO_URL=libsql://...
TURSO_AUTH_TOKEN=...
BLOB_ENDPOINT=https://...
BLOB_BUCKET=...
BLOB_ACCESS_KEY=...
BLOB_SECRET_KEY=...
LLM_PROVIDER=anthropic
ANTHROPIC_API_KEY=sk-ant-...
```

---

## Troubleshooting

**Server won't start / panics on boot:**
- Check that all required env vars are set (especially `TURSO_URL` and `TURSO_AUTH_TOKEN`)
- Check Railway logs: `railway logs` or view in the Railway dashboard

**"Unresolved secret template" error in Paw chat:**
- The configured LLM provider's API key is missing or empty
- Go to Settings in the dashboard and verify the LLM provider and API key are set
- The WASM auto-fallback will try other providers if available, but at least one must have a valid key

**Health check failing:**
- The server takes time to boot (compiling WASM, running migrations)
- Railway's health check timeout is 60 seconds — increase if needed
- Check logs for startup errors

**Dashboard shows "Loading..." forever:**
- The API may be unreachable — check that the server is running
- Check browser console for CORS or network errors

**Deploy buttons not working:**
- Ensure `RAILWAY_TOKEN`, `RAILWAY_PROJECT_ID`, `RAILWAY_ENVIRONMENT_ID`, and `RAILWAY_SERVICE_ID` are set
- These are seeded automatically by `temperpaw deploy` but need manual setup if you deployed differently
