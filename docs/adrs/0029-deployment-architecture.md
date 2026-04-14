# ADR-0029: Deployment Architecture

**Status:** Accepted
**Date:** 2026-04-14
**Related:** ADR-0027 (deployment, dashboard auth, observability & DX), ADR-0028 (bounded startup surface and WASM artifact contract)

## Context

ADR-0027 established the deployment path: `openpaw` CLI provisions Turso Cloud, Cloudflare R2, and Railway, then deploys the server. ADR-0028 bounded the startup surface so warm restarts are fast. What remained unresolved was the concrete deployment architecture — how the Docker images are built, how the binary is structured for deploy vs local use, how the OTEL collector auto-configures, and how credentials are cached across deploy runs.

Several problems drove these decisions:

1. **Building on Railway causes OOM.** The full Temper stack is a large Rust workspace. Compiling it on Railway's free tier (512MB RAM) reliably runs out of memory, and even when it succeeds, builds take 25-30 minutes. This makes Railway-side builds unviable.

2. **The CLI and server have fundamentally different profiles.** The deploy/doctor/run CLI needs to be lightweight and build instantly for contributor DX. The server binary links the full Temper runtime, WASM engine, embedded dashboard, and all OS-app artifacts. Forcing contributors to compile the full server just to run `openpaw doctor` is wasteful.

3. **Observability should be zero-config.** The OTEL collector needs to work in two modes — Datadog export for production, debug logging for development — without requiring the user to choose a config file or pass flags. The decision should be driven by whether credentials are present.

4. **Re-entering credentials on every deploy is painful.** Cloudflare R2 API tokens, Turso auth tokens, and Railway project tokens are long-lived. Requiring the user to paste them on every `openpaw` invocation makes re-deploys and updates unnecessarily tedious.

5. **First boot is slow on Turso Cloud.** When the server connects to a fresh Turso database, it creates tables and indexes for all entity types. This takes approximately 3 minutes due to Turso Cloud's edge replication latency, well beyond typical health check timeouts.

## Decision

### 1. Pre-built Docker images via GitHub Actions and GHCR

Docker images are built in CI (GitHub Actions), pushed to the GitHub Container Registry (`ghcr.io`), and pulled by Railway at deploy time. Railway never builds from source.

The CI pipeline compiles the Rust workspace in a multi-stage Docker build with access to full GitHub Actions runner resources (7GB RAM, 2 vCPU). The resulting image contains only the compiled binary, OS-app WASM artifacts, and the embedded dashboard static build.

This reduces deploy time from ~30 minutes (Railway source build) to ~10 seconds (image pull). It also eliminates the OOM failures that made Railway-side builds unreliable on the free tier.

### 2. CLI/Server binary split

The project produces two binaries:

- **`openpaw`** — A lightweight CLI that handles `deploy`, `run`, `doctor`, and other developer-facing commands. It compiles in under 1 second because it does not link the Temper runtime or WASM engine. When the user runs `openpaw run`, the CLI locates and `exec`s `openpaw-server`.

- **`openpaw-server`** — The full Temper stack: HTTP server, entity runtime, WASM engine, embedded dashboard, OTEL instrumentation. This is what runs inside the Docker container in production.

The split means contributors can build and iterate on the CLI without compiling the server. The CLI is the entry point for all user interactions; the server is an implementation detail that runs either locally (found via PATH or a well-known location) or in Docker.

### 3. OTEL collector with dynamic entrypoint

The OTEL collector runs as a separate container (see ADR-0027, decision 6). Its entrypoint script auto-detects the telemetry backend at startup:

- If `DD_API_KEY` is set on the collector service, the entrypoint selects `otel-datadog.yaml`, which configures the Datadog exporter (metrics, traces, logs forwarded to Datadog).
- If `DD_API_KEY` is not set, the entrypoint selects `otel-debug.yaml`, which configures the debug exporter (all telemetry logged to stdout for `railway logs` inspection).

No flags, no environment variable switches beyond the API key itself. To activate Datadog, set `DD_API_KEY` on the collector service. To deactivate, remove it. The collector restarts and picks the correct config automatically.

### 4. Credential caching

Deploy tokens are cached locally at `~/.local/share/openpaw/deploy_cache.json` with `0600` file permissions (owner read/write only). The cache stores:

- Cloudflare R2 API tokens and bucket configuration
- Turso Cloud database URL and auth token
- Railway project and service identifiers

On subsequent runs, `openpaw deploy` reads cached credentials and skips the provisioning prompts. This makes re-deploys and updates idempotent — running `openpaw deploy` again after a code change just pushes the new image and restarts the service, with no credential re-entry.

The cache location follows the XDG Base Directory Specification (`$XDG_DATA_HOME/openpaw/` or `~/.local/share/openpaw/`). The `0600` permissions ensure credentials are not readable by other users on shared machines.

### 5. Three-tier free infrastructure

Production infrastructure uses three managed services, all within their free tiers:

| Tier | Service | Role | Free tier limits |
|------|---------|------|------------------|
| Database | Turso Cloud | SQLite-compatible database (libSQL) | 9GB storage, 500M rows read/month |
| Blob storage | Cloudflare R2 | S3-compatible object storage | 10GB storage, 1M Class A ops/month |
| Compute | Railway | Container hosting | 512MB RAM, 1 vCPU, 500 execution hours/month |

No credit card is required for any of these services. This makes OpenPaw deployable at zero cost for personal use and experimentation.

The free tier limits are sufficient for single-user deployments with moderate usage. If any tier is exhausted, the respective service degrades gracefully (Turso returns read errors, R2 returns 429s, Railway suspends the service) rather than incurring charges.

### 6. Railway private networking for telemetry

The OTEL collector and the OpenPaw server communicate over Railway's private network. The server exports telemetry to `otel-collector.railway.internal:4318` (OTLP/HTTP). This DNS name resolves only within the Railway project's private network.

No telemetry endpoints are exposed publicly. The collector's public port mapping is disabled. This prevents accidental exposure of trace data, metrics, or log payloads to the internet.

### 7. Health check timeout of 300 seconds

The Railway health check timeout is set to 300 seconds (5 minutes). This accommodates first-boot latency caused by Turso Cloud entity creation.

On first connect to a fresh Turso database, the Temper runtime creates tables and indexes for all registered entity types. Due to Turso Cloud's edge replication model (writes propagate from the primary to edge replicas), this schema creation takes approximately 3 minutes. Subsequent restarts are fast because the schema already exists.

The 300-second timeout provides margin above the observed ~180-second first-boot time. After first boot, health checks respond in under 1 second. Railway does not penalize long health check timeouts on warm restarts — it only waits the full duration if the check has not yet succeeded.

## Consequences

### Positive

- **Deploy in seconds, not minutes.** Pre-built images eliminate the 30-minute Railway build and the OOM risk on 512MB instances.
- **Fast CLI iteration.** Contributors build the CLI in <1s without compiling the full server. `openpaw doctor` and `openpaw deploy` are instantly available.
- **Zero-config observability.** The OTEL collector auto-selects the right exporter. No flags to remember, no config files to edit.
- **Idempotent deploys.** Cached credentials make `openpaw deploy` safe to run repeatedly. Update the code, run deploy, done.
- **Zero-cost hosting.** All three infrastructure services have free tiers sufficient for personal use. No credit card barrier to trying OpenPaw.
- **No telemetry exposure.** Private networking ensures trace data and metrics never leave the Railway project network.

### Negative

- **CI dependency for image builds.** Deploys depend on GitHub Actions having successfully built and pushed the image. If CI is broken or GHCR is down, deploys are blocked until the pipeline is fixed.
- **Two binaries to manage.** The CLI/server split means contributors need to understand that `openpaw` and `openpaw-server` are separate artifacts with different build profiles.
- **Credential cache is a local secret store.** `deploy_cache.json` contains production tokens. If the user's machine is compromised, these credentials are exposed. Mitigation: `0600` permissions and XDG-standard location.
- **300-second health check timeout** may mask genuine startup failures on first deploy. If the server is crash-looping, Railway waits 5 minutes before marking it unhealthy. Mitigation: structured startup logging makes the cause visible in `railway logs` immediately.

### Risks

- **Turso Cloud free tier changes.** If Turso reduces free tier limits, users may hit row-read or storage caps. Mitigation: OpenPaw's storage patterns are lightweight (entity metadata, not bulk data), and Turso's paid tier is inexpensive.
- **Railway private networking reliability.** Internal DNS resolution (`*.railway.internal`) is relatively new. If it fails, the OTEL collector cannot receive telemetry. Mitigation: the server continues to operate normally without a collector — telemetry is fire-and-forget via OTLP.
- **GHCR rate limits.** Anonymous pulls from `ghcr.io` are rate-limited. If the image is public and Railway pulls frequently (e.g., during rapid iteration), pulls may be throttled. Mitigation: Railway caches pulled images; re-deploys without image changes do not re-pull.
