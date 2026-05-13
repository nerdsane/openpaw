# TemperPaw Datadog Live Baseline

Date: 2026-05-11T16:19:36Z
Last local verification refresh: 2026-05-13T04:15:49Z
Last live Datadog/Railway verification refresh: 2026-05-13T07:54:12Z

Purpose: record the live Datadog state observed while converting the active system
from OpenPAW/OpenPaw/openpaw identity to TemperPaw and while designing the agent
session observability contract.

## Live public artifact URL verification refresh - 2026-05-13T07:54Z

Purpose: close the public URL gap found in the previous refresh without moving
the already-active `assets.katagami.ai` domain away from the Katagami bucket.

Domain and deployment:

- Added R2 custom domain `temperpaw-assets.katagami.ai` to the currently
  writable bucket `openpaw-fs-seshendranalla` with Wrangler:
  `wrangler r2 bucket domain add openpaw-fs-seshendranalla --domain temperpaw-assets.katagami.ai --zone-id 4d7abaf0f0010529691d6ebcb5e442a7 --min-tls 1.2 --force`.
- Wrangler reports the new domain as `enabled: Yes`,
  `ownership_status: active`, `ssl_status: active`, and `min_tls_version: 1.2`.
- Public authoritative DNS trace resolves
  `temperpaw-assets.katagami.ai` to Cloudflare A records with TTL 300.
- `PUBLISHED_BLOB_PUBLIC_BASE_URL` was changed to
  `https://temperpaw-assets.katagami.ai`.
- Railway deployment `fd079e03-0cf7-49d8-942e-2c180a35b4b3` reached
  `SUCCESS` at `2026-05-13T07:43:03.881Z` on the same wrapper digest
  `sha256:db79bb726d765572a9a0b9d3ab1ef7d9369698643ff0014ebc8f8d9c2ca08ee1`.
- `GET https://openpaw-production.up.railway.app/readyz` returned HTTP 200 in
  63 ms with `status:"ready"` and Discord `connected:true`.

Publish-artifact route proof:

- Authenticated `POST /api/files/publish-artifact` with label
  `codex-live-publish-aa5c69b-temperpaw-public-url` returned HTTP 200 in
  403 ms.
- Returned artifact id:
  `part-7989620b5854d0d2c7a05c3c41356c5e`.
- Returned public storage key:
  `codex-live-proof/CodexProof/fd079e03-0cf7-49d8-942e-2c180a35b4b3/codex-live-publish-aa5c69b-temperpaw-public-url-a7b843737b4e8d4eaab95a060898b7abbaad53b4b618dcbe2c18b14e5a7eeaa9.md`.
- Returned public URL:
  `https://temperpaw-assets.katagami.ai/codex-live-proof/CodexProof/fd079e03-0cf7-49d8-942e-2c180a35b4b3/codex-live-publish-aa5c69b-temperpaw-public-url-a7b843737b4e8d4eaab95a060898b7abbaad53b4b618dcbe2c18b14e5a7eeaa9.md`.
- Public URL read with Cloudflare public DNS resolution returned HTTP 200 in
  124 ms, downloaded 18,568 bytes, and produced SHA-256
  `a7b843737b4e8d4eaab95a060898b7abbaad53b4b618dcbe2c18b14e5a7eeaa9`,
  matching the source content hash.
- S3 `head-object` using the runtime credentials returned
  `content_length:18568`, `content_type:text/markdown`, ETag
  `"e8b8084858e0ab21ca8f805fd0028506"`, and
  `last_modified:2026-05-13T07:45:04+00:00`.
- Immediately after DNS creation, the local macOS/Tailscale resolver and
  Railway one-off shell resolver still returned NXDOMAIN for the new hostname,
  while authoritative Cloudflare DNS and public resolvers returned A records.
  Treat that as propagation/cache lag, not an object-store failure.

Datadog APM proof:

- Successful trace:
  `4f199e8e4b37b233e7c6844df074f304`.
- Trace deep link:
  `https://app.datadoghq.com/apm/trace/4f199e8e4b37b233e7c6844df074f304?graphType=flamegraph&shouldShowLegend=true&spanID=17717225011287168071&timeHint=1778658304509.9753&trace=4f199e8e4b37b233e7c6844df074f30417717225011287168071&traceQuery=`
- Trace hierarchy:
  `http.server.request POST /api/files/publish-artifact` -> API handler
  `POST /api/files/publish-artifact` -> `state.publish_file_artifact` with
  child spans `state.read_file_stream_indexed`, `state.put_public_blob`, and
  `postgres.upsert_published_artifact`.
- Root HTTP span: status OK, HTTP 200, duration 350.122048 ms,
  `service:temperpaw`, `env:prod`, `service.version:sha-aa5c69b`.
- `state.put_public_blob` duration was 203.424016 ms and includes
  `bucket:openpaw-fs-seshendranalla`, the full `storage_key`,
  `endpoint_host:075a5c0a617de3bdc08a44f9794b6f2f.r2.cloudflarestorage.com`,
  `mime_type:text/markdown`, `byte_length:18568`, and
  `http.status_code:"200"`.
- `postgres.upsert_published_artifact` duration was 14.080003 ms and contained
  a `published_artifacts` `INSERT ... ON CONFLICT DO UPDATE` child span plus a
  `published_artifacts` `SELECT` load child span.
- The parent `state.publish_file_artifact` span emitted
  `published artifact metadata persisted` with `metadata_backend:"postgres"`.

What this proves:

- New public artifact URLs now use the TemperPaw-specific host
  `temperpaw-assets.katagami.ai` instead of `assets.katagami.ai`.
- The returned public URL is readable through public Cloudflare DNS, returns the
  source markdown bytes, and preserves the source content hash.
- The remaining external storage identity gap is now narrower:
  `PUBLISHED_BLOB_BUCKET` and the Railway service/domain still include
  `openpaw`. Replacing that bucket name still requires new R2 S3 credentials or
  a planned object migration; it is no longer blocking readable public artifact
  URLs.

## Live Postgres metadata persistence verification refresh - 2026-05-13T07:21Z

Temper/TemperPaw revisions:

- TemperPaw branch: `codex/temperpaw-observability-live-image`
- TemperPaw commit: `aa5c69bbbfe17ab0185468a183fecc05ad52a6b9`
  (`ghcr.io/nerdsane/temperpaw:sha-aa5c69b`)
- Temper branch: `codex/temperpaw-llmobs-service-identity-main`
- Temper commit: `7b170cf71246e01c337e81062b54ea8c597b9293`
- Temper ADR: `docs/adrs/0085-published-artifacts-postgres-metadata.md`
- TemperPaw pins all Temper crates to
  `7b170cf71246e01c337e81062b54ea8c597b9293`.

Build and deploy:

- Docker workflow run `25783023822` completed successfully for commit
  `aa5c69bbbfe17ab0185468a183fecc05ad52a6b9` in 25m17s.
- Runtime Datadog identity was set to `DD_SERVICE=temperpaw`,
  `DD_ENV=prod`, `DD_VERSION=sha-aa5c69b`,
  `DD_GIT_COMMIT_SHA=aa5c69bbbfe17ab0185468a183fecc05ad52a6b9`, and
  `DD_GIT_REPOSITORY_URL=https://github.com/nerdsane/temperpaw`.
- The first Railway deploy of this image,
  `f90814d6-e628-4d40-947b-5e10d0a2734c`, reached `SUCCESS` at
  `2026-05-13T07:09:45.781Z` with wrapper digest
  `sha256:db79bb726d765572a9a0b9d3ab1ef7d9369698643ff0014ebc8f8d9c2ca08ee1`.
- Before that first startup, `PUBLISHED_BLOB_BUCKET` was changed to
  `katagami-published-assets` to test whether the public domain and write
  bucket could be consolidated by config alone. The publish route returned
  HTTP 500 with an R2 `403 Forbidden` from the bucket. This proves the current
  S3/R2 write credentials are scoped away from `katagami-published-assets`.
- `PUBLISHED_BLOB_BUCKET` was restored to `openpaw-fs-seshendranalla`, and the
  same image was redeployed as Railway deployment
  `4a719ff4-3ad4-4c29-bd15-93c20b24ef37`; it reached `SUCCESS` at
  `2026-05-13T07:14:41.265Z` with the same wrapper digest.
- `GET https://openpaw-production.up.railway.app/readyz` returned HTTP 200 in
  68 ms with `status:"ready"` and Discord `connected:true`.

Publish-artifact route proof:

- Authenticated OData read of
  `GET /tdata/Files('bootstrap-soul-file-paw')` returned HTTP 200 in 76 ms
  with `has_content:true`, MIME `text/markdown`, size `18568`, and content hash
  `sha256:a7b843737b4e8d4eaab95a060898b7abbaad53b4b618dcbe2c18b14e5a7eeaa9`.
- Authenticated `POST /api/files/publish-artifact` with label
  `codex-live-publish-aa5c69b-postgres-metadata-writable` returned HTTP 200 in
  544 ms.
- Returned artifact id:
  `part-199c6b292946f98651137e0ef65331c6`.
- Returned public storage key:
  `codex-live-proof/CodexProof/4a719ff4-3ad4-4c29-bd15-93c20b24ef37/codex-live-publish-aa5c69b-postgres-metadata-writable-a7b843737b4e8d4eaab95a060898b7abbaad53b4b618dcbe2c18b14e5a7eeaa9.md`.
- Returned public URL:
  `https://assets.katagami.ai/codex-live-proof/CodexProof/4a719ff4-3ad4-4c29-bd15-93c20b24ef37/codex-live-publish-aa5c69b-postgres-metadata-writable-a7b843737b4e8d4eaab95a060898b7abbaad53b4b618dcbe2c18b14e5a7eeaa9.md`.
- Curling the returned public URL returned HTTP 404 in 130 ms with a 9 byte
  `not found` body.
- `wrangler r2 object get` using the local Cloudflare OAuth token did not find
  the returned key in either visible bucket, but the AWS S3 API using the same
  Railway S3 credentials used by the runtime did find it in
  `openpaw-fs-seshendranalla`.
- S3 `list-objects-v2` returned `key_count:1` for the exact key. S3
  `head-object` returned `content_length:18568`, `content_type:text/markdown`,
  ETag `"e8b8084858e0ab21ca8f805fd0028506"`, and
  `last_modified:2026-05-13T07:16:46+00:00`.
- Direct `psql` from the local machine could not resolve Railway's private
  `postgres.railway.internal` host, so the durable metadata proof below uses
  the production Datadog APM SQL spans and metadata persistence event.

Datadog APM proof:

- Successful trace:
  `edff183f3a864f743ae74c13c14bb79f`.
- Trace deep link:
  `https://app.datadoghq.com/apm/trace/edff183f3a864f743ae74c13c14bb79f?graphType=flamegraph&shouldShowLegend=true&spanID=13788590875260027289&timeHint=1778656606545.4075&trace=edff183f3a864f743ae74c13c14bb79f13788590875260027289&traceQuery=`
- Trace hierarchy:
  `http.server.request POST /api/files/publish-artifact` -> API handler
  `POST /api/files/publish-artifact` -> `state.publish_file_artifact` with
  child spans `state.read_file_stream_indexed`, `state.put_public_blob`, and
  `postgres.upsert_published_artifact`.
- Root HTTP span: status OK, HTTP 200, duration 485.615488 ms,
  `service:temperpaw`, `env:prod`, `service.version:sha-aa5c69b`.
- `state.publish_file_artifact` duration was 485.396224 ms and includes
  `tenant:default`, `file_id:bootstrap-soul-file-paw`,
  `artifact_label:codex-live-publish-aa5c69b-postgres-metadata-writable`,
  `owner_ref_type:CodexProof`, and
  `owner_ref_id:4a719ff4-3ad4-4c29-bd15-93c20b24ef37`.
- `state.read_file_stream_indexed` duration was 194.430320 ms and contained an
  `entity_catalog` Postgres `SELECT` child.
- `state.put_public_blob` duration was 268.298208 ms and includes
  `tenant:default`, `bucket:openpaw-fs-seshendranalla`, the full `storage_key`,
  `endpoint_host:075a5c0a617de3bdc08a44f9794b6f2f.r2.cloudflarestorage.com`,
  `mime_type:text/markdown`, `byte_length:18568`, and
  `http.status_code:"200"`.
- `postgres.upsert_published_artifact` duration was 22.528208 ms and contained
  a `published_artifacts` Postgres `INSERT ... ON CONFLICT DO UPDATE` child
  span (11.477330 ms) plus a `published_artifacts` `SELECT` load child span
  (10.884542 ms).
- The parent `state.publish_file_artifact` span emitted
  `published artifact metadata persisted` with `metadata_backend:"postgres"`
  for artifact id `part-199c6b292946f98651137e0ef65331c6`.

Datadog logs and DBM:

- Searching `service:temperpaw env:prod` for `public blob PUT succeeded`,
  `published artifact metadata persisted`, or
  `published artifact metadata store unavailable` returned exactly two
  publish-related logs for the successful proof at `2026-05-13T07:16:47Z`:
  `public blob PUT succeeded` and `published artifact metadata persisted`.
- Log analysis for
  `service:temperpaw env:prod version:sha-aa5c69b "published artifact metadata store unavailable"`
  returned count `0`.
- Log status counts in the checked 45-minute `sha-aa5c69b` window were
  `info:9720`, `warn:600`, and `error:2`. The two error logs correspond to the
  intentional failed `katagami-published-assets` bucket test.
- DBM sampling did not capture the specific `published_artifacts` INSERT in the
  short proof window. A broader two-hour DBM query still returned recent
  TemperPaw samples with SQLCommenter `trace.caller.service:temperpaw` and
  `database_instance:temperpaw-postgres`, but the durable metadata proof for
  this route is the APM SQL span tree above.

What this proves:

- The deployed `sha-aa5c69b` runtime is reporting to Datadog with the intended
  `temperpaw` service/version tags.
- The production Postgres-backed `publish-artifact` path now persists metadata
  through `postgres.upsert_published_artifact`; the old
  `published artifact metadata store unavailable` fallback warning is gone for
  the successful route proof.
- Humans and agents can open one Datadog trace and see the chronological file
  read, R2 PUT, Postgres metadata INSERT/SELECT, status, timings, bucket,
  storage key, artifact label, owner reference, and backend used.

Known remaining gaps from this refresh:

- Public artifact serving is still not correct: the app can write only with the
  current credentials to `openpaw-fs-seshendranalla`, while
  `assets.katagami.ai` does not serve that bucket.
- Changing `PUBLISHED_BLOB_BUCKET` to `katagami-published-assets` is not enough;
  that bucket needs matching R2 S3 credentials, or the public domain must be
  attached to the writable bucket after a planned migration.
- External Railway service/domain/storage names still include `openpaw`; each
  remaining instance must be renamed, replaced, or explicitly allowlisted as a
  migration artifact.

## Live public-blob observability verification refresh - 2026-05-13T06:02Z

Temper/TemperPaw revisions:

- TemperPaw branch: `codex/temperpaw-observability-live-image`
- TemperPaw commit: `702d830f488c702bd9def3decde05d2c35601b5c`
  (`ghcr.io/nerdsane/temperpaw:sha-702d830`)
- Temper branch: `codex/temperpaw-llmobs-service-identity-main`
- Temper commit: `6021d918d0f8daa88f0c9687f4e3c435a2568f4d`
- TemperPaw pins all Temper crates to `6021d918d0f8daa88f0c9687f4e3c435a2568f4d`.

Build and deploy:

- Docker workflow run `25780311599` completed successfully for commit
  `702d830f488c702bd9def3decde05d2c35601b5c`.
- Railway deployment `058db7cd-4f94-4a5c-b6fc-931b9ebe4111` reached
  `SUCCESS`; deployment created at `2026-05-13T05:55:30.681Z`.
- Runtime Datadog identity was set to `DD_SERVICE=temperpaw`,
  `DD_ENV=prod`, `DD_VERSION=sha-702d830`,
  `DD_GIT_COMMIT_SHA=702d830f488c702bd9def3decde05d2c35601b5c`, and
  `DD_GIT_REPOSITORY_URL=https://github.com/nerdsane/temperpaw`.
- `GET https://openpaw-production.up.railway.app/readyz` returned HTTP 200 in
  69 ms with `status:"ready"` and Discord `connected:true`.

Publish-artifact route proof:

- Authenticated OData read of
  `GET /tdata/Files('bootstrap-soul-file-paw')` returned `has_content:true`,
  MIME `text/markdown`, size `18568`, version count `90`, and content hash
  `sha256:a7b843737b4e8d4eaab95a060898b7abbaad53b4b618dcbe2c18b14e5a7eeaa9`.
- Authenticated `POST /api/files/publish-artifact` with label
  `codex-live-publish-702d830-public-blob-span` returned HTTP 200 in 448 ms.
- Returned artifact id:
  `part-74c1e8c408dfafdd6a6d3b4f717bc77f`.
- Returned public storage key:
  `codex-live-proof/CodexProof/058db7cd-4f94-4a5c-b6fc-931b9ebe4111/codex-live-publish-702d830-public-blob-span-a7b843737b4e8d4eaab95a060898b7abbaad53b4b618dcbe2c18b14e5a7eeaa9.md`.
- Returned public URL:
  `https://assets.katagami.ai/codex-live-proof/CodexProof/058db7cd-4f94-4a5c-b6fc-931b9ebe4111/codex-live-publish-702d830-public-blob-span-a7b843737b4e8d4eaab95a060898b7abbaad53b4b618dcbe2c18b14e5a7eeaa9.md`.
- Curling the returned public URL returned HTTP 404. The API response and
  Datadog span prove the publish route completed and R2 returned HTTP 200 for
  the PUT, but they do not prove the public URL is readable.
- `wrangler r2 object get` did not find the returned key in either
  `openpaw-fs-seshendranalla` or `katagami-published-assets` during this
  refresh, despite the runtime span recording an R2 PUT HTTP 200. Treat this as
  an unresolved storage verification discrepancy until bucket/domain/credential
  consolidation is complete.

Datadog APM proof:

- Successful trace:
  `b0bfd80bb2f46a61f6cf07d4cbb2c96f`.
- Trace deep link:
  `https://app.datadoghq.com/apm/trace/b0bfd80bb2f46a61f6cf07d4cbb2c96f?graphType=flamegraph&shouldShowLegend=true&spanID=11790653085075478078&timeHint=1778651900073.8010&trace=b0bfd80bb2f46a61f6cf07d4cbb2c96f11790653085075478078&traceQuery=`
- Trace hierarchy:
  `http.server.request POST /api/files/publish-artifact` -> API handler
  `POST /api/files/publish-artifact` -> `state.publish_file_artifact` with
  child spans `state.read_file_stream_indexed` -> `postgresql.query` and
  `state.put_public_blob`.
- Root HTTP span: status OK, HTTP 200, duration 396.591776 ms,
  `service:temperpaw`, `env:prod`, `service.version:sha-702d830`.
- `state.publish_file_artifact` span includes `tenant:default`,
  `file_id:bootstrap-soul-file-paw`,
  `artifact_label:codex-live-publish-702d830-public-blob-span`,
  `owner_ref_type:CodexProof`, and
  `owner_ref_id:058db7cd-4f94-4a5c-b6fc-931b9ebe4111`.
- `state.read_file_stream_indexed` duration was 123.199960 ms and contained a
  Postgres query child.
- `state.put_public_blob` duration was 273.026656 ms and includes
  `tenant:default`, `bucket:openpaw-fs-seshendranalla`, the full `storage_key`,
  `endpoint_host:075a5c0a617de3bdc08a44f9794b6f2f.r2.cloudflarestorage.com`,
  `mime_type:text/markdown`, `byte_length:18568`, and
  `http.status_code:"200"`.
- The `state.put_public_blob` span emitted `public blob PUT succeeded` with
  `http.status_code:"200 OK"`, bucket, storage key, and endpoint host.
- The parent `state.publish_file_artifact` span emitted
  `published artifact metadata store unavailable; returning derived artifact row`
  for the returned artifact id. This is not a healthy steady-state condition:
  production is Postgres-backed, but published-artifact metadata persistence is
  currently implemented only through the Turso store path.

Datadog logs:

- `service:temperpaw version:sha-702d830` logs were present after deploy.
- In the checked 30-minute window Datadog log analysis returned:
  `info:4887`, `warn:300`, and no `error` rows.
- Searching for `public blob PUT succeeded` or
  `published artifact metadata store unavailable` returned exactly the two
  publish-related logs at `2026-05-13T05:58:20Z`.

What this proves:

- The deployed `sha-702d830` runtime is actually reporting to Datadog with the
  intended `temperpaw` service/version tags.
- The published-artifact route now has a useful public blob write boundary span
  under the route trace, making the file read, database query, and R2 PUT timing
  visible in one chronological Datadog trace.
- The route still returns HTTP 200 while metadata persistence is unavailable in
  the production Postgres path. That warning is now observable, but the
  persistence gap remains.

Known remaining gaps from this refresh:

- Implement backend-neutral published-artifact metadata persistence for the
  Postgres production path, or explicitly decide and document a different
  durable storage model in an ADR.
- Consolidate public artifact bucket, public domain, and write credentials so
  the returned `public_url` is readable and no longer depends on
  `openpaw-fs-seshendranalla`.
- External Railway project/service/domain/storage names still include
  `openpaw`; each remaining instance must be renamed, replaced, or explicitly
  allowlisted as a migration artifact.
- No ADR was added for the `6021d918...` instrumentation patch because it only
  added span/log/error context around an existing boundary and did not change
  architecture. The metadata persistence fix will require ADR treatment if it
  changes storage contracts or backend ownership.

## Local publish-artifact regression fix refresh - 2026-05-13T04:15Z

While verifying blob/document observability, Datadog surfaced a fresh
production error on the published-artifact data path:

- Route: `POST /api/files/publish-artifact`
- Live APM trace: `895a791073db9e5dafb3b927caf8a266`
- Service/version at the time: `temperpaw`, `sha-afeca721`
- Error count: two `POST /api/files/publish-artifact` error spans in the
  checked `2026-05-13T02:45:00Z` to `2026-05-13T03:50:00Z` window.
- Expanded span path included
  `state.publish_file_artifact -> state.read_file_stream_indexed`.
- Log lookup by lower-64 trace id `12660666558028751462` returned zero logs,
  so the actionable evidence was the trace tree itself.

Local red/green fix:

- Temper branch: `codex/temperpaw-llmobs-service-identity-main`
- Temper commit: `81760436f3302f50d50c539cf5b78865ee41b362`
- Fix: `read_file_stream_indexed` and `read_file_version_stream_indexed` keep
  the indexed fast path, but fall back to current `File`/`FileVersion` entity
  state when the query projection is missing or points at stale blob content.
- Regression tests added in Temper:
  - missing File query projection with valid File state returns content
  - stale File query projection with newer File state returns current content
- Temper pre-push gates passed: rustfmt, clippy, readability ratchet, full test
  suite, and doctests.

TemperPaw pin refresh:

- `crates/temperpaw/Cargo.toml` and `Cargo.lock` now pin Temper crates to
  `81760436f3302f50d50c539cf5b78865ee41b362`.
- Contract test updated to enforce that exact revision.
- Local verification passed:
  - `cargo fmt --check`
  - `git diff --check`
  - `cargo test --locked -p temperpaw --test datadog_observability_contract -- --nocapture`
    (`20 passed`)

Live proof status after build/deploy: the refresh below publishes the new
TemperPaw image, exercises `POST /api/files/publish-artifact`, and confirms
Datadog shows a non-error
`state.publish_file_artifact -> state.read_file_stream_indexed` trace under the
new deployed `DD_VERSION`. Missing/stale projection behavior remains covered by
the local red/green regression tests because directly deleting or corrupting
live production projections is not a safe verification method.

## Live publish-artifact fallback verification refresh - 2026-05-13T04:58Z

Temper/TemperPaw revisions:

- TemperPaw branch: `codex/temperpaw-observability-live-image`
- TemperPaw commit: `fe86af8f384a542a221379a8f8cce37f96235405`
  (`ghcr.io/nerdsane/temperpaw:sha-fe86af8`)
- Temper branch: `codex/temperpaw-llmobs-service-identity-main`
- Temper commit: `81760436f3302f50d50c539cf5b78865ee41b362`
- TemperPaw pins all Temper crates to `81760436f3302f50d50c539cf5b78865ee41b362`.

Build and deploy:

- Docker workflow run `25778012799` completed successfully for commit
  `fe86af8f384a542a221379a8f8cce37f96235405`.
- Railway deployment `f0110eeb-872e-4b87-a996-b0bd6596e816` reached
  `SUCCESS`; deployment created at `2026-05-13T04:47:43.184Z`.
- Railway build logs pulled
  `ghcr.io/nerdsane/temperpaw:sha-fe86af8@sha256:6f06f85707bd01c8668cd7f499edb79491a37407cbdcfe5b65915d6ceaff8cfd`.
- Railway wrapper image digest:
  `sha256:3550d3c56d0fd603d67cadf14e29d720d2b0a02d145b6cdbba84e4fa15e5b00e`.
- Runtime Datadog identity was set to `DD_SERVICE=temperpaw`,
  `DD_ENV=prod`, `DD_VERSION=sha-fe86af8`,
  `DD_GIT_COMMIT_SHA=fe86af8f384a542a221379a8f8cce37f96235405`, and
  `DD_GIT_REPOSITORY_URL=https://github.com/nerdsane/temperpaw`.
- `GET https://openpaw-production.up.railway.app/readyz` returned HTTP 200 in
  188 ms with `status:"ready"` and Discord `connected:true`.

Publish-artifact route proof:

- Authenticated OData read of `GET /tdata/Files?$top=1` returned
  `File('bootstrap-soul-file-paw')` with `has_content:true`, MIME
  `text/markdown`, size `18568`, and content hash
  `sha256:a7b843737b4e8d4eaab95a060898b7abbaad53b4b618dcbe2c18b14e5a7eeaa9`.
- Authenticated
  `POST /api/files/publish-artifact` with label
  `codex-live-publish-fe86af8-revert-ok` returned HTTP 200 in 405 ms.
- Returned artifact id:
  `part-104533d8f1f49e278c47eb3396c62882`.
- Returned public storage key:
  `codex-live-proof/CodexProof/f0110eeb-872e-4b87-a996-b0bd6596e816/codex-live-publish-fe86af8-revert-ok-a7b843737b4e8d4eaab95a060898b7abbaad53b4b618dcbe2c18b14e5a7eeaa9.md`.
- Datadog retained successful trace:
  `cfebe35b269cb4970f2ced34949f13bb`.
- Trace deep link:
  `https://app.datadoghq.com/apm/trace/cfebe35b269cb4970f2ced34949f13bb?graphType=flamegraph&shouldShowLegend=true&spanID=9340654343223372582&timeHint=1778648162291.6514&trace=cfebe35b269cb4970f2ced34949f13bb9340654343223372582&traceQuery=`
- Trace hierarchy:
  `http.server.request POST /api/files/publish-artifact` -> API handler
  `POST /api/files/publish-artifact` -> `state.publish_file_artifact` ->
  `state.read_file_stream_indexed` -> `postgresql.query`.
- Root HTTP span: status OK, HTTP 200, duration 354.120864 ms,
  `service:temperpaw`, `env:prod`, `version:sha-fe86af8`,
  `service.version:sha-fe86af8`.
- `state.publish_file_artifact` span includes `tenant:default`,
  `file_id:bootstrap-soul-file-paw`,
  `artifact_label:codex-live-publish-fe86af8-revert-ok`,
  `owner_ref_type:CodexProof`, and
  `owner_ref_id:f0110eeb-872e-4b87-a996-b0bd6596e816`.
- `state.read_file_stream_indexed` was a child of `state.publish_file_artifact`
  and contained the Postgres query child for `entity_catalog` plus
  `entity_field_index`.

What this proves:

- The deployed `sha-fe86af8` runtime is actually reporting to Datadog with the
  intended `temperpaw` service/version tags.
- The published-artifact route no longer reproduces the earlier
  `state.read_file_stream_indexed` 500 on the exercised production File.
- The missing/stale projection behavior is covered by the local red/green
  regression tests in Temper. Production projection mutation was not performed
  because directly deleting or corrupting live query projections would be an
  unsafe verification technique.

Datadog logs:

- `service:temperpaw version:sha-fe86af8` logs were present immediately after
  deploy.
- In the checked 20-minute window Datadog log analysis returned:
  `info:10025`, `warn:318`, `error:1`.
- The single error was the intentional public-bucket permission experiment at
  `2026-05-13T04:55:16Z`.
- After reverting that experiment, the query
  `service:temperpaw version:sha-fe86af8 status:error` from
  `2026-05-13T04:56:03Z` to `now` returned `0`.
- Searching logs by successful trace id
  `cfebe35b269cb4970f2ced34949f13bb` returned no log rows. The HTTP response
  data is visible as APM span events, but response logs for this route are not
  currently searchable by trace id. Guest WASM logs remain trace-correlated per
  the earlier session proof.

Public artifact serving and identity gap:

- The route writes through `PUBLISHED_BLOB_BUCKET`. Live production currently
  has `PUBLISHED_BLOB_BUCKET=openpaw-fs-seshendranalla` and
  `PUBLISHED_BLOB_PUBLIC_BASE_URL=https://assets.katagami.ai`.
- `assets.katagami.ai` is attached to R2 bucket `katagami-published-assets`;
  `openpaw-fs-seshendranalla` has no custom domain.
- The route returned a public URL under `https://assets.katagami.ai/...`, but
  that URL returned HTTP 404 because the bytes were written to the old
  `openpaw-fs-seshendranalla` bucket.
- Wrangler verified the object exists in `openpaw-fs-seshendranalla` with
  18,568 bytes.
- A direct runtime secret experiment changed `published_blob_bucket` to
  `katagami-published-assets`; the next publish attempt failed with HTTP 500
  because the configured S3 credentials received `403 Forbidden` for that
  bucket. Datadog trace: `f9a9cc3e48ea28fa25c8a9e32a55d8d5`.
- The experiment was reverted immediately by restoring both the live secret and
  Railway env var to `openpaw-fs-seshendranalla`, after which publish returned
  HTTP 200 again.
- This remains a concrete OpenPAW residual and data-service gap: production
  public artifact publishing can write bytes and return a derived artifact row,
  but the public URL is not readable until the bucket, public domain, and R2
  write credentials are consolidated onto the same non-OpenPAW public artifact
  bucket. The later `sha-702d830` refresh also proved the Postgres production
  path is not durably persisting published-artifact metadata yet.

## Live production verification refresh - 2026-05-13T03:45Z

Temper/TemperPaw revisions:

- TemperPaw branch: `codex/temperpaw-observability-live-image`
- TemperPaw commit: `afeca72116deaa4da019c277bc7b2ca90a49d4c5`
  (`ghcr.io/nerdsane/temperpaw:sha-afeca72`)
- Temper branch: `codex/temperpaw-llmobs-service-identity-main`
- Temper commit: `974b13bf02342a1b8faafdb1b762572933fe1c3e`
- Temper commit contents include the earlier direct LLMObs hierarchy, Postgres
  DBM/APM attribution, pprof upload envelopes, WASM span hints, guest-log
  trace/span correlation, and ADR-0084 long-lived workflow root spans.

Build and deploy:

- Docker workflow run `25774969998` completed successfully.
- Published image: `ghcr.io/nerdsane/temperpaw:sha-afeca72`
- GHCR image digest:
  `sha256:5b111eaa0e7c1bed0aa306609e299496dfdf02ca2c3a61496d9305188bd404e1`
- Railway variables were corrected to `DD_SERVICE=temperpaw`,
  `DD_ENV=prod`, `DD_VERSION=sha-afeca721`,
  `DD_GIT_COMMIT_SHA=afeca72116deaa4da019c277bc7b2ca90a49d4c5`, and
  `DD_GIT_REPOSITORY_URL=https://github.com/nerdsane/temperpaw`.
- Railway deployment `20079bb1-1e83-4ed2-84dd-1689df3d2907` reached
  `SUCCESS` at `2026-05-13T03:17:53Z`.
- Railway wrapper image digest:
  `sha256:2f40c46b2d9c907097161f49a9b6aeb05362eec9fd6f42145f45b09ecd4e19e9`
- `GET https://openpaw-production.up.railway.app/readyz` returned HTTP 200 with
  `status:"ready"` and Discord `connected:true`.
- Remaining deployment identity gap: the Railway service/public domain and some
  external storage/database resource names still use `openpaw` because Railway
  does not expose a safe rename path through the CLI used in this proof.

Primary live proof session:

- Production Session: `ss-019e1f59-41b4-7993-870f-9bf9ac7e4a18`
- Result: `TemperPaw workflow root trace verified.`
- Session started at `2026-05-13T03:20:03Z` and completed at
  `2026-05-13T03:20:18Z`.
- APM trace id: `00795a1c90435bf41a99f0a051f9d729`
- LLMObs trace id: `630095599782866875251990789384427305`
- Datadog lower-64 log trace id: `1916827687783618345`
- Session entity fields captured `gen_ai_parent_trace_id`,
  `gen_ai_parent_span_id`, `llmobs_agent_span_id`, and
  `llmobs_workflow_span_id`.
- Wait/event payload showed the expected chronological actions:
  `Configure`, `ProvisionWorkspace`, `WorkspaceReady`, `ContextReady`,
  `ProviderAuthReady`, `Heartbeat`, `ProgressMade`, `ProviderResponseReady`,
  `CheckSteering`, and `FinalizeResult`.

APM:

- `get_datadog_trace(00795a1c90435bf41a99f0a051f9d729,
  only_service_entry_spans=true)` returned root resource `Session.workflow`,
  service `temperpaw`, root span id `9416528624571850451`, and duration
  `15738.067968 ms`.
- The root span is not a short OData HTTP request. It has `parent: None`,
  `entity_id:ss-019e1f59-41b4-7993-870f-9bf9ac7e4a18`,
  `entity_type:Session`, `workflow.root_entity_type:Session`,
  `workflow.root_entity_id:ss-019e1f59-41b4-7993-870f-9bf9ac7e4a18`,
  `workflow.run_id:Session:ss-019e1f59-41b4-7993-870f-9bf9ac7e4a18`,
  `service.version:sha-afeca721`, and `version:sha-afeca721`.
- The summarized root reported 477 hidden child spans. A raw span search for the
  trace returned 512 spans.
- Aggregation by resource/operation returned 55 buckets, including Temper
  dispatch/action spans, Postgres spans for `entity_catalog`,
  `entity_field_index`, `events`, `snapshots`, and `wasm_invocation_logs`, and
  WASM integration spans for `workspace_provisioner`, `context_preparer`,
  `provider_auth_gate`, `provider_response_applier`, `steering_checker`,
  `agent_reply`, and `emit_ots_trajectory`.
- Expanded WASM spans include `module_name`, `trigger_action`,
  `service.version:sha-afeca721`, `wasm.module`, and guest-log span events.

LLM Observability:

- `search_llmobs_spans(ml_app=temperpaw, tags={session_id})` returned three
  spans for the same trace. The decimal LLMObs trace id
  `630095599782866875251990789384427305` maps to APM hex trace id
  `00795a1c90435bf41a99f0a051f9d729`.
- `get_llmobs_trace(630095599782866875251990789384427305)` returned:
  - agent root `temperpaw.agent_session`, span `5327690730953755574`,
    duration 2838 ms, status ok.
  - workflow child `Session.ProviderAuthReady`, span
    `17448521234714304717`, duration 2738 ms.
  - LLM child `wasm:provider_caller`, span `13098621566835330088`,
    duration 2638 ms, provider `openai`, model `gpt-5.5`.
- LLM token metadata reported 209 input tokens, 23 output tokens, total 232,
  and the output preview matched the proof result.
- Known gap: `get_llmobs_agent_loop` still returned `iterations:[]`,
  `timeline:null`, and `total_messages:0`; use the LLMObs tree, APM trace, and
  OData/OTS chronology instead.

WASM guest logs:

- Log query
  `service:temperpaw env:prod version:sha-afeca721 @guest_log.target:wasm_guest @session_id:ss-019e1f59-41b4-7993-870f-9bf9ac7e4a18`
  returned 26 structured WASM guest logs.
- Querying by the Datadog lower-64 log trace id
  `trace_id:1916827687783618345 @guest_log.target:wasm_guest` returned the
  same log set.
- Representative messages include `workspace_provisioner: starting`,
  `context_preparer: starting`, `provider_auth_gate: dispatching
  OpenAICodexAuth.EnsureFresh`, `provider_caller: starting`,
  `session_turn: calling OpenAI API`, `session_turn: OpenAI Codex response`,
  `session_phase phase=provider_caller step=provider_http result=ok`,
  `provider_response_applier: starting`, `steering_checker: starting`,
  `agent_reply: no channel session linked`, and `emit_ots_trajectory: emitted
  trajectory`.
- Each returned log includes top-level `trace_id`, `span_id`, service, env, and
  version. APM span events additionally include `otel.trace_id`,
  `otel.span_id`, `dd.trace_id`, and `dd.span_id` attributes.

Postgres DBM and APM correlation:

- The proof session trace itself contains Postgres APM spans, but DBM samples
  are sparse. To prove DBM with the current version, a read-only OData burst of
  80 `GET /tdata/Sessions?$top=3` requests ran from `2026-05-13T03:22:25Z` to
  `2026-05-13T03:22:54Z`.
- DBM sample query
  `database_instance:temperpaw-postgres service:temperpaw` returned a fresh
  `2026-05-13T03:22:26Z` sample for table `entity_catalog`, command `SELECT`,
  service `temperpaw`, team `temperpaw`, and query signature
  `12941344394c8422`.
- The SQLCommenter comment included
  `dddbs='temperpaw-postgres',ddps='temperpaw',dde='prod',ddpv='sha-afeca721'`
  and traceparent
  `00-e5139e30de2db2af1cb696ab7a25d899-d3aad2d3beb6b61e-01`.
- `get_datadog_trace(e5139e30de2db2af1cb696ab7a25d899)` returned root
  `GET /tdata/Sessions`, service `temperpaw`, version `sha-afeca721`, duration
  1612 ms, with a child Postgres span for the same `entity_catalog` statement
  and `peer.service:temperpaw-postgres`.

Profiling:

- Runtime variables showed `TEMPER_PROFILING_ENABLED=true`,
  `TEMPER_PROFILING_AUTO_UPLOAD=true`, `DD_AGENT_HOST` pointing at the Datadog
  Agent, and on-demand pprof upload enabled.
- `GET /_admin/profile/cpu?seconds=5&frequency=100` with auth returned HTTP
  200, content type `application/vnd.google.protobuf`, content disposition
  `cpu-profile-5s.pb`, and 40,450 bytes while read traffic was active.
- Profile window: `2026-05-13T03:24:31Z` to `2026-05-13T03:24:36Z`.
- Logs for `service:temperpaw` showed `ADR-0055: starting CPU profile capture`,
  `ADR-0055: CPU profile capture complete`, and `profile uploaded to Datadog
  Agent intake`, all with version `sha-afeca721`.
- Metric query
  `sum:datadog.profiling.rust.profiles_uploaded{service:temperpaw,env:prod} by {version,profile_type}.as_count()`
  returned `profile_type:cpu,version:sha-afeca721` with one upload point.
- Matching upload-error metric query returned no data in the checked window.

Datadog assets and legacy identity checks:

- Dashboard search returned dashboard `mn4-k3k-i66`,
  `TemperPaw — Platform Overview`, with queries scoped to `service:temperpaw`
  and profiling coverage included.
- `monitor_groups_search(group_status:alert (TemperPaw OR Temper))` returned
  zero active alert groups.
- Key monitors were `OK`: `[TemperPaw] Postgres DBM Activity Missing`,
  `[Temper] Profiler Upload Failures`, `[TemperPaw] Agent Session Trace
  Correlation Missing`, and `[TemperPaw] LLM Error Rate Spike`. Low-traffic
  monitors that were `No Data` did not have active alert groups.
- Fresh-window legacy checks returned zero:
  - logs query
    `service:temperpaw env:prod version:sha-afeca721 (OpenPAW OR OpenPaw OR "Open Paw" OR openpaw)`
  - logs query `(service:openpaw OR OpenPAW OR OpenPaw OR "Open Paw")`
  - APM spans query `service:openpaw OR OpenPAW OR OpenPaw`

Streaming/blob route probe:

- A separate authenticated production streaming probe posted a tiny raw body to
  `POST /tdata/Blobs/Temper.IngestRaw` with `X-Repository-Id:
  rp-temperpaw-datadog-proof`.
- The endpoint returned HTTP 201 and created
  `Blobs('c251953bbf2daa464647db9ffe6b7d9a80b07c5d')`.
- Datadog APM retained trace `993e74a7129a8c286ce53d8c5b1e9f8a` with root
  resource `POST /tdata/Blobs/Temper.IngestRaw`, HTTP 201, duration
  130.405824 ms, `service.version:sha-afeca721`, and `version:sha-afeca721`.
- Expanding the trace showed five children, including
  `constraint.pre_upsert_relation_checks`,
  `constraint.pre_upsert_field_invariant_checks`,
  `constraint.post_write_invariant_checks`, and
  `entity.get_or_create_tenant_entity -> entity.get_or_spawn_tenant_actor_with_fields`.
- This is accepted as Datadog-retained streaming/blob endpoint proof. The
  agent-session proof also covers streaming provider HTTP under
  `wasm:provider_caller`.

## Live production verification refresh — 2026-05-12T23:20Z

Temper/TemperPaw revisions:

- TemperPaw branch: `codex/temperpaw-observability-live-image`
- TemperPaw commit: `bd419f15` (`ghcr.io/nerdsane/temperpaw:sha-bd419f1`)
- Temper branch: `codex/temperpaw-llmobs-service-identity-main`
- Temper commit: `314a246d32a91036a0a6e542dfdd66532d7aec7a`
- Temper commit contents include direct LLMObs agent/workflow/LLM hierarchy,
  Postgres DBM/APM attribution, Datadog-compatible profile upload envelopes,
  Datadog-visible WASM host span hints, and guest-log trace/span correlation.

Build and deploy:

- Docker workflow run `25765531534` completed successfully.
- Published image: `ghcr.io/nerdsane/temperpaw:sha-bd419f1`
- GHCR image digest:
  `sha256:33ba65eaa3b319c0befa9431eb4575f6c5a8049608fb2fc338dcbc1111aedc27`
- Railway deployment `31f39d27-68c9-4cf7-a7b5-b30572fa4d06` reached `SUCCESS`.
- Railway build logs show the deployed wrapper pulled
  `ghcr.io/nerdsane/temperpaw:sha-bd419f1@sha256:33ba65eaa3b319c0befa9431eb4575f6c5a8049608fb2fc338dcbc1111aedc27`.
- Railway wrapper image digest:
  `sha256:5e3c5525ad129c69cb3783218b655723137faecb39d5bfb5065ff6865ee75e30`
- `GET https://openpaw-production.up.railway.app/readyz` returned HTTP 200 with
  `status:"ready"` and Discord `connected:true`.
- Runtime identity: `DD_SERVICE=temperpaw`, `DD_ENV=prod`,
  `DD_VERSION=sha-bd419f1`, and `DD_DBM_DATABASE_SERVICE=temperpaw-postgres`.
- Remaining deployment identity gap: the Railway service/public domain and some
  external storage/database resource names still use `openpaw` because Railway
  does not expose a safe rename path through the CLI used in this proof.

Primary live proof session:

- Production Session: `ss-019e1e65-c1ff-7ac3-bbf7-0feb6220fc7c`
- Result: `TemperPaw DBM full trace verified.`
- APM trace id: `c80f416a8f1a6c61c86d873747ca26e3`
- LLMObs trace id: `265924810408958961160905709497239611107`
- Datadog lower-64 log trace id: `14442348251544430307`
- A second profile-overlapped proof session also completed:
  `ss-019e1e6a-04c2-7e50-8e11-42cf5c3163cb`, APM trace
  `90b2891aa7a3f47957602667c14ad299`, LLMObs trace
  `192335841035777216710283241883473466009`.

LLM Observability:

- `get_llmobs_trace(265924810408958961160905709497239611107)` returned
  `total_spans:3`, `tree_depth:3`, `error_count:0`, services `["temperpaw"]`,
  and span kinds `{agent:1, workflow:1, llm:1}`.
- LLMObs tree:
  - agent root `temperpaw.agent_session`, span `5097270632223975381`,
    duration 4569 ms.
  - workflow child `Session.ProviderAuthReady`, span
    `13129598092557942707`, duration 4469 ms.
  - LLM child `wasm:provider_caller`, span `11691726881029401664`,
    duration 4369 ms, provider `openai`, model `gpt-5.5`, input tokens `208`,
    output tokens `24`.
- Known gap: `get_llmobs_agent_loop` still returns `iterations:[]`,
  `timeline:null`, and `total_messages:0` for the direct Session trace even
  though the LLMObs tree is correct. Operators should use `get_llmobs_trace`,
  APM, and OData/OTS event history for chronology.

APM:

- `get_datadog_trace(c80f416a8f1a6c61c86d873747ca26e3)` showed root
  `POST /tdata/Sessions('ss-{guid}')/TemperPaw.Configure`, HTTP 200, duration
  76.5 ms, `service:temperpaw`, `env:prod`, `version:sha-bd419f1`,
  `service.version:sha-bd419f1`, and 484 hidden child spans.
- Expanded trace shows the chronological flow:
  `Session.Configure -> ProvisionWorkspace -> workspace_provisioner ->
  WorkspaceReady -> context_preparer -> ContextReady -> ProviderAuthReady ->
  provider_auth_gate -> provider_caller -> ProviderResponseReady ->
  provider_response_applier -> CheckSteering -> steering_checker ->
  FinalizeResult -> agent_reply -> emit_ots_trajectory`.
- The trace includes Temper state transition events, WASM guest-log events,
  module names, trigger actions, entity ids, action names, and Postgres client
  spans under the same trace.
- `search_datadog_spans` for the trace returned 587 spans. `aggregate_spans`
  showed high-volume but explainable DB fanout: 330
  `INSERT INTO entity_field_index...` spans in the primary proof trace, plus
  `entity_catalog`, `events`, `snapshots`, and `wasm_invocation_logs` spans.

WASM guest logs:

- Log query:
  `service:temperpaw env:prod version:sha-bd419f1 @guest_log.target:wasm_guest @session_id:ss-019e1e65-c1ff-7ac3-bbf7-0feb6220fc7c`
  returned 27 structured WASM guest log events.
- The events include top-level `trace_id`/`span_id`, plus
  `otel.trace_id`, `otel.span_id`, `session_id`, `gen_ai.conversation.id`,
  `entity_id`, `entity_type`, `trigger_action`, `guest_log.message`,
  `guest_log.severity`, `guest_log.target`, and
  `service.version:sha-bd419f1`.
- Representative messages include `workspace_provisioner: starting`,
  `context_preparer: starting`, `provider_caller: starting`,
  `session_turn: calling OpenAI API`, `session_turn: OpenAI Codex response`,
  `provider_response_applier`, `steering_checker`, `agent_reply`, and
  `emit_ots_trajectory`.
- Querying by `trace_id:14442348251544430307 @guest_log.target:wasm_guest`
  returns the same log set. Datadog normalizes raw `dd.trace_id`/`dd.span_id`
  into top-level `trace_id`/`span_id` at ingest; the raw `dd.*` fields are not
  retained as separate searchable attributes in the current pipeline.

Postgres DBM and APM correlation:

- `search_datadog_database_samples(query="service:temperpaw @trace.caller.version:sha-bd419f1")`
  returned three recent DBM samples for `database_instance:temperpaw-postgres`.
- A full-mode sample at `2026-05-12T23:01:33Z` for `entity_catalog` included
  SQLCommenter:
  `dddbs='temperpaw-postgres',ddps='temperpaw',dde='prod',ddpv='sha-bd419f1',traceparent='00-e817e047f0aedd5edd711707c490cd72-ca0c464413736a05-01'`.
- That sample included `trace.mode:"full"`,
  `trace.caller.service:"temperpaw"`, `trace.caller.env:"prod"`,
  `trace.caller.version:"sha-bd419f1"`,
  `trace.caller.resource:"GET /tdata/Sessions"`, and `trace.sampled:true`.
- `get_datadog_trace(e817e047f0aedd5edd711707c490cd72)` showed root
  `GET /tdata/Sessions`, HTTP 200, with a child Postgres span for the same
  `entity_catalog` statement and `peer.service:temperpaw-postgres`.
- `search_datadog_spans` for
  `service:temperpaw type:sql @db.system:postgresql @peer.service:temperpaw-postgres`
  returned 5,406 spans in the checked 30-minute window. `aggregate_spans` for
  the same query returned live buckets for `entity_catalog`, `entity_field_index`,
  `snapshots`, `events`, `trajectories`, and `wasm_invocation_logs`.
- `datadog.dbm.activity_rows{service:temperpaw,database_instance:temperpaw-postgres}`
  reported three DBM activity rows in the same 30-minute window.

Profiling:

- On-demand CPU profile request during live session traffic returned HTTP 200,
  content type `application/vnd.google.protobuf`, and 86,346 bytes.
- Log query
  `service:temperpaw env:prod version:sha-bd419f1 "profile uploaded to Datadog Agent intake"`
  returned two upload logs at `2026-05-12T22:58:01Z` and
  `2026-05-12T22:58:54Z`, both uploading to
  `http://datadog-postgres-agent.railway.internal:8126/profiling/v1/input`.
- Metric query
  `sum:datadog.profiling.rust.profiles_uploaded{service:temperpaw,env:prod,version:sha-bd419f1}.as_count()`
  returned values `1,0,1,0,0,0,0` over the one-hour checked window starting
  `2026-05-12T22:58:00Z`.
- Matching upload-error metric query returned no data in the checked window.

Datadog assets:

- Dashboard search returned dashboard `mn4-k3k-i66`,
  `TemperPaw — Platform Overview`, description `Single pane of glass for
  TemperPaw agent orchestration health, runtime behavior, and Temper platform
  metrics`, with queries scoped to `service:temperpaw`.
- Monitor reconciliation updated all desired TemperPaw/Temper monitors and
  deleted no additional orphans.
- `[TemperPaw] Agent Session Trace Correlation Missing` now uses an event-gated
  log alert:
  `service:temperpaw @observability_event:temperpaw.agent.session -trace_id:*`.
  This avoids false alerts when there is no managed-session traffic.
- The false-alerting trace-analytics absence monitor
  `[TemperPaw] Postgres DBM Missing APM Correlation` was replaced because
  Datadog Trace Explorer and span aggregation returned thousands of matching
  child SQL spans while monitor evaluation still reported zero.
- Replacement monitor `[TemperPaw] Postgres DBM Activity Missing`
  (`282522099`) is a metric alert on
  `datadog.dbm.activity_rows{service:temperpaw,database_instance:temperpaw-postgres}`;
  Datadog reports it `OK`.
- The DBM monitor runbook keeps the human/agent correlation query:
  `service:temperpaw type:sql @db.system:postgresql @peer.service:temperpaw-postgres`,
  plus DBM sample filter `service:temperpaw @trace.caller.service:temperpaw`.
- `[TemperPaw] Postgres DBM Query Latency Regression` was corrected from
  `> 1` to `> 30000000` because Datadog reports `postgresql.queries.time` in
  nanoseconds. Live metric data showed normal queries in the hundreds of
  microseconds to low milliseconds.
- `[Temper] Profiler Uploads Stalled` was deleted because Railway profiling is
  on-demand. `[Temper] Profiler Upload Failures` remains as the paging monitor,
  and proof uploads are documented through logs/metrics.
- `[Temper] State Timeout Reset Rate Drop` was changed to resolve no-data and
  recreated as monitor `282523788`, because idle traffic should not page as a
  reset-rate regression.
- `monitor_groups_search(status:alert)` returned zero active alert groups after
  the monitor corrections and reconciliation.

Red/green and command verification in this refresh:

- Red/green monitor contract:
  - First required `@db.system:postgresql` and observed the test fail against
    the old `operation_name:postgresql.query` monitor query.
  - Patched the monitor to use live indexed DB span attributes and the test
    passed.
  - Then required `type:sql`, observed the test fail again, patched the query,
    and the test passed.
- `cargo test --locked -p temperpaw --test datadog_observability_contract monitors_cover_session_trace_llmobs_and_postgres_dbm_health -- --nocapture` passed.
- Earlier verification for this deployed branch passed:
  `cargo fmt --check`,
  `cargo test --locked -p temperpaw --test datadog_observability_contract -- --nocapture`,
  `cargo test -p temperpaw dashboard -- --nocapture`, and
  `cargo clippy --locked -p temperpaw -p paw-codex-worker --all-targets -- -D warnings`.

## Live production verification refresh — 2026-05-12T19:10Z

Temper/TemperPaw revisions:

- TemperPaw branch: `codex/temperpaw-observability-live-image`
- TemperPaw commit: `30feec4fadd171331ea410b83f0cc54a310ec6e4`
- Temper branch: `codex/temperpaw-llmobs-service-identity-main`
- Temper commit: `d3d3814a4d076212dbfb378a6c606124afa4b9dd`
- Temper commit contents include direct LLMObs agent/workflow/LLM hierarchy,
  SQLCommenter DBM attribution, full-mode `traceparent` propagation, and
  Postgres client spans.

Build and deploy:

- Docker workflow run `25753233353` completed successfully.
- Published image: `ghcr.io/nerdsane/temperpaw:sha-30feec4`
- GHCR image digest:
  `sha256:b37575351b600f05df30030b826f4df53a2983856d131ca97632ccf53e7a4885`
- Railway wrapper deploy after Datadog variables:
  `06c812de-cca5-4424-9149-0c3bfaf27ebd`
- Railway deploy reached `SUCCESS`.
- Railway wrapper image digest:
  `sha256:c202fb97d1403f6c132f54dcd90536dc0293d9a774edde04866175335c450855`
- `GET https://openpaw-production.up.railway.app/readyz` returned HTTP 200 with
  `status:"ready"` and Discord `connected:true`.
- Runtime variable checks:
  - `DD_SERVICE=temperpaw`
  - `DD_ENV=prod`
  - `DD_VERSION=30feec4fadd171331ea410b83f0cc54a310ec6e4`
  - `DD_DBM_PROPAGATION_MODE=full`
  - `DD_DBM_DATABASE_SERVICE=temperpaw-postgres`
  - `DD_API_KEY` present, checked without printing the secret
- Note: setting Railway variables created config deploys that briefly used the
  older configured `edge` image. The wrapper image was redeployed afterwards;
  deployment `06c812de-cca5-4424-9149-0c3bfaf27ebd` is the final verified
  production deployment in this refresh.

Live proof session:

- Created production Session `ss-019e1d97-313d-7063-8cd9-710d454ea497` through
  OData `TemperPaw.Configure` with `provider:openai_codex`, `model:gpt-5.5`,
  `max_turns:1`, `tools_enabled:""`, and prompt:
  `Datadog full DBM/APM/LLMObs proof. Reply exactly: TemperPaw DBM full trace verified.`
- Session result was exactly `TemperPaw DBM full trace verified.`
- LLMObs trace id:
  `134057669320178038853739325168218517190`
- Same APM trace in hex:
  `64da9165f2a01864fa923a6dc71faec6`

LLM Observability:

- `get_llmobs_trace` for trace
  `134057669320178038853739325168218517190` returned
  `error_count:0`, `has_errors:false`, service list `["temperpaw"]`,
  `span_kinds:{agent:1, workflow:1, llm:1}`, and tree depth `3`.
- LLMObs tree:
  - agent root `temperpaw.agent_session`, span `17970935652202304378`,
    duration 3068 ms.
  - workflow child `Session.ProviderAuthReady`, span
    `6487776932174714096`, parent `17970935652202304378`, duration 2968 ms.
  - LLM child `wasm:provider_caller`, span `8743697503059954175`, parent
    `6487776932174714096`, duration 2868 ms.
- The direct LLMObs tree is coherent, chronological, non-duplicative, and uses
  `service:temperpaw`.
- `get_llmobs_agent_loop` for the same trace/root returned
  `iterations:[]`, `timeline:null`, and `total_messages:0`. Operators should
  currently use `get_llmobs_trace`, APM, and OData event history instead of
  relying on the helper timeline for direct Session traces.

APM:

- `search_datadog_spans` for
  `service:temperpaw @entity_id:ss-019e1d97-313d-7063-8cd9-710d454ea497`
  returned 85 spans in the checked window.
- `get_datadog_trace` for `64da9165f2a01864fa923a6dc71faec6` showed root
  `POST /tdata/Sessions('ss-{guid}')/TemperPaw.Configure`, HTTP 200,
  duration 55.847 ms, and 484 hidden child spans.
- Expanded trace showed chronological state/WASM/DB work:
  `Session.Configure`, `ProvisionWorkspace`, `workspace_provisioner`,
  `WorkspaceReady`, `context_preparer`, `ContextReady`,
  `ProviderAuthReady`, `provider_auth_gate`, `provider_caller`,
  `ProviderResponseReady`, `provider_response_applier`, `CheckSteering`,
  `steering_checker`, `FinalizeResult`, `agent_reply`, and
  `emit_ots_trajectory`.
- `aggregate_spans` for the trace grouped by operation/resource returned
  57 buckets, including:
  - `postgresql.query` `INSERT INTO entity_field_index...` count 330
  - `postgresql.query` `DELETE FROM entity_field_index...` count 14
  - `postgresql.query` `INSERT INTO entity_catalog...` count 14
  - `postgresql.query` `INSERT INTO wasm_invocation_logs...` count 8
  - action/WASM resources for the session flow listed above
- `aggregate_spans` for
  `trace_id:64da9165f2a01864fa923a6dc71faec6 operation_name:postgresql.query`
  showed DB spans with `@db.collection.name`, `@db.operation`, and
  `@peer.service:temperpaw-postgres`.

Postgres DBM:

- `search_datadog_database_samples(query="database_instance:temperpaw-postgres")`
  over the live window returned three samples.
- A full-mode sample for `entity_field_index` included SQLCommenter fields:
  `dddbs='temperpaw-postgres'`, `ddps='temperpaw'`, `dde='prod'`,
  `ddpv='30feec4fadd171331ea410b83f0cc54a310ec6e4'`, and
  `traceparent='00-265496fc52f3f7c35e569c2e07107d25-bb5ba5aa7e1a3aa7-01'`.
- The same sample included DBM trace metadata:
  - `trace.mode:"full"`
  - `trace.caller.env:"prod"`
  - `trace.caller.service:"temperpaw"`
  - `trace.caller.version:"30feec4fadd171331ea410b83f0cc54a310ec6e4"`
  - `trace.caller.operation:"http.server.request"`
  - `trace.caller.resource:"PUT /tdata/Files('bootstrap-soul-file-swe')/$value"`
  - `trace.root.resource:"PUT /tdata/Files('bootstrap-soul-file-swe')/$value"`
  - `trace.sampled:true`
  - `trace.span.service:"temperpaw"`
- `get_datadog_database_calling_services(database_instance="temperpaw-postgres")`
  returned `calling_service:"temperpaw"` and calling resources:
  - `GET /tdata/WorkCycles('wc-{guid}')`
  - `GET /tdata/WorkerRuns`
  - `PUT /tdata/Files('bootstrap-soul-file-swe')/$value`
- APM trace `265496fc52f3f7c35e569c2e07107d25` from the DBM `traceparent`
  contained DB spans and an HTTP root. `aggregate_spans` showed 40
  `postgresql.query INSERT INTO entity_field_index`, 4 `DELETE FROM
  entity_field_index`, 4 `INSERT INTO entity_catalog`, and the PUT file root.

Logs:

- `search_datadog_logs` for `service:temperpaw` LLMObs/failure terms over the
  checked 30-minute window returned count 0.
- `search_datadog_logs` for `service:temperpaw "failed to submit llm span"`
  over the checked one-hour window returned count 0.
- This proves no live LLMObs submission failure logs were observed during the
  proof window, not that all application logs have been exhaustively classified.

Datadog asset reconciliation:

- Dashboard source `dd-dashboards/temperpaw-overview.json` was updated to remove
  invalid nested note-widget `title` fields. Datadog rejected those fields
  during a PUT; after the fix, `scripts/deploy_dashboard.py --reconcile`
  succeeded and updated dashboard `mn4-k3k-i66`.
- `get_datadog_dashboard("mn4-k3k-i66")` returned title
  `TemperPaw — Platform Overview`, description scoped to TemperPaw, tag
  `team:temperpaw`, and dashboard queries using `service:temperpaw`.
- Local JSON validation passed for:
  - `dd-dashboards/temperpaw-overview.json`
  - `dd-monitors/temperpaw-monitors.json`
  - `dd-pipelines/temper-temperpaw.json`
  - `dd-pipelines/facets.json`
  - `dd-pipelines/sensitive-data-scanner.json`
  - `dd-log-metrics/temper-log-metrics.json`
- `scripts/deploy_monitors.py --reconcile` initially failed on
  `[TemperPaw] Error Rate Spike` because Datadog rejects
  `on_missing_data:"resolve"` when the query uses `default_zero`. The monitor
  definition was changed to `on_missing_data:"default"` and
  `validate_monitor_definition` returned `is_valid:true`.
- Monitor reconciliation then completed. It created new TemperPaw monitors:
  `[TemperPaw] Error Rate Spike` `282445906`,
  `[TemperPaw] Request Latency Spike (P95)` `282445910`,
  `[TemperPaw] No Traffic` `282445911`,
  `[Temper] Blob Transport Wait Spike` `282445961`,
  `[TemperPaw] TemperFS Metadata Operation Errors` `282445965`,
  `[TemperPaw] Webhook Receive Errors` `282446025`,
  `[TemperPaw] Channel Transport Dispatch Failures` `282446031`,
  `[TemperPaw] Approval Notification Failures` `282446038`,
  `[TemperPaw] Session Phase Budget Exceeded` `282446057`,
  `[TemperPaw] Agent Session Trace Missing` `282446059`,
  `[TemperPaw] LLM Error Rate Spike` `282446060`,
  `[TemperPaw] LLM Latency Regression` `282446062`,
  `[TemperPaw] Postgres DBM Query Latency Regression` `282446064`,
  `[TemperPaw] Postgres DBM Missing APM Correlation` `282446065`, and
  `[TemperPaw] Sandbox Host HTTP Error Spike` `282446066`.
- Monitor reconciliation also updated the shared `[Temper] ...` monitors to
  `service:temperpaw` and `@slack-temperpaw-alerts`, then deleted five orphan
  old-named monitors:
  `[OpenPaw] Error Rate Spike` `270470278`,
  `[OpenPaw] Request Latency Spike (P95)` `270470281`,
  `[OpenPaw] No Traffic` `270470295`,
  `[OpenPaw] Webhook Receive Errors` `275383895`, and
  `[OpenPaw] Session Phase Budget Exceeded` `280526611`.
- Post-reconcile `search_datadog_monitors` for TemperPaw/old-identity terms
  returned monitors scoped to `service:temperpaw` and
  `@slack-temperpaw-alerts`.
- A later monitor refresh changed three trace-related monitors from generated
  trace-metric alerts to trace-analytics alerts because Datadog was not
  emitting the expected generated trace metrics for this OTLP setup. A direct
  PUT failed when changing monitor type, so `scripts/deploy_monitors.py` was
  patched to delete and recreate a named monitor when the live type differs from
  the source definition. After one transient Datadog 503, reconcile succeeded
  and recreated:
  - `[TemperPaw] Agent Session Trace Missing` as trace-analytics alert
    `282453353`, querying
    `service:temperpaw @entity_type:ManagedSession @action_name:(StartSession OR ResumeSession)`.
  - `[TemperPaw] LLM Latency Regression` as trace-analytics alert
    `282453356`, querying `service:temperpaw @module_name:provider_caller`
    with p95 duration.
  - `[TemperPaw] Postgres DBM Missing APM Correlation` as trace-analytics
    alert `282453358`, querying
    `service:temperpaw operation_name:postgresql.query @peer.service:temperpaw-postgres`.
- `scripts/deploy_pipelines.py --reconcile` updated log pipeline
  `TemperPaw / Temper Logs (ADR-0054)` with id
  `Wyq_6z_fTviM9uVH9MUIrQ`, updated log metrics
  `temperpaw.logs.errors`, `temperpaw.logs.warns`, and
  `temperpaw.logs.wasm.default_timeout_fallback`, and found no remaining
  `openpaw.*` log metrics.
- A credentialed Datadog API check returned exactly one live pipeline with the
  expected name/id, and `legacy_terms_present=False` for the fetched pipeline
  JSON.
- Datadog's facet REST endpoint returned 404 on this account/tier. The script
  therefore logged each desired facet as a manual Log Explorer registration
  step. Sensitive Data Scanner rules also remain source-of-truth definitions in
  the repo and require Datadog UI group context to apply.

Managed-agent production proof:

- First production run of
  `os-apps/paw-managed-agents/tests/prove_paw_managed_agents.py` exposed a
  proof race: after `ResumeSession`, the script accepted the previous `Idle`
  state before the resumed inner session had written its tool-use chronology.
  Production entity rows were correct; the proof logic was too eager.
- The proof was fixed to wait until the chronology includes the second
  `session.status_running`, second `session.status_idle`, `agent.tool_use`, and
  `agent.tool_result` rows before considering resume complete.
- The rerun completed successfully for ManagedSession
  `en-019e1dbb-2ab7-7393-92c1-0af6d853c831`.
  - Final status: `Terminated`.
  - Start inner session: `ss-019e1dbb-3a43-72b1-9308-94b56ce0fbe9`.
  - Resume/final inner session: `ss-019e1dbb-b0ea-7283-bbc0-db455269c873`.
  - Managed agent: `en-019e1dbb-1c07-7633-931a-1729eb401f09`.
  - Environment: `en-019e1dbb-1986-74a0-afc9-7ea131911d32`.
- Verified event chronology:
  1. `user.message`
  2. `session.status_running` for `ManagedAgents.StartSession`
  3. `agent.message`
  4. `session.status_idle`
  5. `user.message`
  6. `session.status_running` for `ManagedAgents.ResumeSession`
  7. `agent.tool_use`
  8. `agent.tool_result`
  9. `agent.message`
  10. `session.status_idle`
  11. `session.status_terminated` for `ManagedAgents.TerminateSession`
- Negative checks also passed: bogus event kind rejected, archived session child
  row creation blocked, and archived agent new-session creation blocked.
- Datadog span search for
  `@entity_id:en-019e1dbb-2ab7-7393-92c1-0af6d853c831 OR @managed_session_id:en-019e1dbb-2ab7-7393-92c1-0af6d853c831`
  returned 196 spans. Representative trace id:
  `aa71d8381c56e44bf2585815d1f2d206`.
- Trace Explorer URL:
  `https://app.datadoghq.com/apm/traces?end=1778615376397&historicalData=true&paused=true&query=%40entity_id%3Aen-019e1dbb-2ab7-7393-92c1-0af6d853c831+OR+%40managed_session_id%3Aen-019e1dbb-2ab7-7393-92c1-0af6d853c831&start=1778613576397`
- Trace analytics aggregate for
  `service:temperpaw @entity_type:ManagedSession @action_name:(StartSession OR ResumeSession)`
  returned Start/Resume buckets for integration execution, dispatch adapter,
  admission acquire, and ask/reply phases.
- Trace analytics aggregate for
  `service:temperpaw operation_name:postgresql.query @peer.service:temperpaw-postgres`
  returned many live DB span buckets, proving the DBM/APM correlation monitor
  query matches actual production spans.
- Querying `service:temperpaw operation_name:temperpaw.agent.session` returned
  zero spans. This is now recorded as a Temper host-span export gap: the
  meaningful ManagedSession action spans and fields exist, but the semantic
  span-hint name is not exposed by Datadog APM as an operation/resource.

Post-refresh source update:

- Temper commit `314a246d32a91036a0a6e542dfdd66532d7aec7a` was created and
  pushed on branch `codex/temperpaw-llmobs-service-identity-main`.
- That commit adds ADR-0083 and changes WASM host HTTP/text, binary, and
  streaming spans so `X-Temper-Span-Name` is used as the initial `otel.name`,
  while common session/entity/tool/LLM hint attributes are recorded as static
  Datadog-visible tracing fields and as generic OTel attributes. It also records
  active trace/span ids, Datadog decimal trace/span ids, session id, and entity
  context on guest WASM log span events.
- Temper verification before push:
  - `cargo fmt --check` passed.
  - `cargo test -p temper-wasm -- --nocapture` passed: 95 unit tests, 2
    authorized host streaming tests, 5 e2e invoke tests, 4 HTTP stream outbound
    tests, and doc tests.
  - `cargo clippy -p temper-wasm --all-targets -- -D warnings` passed.
  - The repository pre-push pipeline passed all gates, including the full test
    suite.
- TemperPaw source now pins the Temper dependency to
  `314a246d32a91036a0a6e542dfdd66532d7aec7a`. This is not live production
  proof until a new TemperPaw image is built, deployed, exercised, and verified
  in Datadog.

Second post-refresh source update:

- Temper commit `974b13bf02342a1b8faafdb1b762572933fe1c3e` was created and
  pushed on branch `codex/temperpaw-llmobs-service-identity-main`.
- That commit adds ADR-0084 and introduces long-lived `temper.workflow` root
  spans for agent/workflow dispatches. Workflow roots are deliberately outside
  the short inbound HTTP request span, but downstream action/WASM/DB/LLM work
  adopts the workflow root as parent context. The registry now keeps only valid
  OpenTelemetry root contexts, so no-exporter tests and deterministic
  simulations do not retain no-op root spans or schedule no-op cleanup tasks.
- Temper verification before push:
  - `cargo fmt --check` passed.
  - `cargo test -p temper-server workflow_root_span -- --nocapture` passed:
    4 workflow tracing tests.
  - `cargo test -p temper-server request_context -- --nocapture` passed:
    9 request-context tests.
  - `cargo clippy --workspace -- -D warnings` passed.
  - The repository pre-push pipeline passed all gates, including the full
    workspace test suite.
- TemperPaw source now pins the Temper dependency to
  `974b13bf02342a1b8faafdb1b762572933fe1c3e`. This is not live production
  proof until a new TemperPaw image is built, deployed, exercised, and verified
  in Datadog.

Remaining blockers after this refresh:

- Railway project name, service name, generated public domain, private domain
  variables, and generated service URL variables still use the old external
  identity. Railway CLI can add services/domains but does not expose safe
  project/service rename. Creating a parallel `temperpaw` service would risk
  double-running production transports/workers against the same database, so
  this remains a dashboard/API-token migration item.
- Data/document service variables still include old external identity in bucket
  and compatibility URL names:
  - `BLOB_BUCKET=openpaw-fs-seshendranalla`
  - `PUBLISHED_BLOB_BUCKET=openpaw-fs-seshendranalla`
  - `TURSO_URL=libsql://openpaw-seshendranalla-...`
  These values should not be changed in-place until replacement buckets/URLs are
  provisioned and the app is migrated, because changing names without backing
  resources would break document/blob access.
- APM spans generated by WASM host calls include the public legacy domain when
  `temper_api_url` points at that URL. This is a consequence of the unresolved
  Railway domain rename, not a Rust service tag regression.
- LLMObs tree hierarchy is fixed, but `get_llmobs_agent_loop` does not yet show
  a populated chronological timeline for direct Session traces.
- Runtime variables previously had `DD_PROFILING_ENABLED=true`, but live profiling upload
  evidence is still absent. The live `[Temper] Profiler Uploads Stalled`
  monitor is alerting and `search_datadog_metrics` found no
  `datadog.profiling.*` data for `service:temperpaw`.
- The live `[TemperPaw] Agent Session Trace Missing` monitor has been adjusted
  to trace analytics over actual ManagedSession action spans. The remaining gap
  is semantic span-hint export: Datadog APM does not yet expose
  `temperpaw.agent.session` as a searchable operation/resource even though the
  bridge context fields and action spans are queryable.
- Log facets and Sensitive Data Scanner rules could not be applied by API on
  this Datadog account/tier. Source definitions are present in
  `dd-pipelines/facets.json` and `dd-pipelines/sensitive-data-scanner.json`,
  but UI application proof is still required.

## Earlier live production verification refresh — 2026-05-12T16:12Z

Deployment and runtime:

- Railway deployment `de0f9350-3b89-4c2a-afbf-5a76d94b3a32` reached
  `SUCCESS` with `stopped:false`.
- `GET https://openpaw-production.up.railway.app/readyz` returned HTTP 200 with
  `status:"ready"` and Discord `connected:true`.
- The deployed image is `ghcr.io/nerdsane/temperpaw:sha-d989ac6`, digest
  `sha256:dd2c3222c36b189b48e5b1a37b764a7cc0ad7d25d13dc62ac1a48c379831f6ad`.
- Datadog APM confirmed live spans from the new build:
  `service:temperpaw`, `service.namespace:temperpaw`, `team:temperpaw`,
  `env:prod`, and
  `version:d989ac6571491223a651476e5f8dd713ac540b83`.
- APM span source paths showed Temper rev `c1be43d`, proving the running image
  includes the Temper LLMObs hierarchy patch.

Live proof session:

- Created production Session `ss-019e1cf2-dea3-7303-911a-1e96b2a096a9` through
  OData `TemperPaw.Configure` with `provider:openai_codex`, `model:gpt-5.5`,
  `max_turns:1`, `tools_enabled:""`, and a proof prompt.
- Session reached `Completed`.
- Result was exactly `TemperPaw LLMObs hierarchy verified.`
- Entity fields included:
  - `gen_ai_parent_trace_id:526d78eb50ad335ea2b3fa23f67a1939`
  - `gen_ai_parent_span_id:c9f4b064482d0e34`
  - `llmobs_agent_span_id:188599c674d42033`
  - `llmobs_workflow_span_id:9134749188225588737`
  - `provider_request_bytes:980`
  - `provider_response_bytes:9943`

LLM Observability:

- `search_llmobs_spans(ml_app="temperpaw", tags={"session_id": ...})`
  returned exactly three spans for trace
  `109565108544682214381101706109007829305`:
  - agent root `temperpaw.agent_session`, span `1766987506455420979`,
    `parent_id:undefined`, duration 5589 ms.
  - workflow child `Session.ProviderAuthReady`, span
    `9134749188225588737`, parent `1766987506455420979`, duration 5489 ms.
  - LLM child `wasm:provider_caller`, span `14552450240695045684`,
    parent `9134749188225588737`, duration 5389 ms.
- `get_llmobs_trace` returned `error_count:0`, `has_errors:false`,
  `span_kinds:{agent:1, workflow:1, llm:1}`, `tree_depth:3`, and service list
  `["temperpaw"]`.
- This fixes the previous broken direct LLMObs trace shape where the content
  span appeared alone with a missing parent.
- `get_llmobs_agent_loop` for the root agent span returned an empty timeline
  (`iterations:[]`, `timeline:null`). The LLMObs tree is correct and
  agent-readable, but Datadog's agent-loop helper is not yet useful for this
  direct-API trace shape.

APM:

- `search_datadog_spans` for
  `service:temperpaw @entity_id:ss-019e1cf2-dea3-7303-911a-1e96b2a096a9`
  returned 72 spans in the last 30-minute window.
- Correlated APM trace id: `526d78eb50ad335ea2b3fa23f67a1939`.
- Condensed trace root:
  `POST /tdata/Sessions('ss-{guid}')/TemperPaw.Configure`, HTTP 200,
  duration 74.94 ms, 175 hidden child spans.
- Expanded trace showed chronological platform/WASM work including
  `Session.Configure`, `ProvisionWorkspace`, `workspace_provisioner`,
  `WorkspaceReady`, `context_preparer`, `ProviderAuthReady`,
  `provider_auth_gate`, `provider_caller`, `CheckSteering`, and terminal
  `FinalizeResult` entity history in the OData wait payload.
- Span fields include `entity_id`, `entity_type`, `action_name`,
  `workflow.run_id`, `workflow.root_entity_id`, `workflow.root_entity_type`,
  `module_name`, `trigger_action`, `service.version`, and `team`.

Logs:

- `analyze_datadog_logs` for `(service:temperpaw OR service:openpaw)`,
  grouped by `service,status,version`, returned:
  - `temperpaw info d989ac6571491223a651476e5f8dd713ac540b83`: 9930
  - `temperpaw info e47720671e2af9d280c0d4a796dee2154a3f2151`: 2549
  - `temperpaw warn d989ac6571491223a651476e5f8dd713ac540b83`: 301
- `search_datadog_logs` for `service:openpaw OR "OpenPAW" OR "OpenPaw" OR
  "Open Paw" OR "openpaw"` over the same live window returned count 0.
- `search_datadog_logs` for
  `service:temperpaw status:(error OR critical OR alert OR emergency)` returned
  count 0.
- Warning patterns on the new build were startup/spec/liveness and proof-related
  warnings, not legacy identity service leaks:
  - 287 `liveness coverage missing (ADR-0050)` warnings at startup.
  - 4 `Skipping app directory — missing required app.toml`.
  - 4 `Could not resolve target entity ID for reaction`.
  - 2 `unmet_intent` warnings, one for `openai-codex-auth` during provider auth.
  - 1 proof-induced soul lookup warning because the proof passed `soul_id:Paw`
    but no active soul matched that reference.

Postgres DBM:

- `find_datadog_database_instances` with tags `service:temperpaw` and
  `team:temperpaw` found `temperpaw-postgres`.
- `search_datadog_database_samples` found live query samples tagged
  `service:temperpaw`, `team:temperpaw`, `database_instance:temperpaw-postgres`,
  `source:postgres`, and `dbm:true`.
- Sample query signature `834f4482c8c79451`:
  `SELECT sequence_nr, state FROM snapshots WHERE tenant = ? AND entity_type = ? AND entity_id = ?`
  against table `snapshots`, command `SELECT`, database `railway`, user
  `postgres`, wait event `CPU`.
- `get_datadog_database_calling_services(database_instance="temperpaw-postgres")`
  returned `calling_resources:[]`. DBM collection is live, but DBM-to-APM
  calling-service correlation is not yet complete.

Remaining blockers:

- Railway project name, service name, generated public domain, private domain
  variables, and generated service URL variables still use the old external
  identity. Railway CLI can add services/domains but does not expose safe
  project/service rename. Creating a parallel `temperpaw` service would risk
  double-running production transports/workers against the same database, so
  this remains a dashboard/API-token migration item.
- Data/document service variables still include old external identity in bucket
  and compatibility URL names:
  - `BLOB_BUCKET=openpaw-fs-seshendranalla`
  - `PUBLISHED_BLOB_BUCKET=openpaw-fs-seshendranalla`
  - `TURSO_URL=libsql://openpaw-seshendranalla-...`
  These values should not be changed in-place until replacement buckets/URLs are
  provisioned and the app is migrated, because changing names without backing
  resources would break document/blob access.
- APM spans generated by WASM host calls include the public legacy domain when
  `temper_api_url` points at that URL. This is a consequence of the unresolved
  Railway domain rename, not a Rust service tag regression.
- LLMObs tree hierarchy is fixed, but `get_llmobs_agent_loop` does not yet show
  a populated chronological timeline for direct Session traces.
- DBM query samples are present, but Datadog calling-service correlation is not
  populated yet.
- Runtime variables previously had `DD_PROFILING_ENABLED=true`, but Datadog metric catalog
  lookups for Rust/generic profile-upload metrics and log search for
  profiler/profiling/profile produced no live profile evidence for
  `service:temperpaw`. Live profiling upload evidence still needs to be
  collected for the active service.
- Datadog dashboard/monitor/log-metric/facet/pipeline reconciliation still needs
  live credentialed apply proof.

## Static validation

- `jq empty dd-dashboards/temperpaw-overview.json dd-monitors/temperpaw-monitors.json dd-pipelines/facets.json dd-pipelines/sensitive-data-scanner.json dd-pipelines/temper-temperpaw.json dd-log-metrics/temper-log-metrics.json` passed.
- `cargo fmt --check` passed after formatting the touched Rust files.
- `git diff --check` passed after the managed-session structured log proof
  refresh.
- `cargo test -p paw-transport -- --nocapture` passed: 30 tests.
- `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture` passed: 18 tests.
- `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture` passed after adding `.github` to active identity scanning and explicit cleanup allowlists.
- `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture` passed again after adding `docs/temperpaw-datadog-observability-guide.md`, confirming the new active guide does not reintroduce legacy identity strings.
- `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture` passed again after renaming the active success contract to `docs/temperpaw-identity-and-observability-success-contract.md`; the exact legacy identity terms remain only where needed to define cleanup criteria or historical allowlists.
- `cargo test -p temperpaw --test session_lifecycle_and_config -- --nocapture` passed: 6 tests.
- `cargo test --manifest-path os-apps/paw-managed-agents/wasm/session_orchestrator/Cargo.toml -- --nocapture` passed: 9 tests, including the managed-session span hint and running-event bridge context unit tests.
- `/Users/seshendranalla/Development/temper/target/debug/temper verify-ioa < os-apps/paw-managed-agents/specs/session_event.ioa.toml` passed: symbolic verification, model check, simulation, and 100 property-test cases all passed.
- `cargo test -p temperpaw --test datadog_monitor_config -- --nocapture` passed: 3 tests.
- `cargo test -p temperpaw-cli -- --nocapture` passed: 34 tests.
- `python3 -m py_compile scripts/deploy_dashboard.py scripts/deploy_monitors.py scripts/deploy_pipelines.py os-apps/paw-agent/modal-bridge/modal_bridge.py` passed.
- `python3 -m py_compile os-apps/paw-managed-agents/tests/prove_paw_managed_agents.py` passed.
- `ruby -e 'require "yaml"; YAML.load_file("scripts/otel-collector-railway.yaml"); YAML.load_file("scripts/otel-collector-datadog.yaml")'` passed.
- Structured managed-session log verification at `2026-05-11T23:40:09Z`:
  - `cargo test -p temperpaw --test datadog_observability_contract managed_session_events_expose_queryable_bridge_context -- --nocapture` passed.
  - `cargo test --manifest-path os-apps/paw-managed-agents/wasm/session_orchestrator/Cargo.toml -- --nocapture` passed: 12 tests.
  - `cargo test --manifest-path os-apps/paw-managed-agents/wasm/event_emitter/Cargo.toml -- --nocapture` passed: 13 tests.
  - `cargo test --manifest-path os-apps/paw-managed-agents/wasm/session_terminator/Cargo.toml -- --nocapture` passed: 11 tests.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture` passed: 18 tests.
  - `jq empty dd-pipelines/facets.json dd-pipelines/temper-temperpaw.json` passed.
  - `bash os-apps/paw-managed-agents/wasm/build.sh` passed and refreshed the managed-agent WASM artifacts.
- Local runtime smoke at `2026-05-11T19:19:18Z`:
  - Started the already-built server with
    `DD_SERVICE=temperpaw`, `DD_ENV=local`, `DD_TAGS=team:temperpaw`,
    `OTEL_ENABLED=false`, `TEMPERPAW_WASM_STARTUP_POLICY=build`,
    `TURSO_URL=file:/tmp/temperpaw-observability-e2e.db`,
    `TEMPER_API_KEY=observability-e2e-key`, `PAW_TENANT=observability_e2e`,
    and `PORT=4478`.
  - Boot reached `Temper Paw is running.`, `API: http://localhost:4478/tdata`,
    `Dashboard: http://localhost:4478/dashboard`, trigger listener
    `0.0.0.0:4490`, and `Paw is ready.`
  - `GET /healthz` returned HTTP 200.
  - `GET /readyz` returned HTTP 200 with `{"status":"ready","healthz":"/healthz","discord":{"status":"disconnected","configured":false,"connected":false}}`.
  - Authenticated `GET /tdata/Agents?$top=1` returned HTTP 200 and an active
    Paw Agent configured with `provider:"openai_codex"` and the
    `temper_datadog_query` tool.
  - The smoke used local OTEL disabled, so it proves boot/API/OData health, not
    live Datadog ingestion.
- Local runtime smoke refresh at `2026-05-11T19:54:28Z`, after the
  `monty_repl` chunk-size patch:
  - Started the server with `TEMPERPAW_WASM_STARTUP_POLICY=build`,
    `DD_SERVICE=temperpaw`, `DD_ENV=local`, `DD_TAGS=team:temperpaw`,
    `OTEL_ENABLED=false`, `PAW_TENANT=observability_e2e`, and `PORT=4479`.
  - Boot reached `Temper Paw is running.`, `API: http://localhost:4479/tdata`,
    `Dashboard: http://localhost:4479/dashboard`, trigger listener
    `0.0.0.0:4491`, and `Paw is ready.`
  - The previous `STREAM_CHUNK_BYTES` startup build error did not recur.
  - `GET /healthz` returned HTTP 200.
  - `GET /readyz` returned HTTP 200 with
    `{"status":"ready","healthz":"/healthz","discord":{"status":"disconnected","configured":false,"connected":false}}`.
  - Authenticated `GET /tdata/Agents` returned HTTP 200 and active Paw/SWE/SRE
    agent records using `provider:"openai_codex"` and the
    `temper_datadog_query` tool.
  - Stopped the local server and confirmed no
    `target/debug/temperpaw-server run` process remained.
- Local webhook trigger observability E2E at `2026-05-11T21:49:09Z`:
  - Started the already-built server with `DD_SERVICE=temperpaw`,
    `DD_ENV=local`, `DD_TAGS=team:temperpaw`, `OTEL_ENABLED=false`,
    `TEMPERPAW_WASM_STARTUP_POLICY=build`,
    `TURSO_URL=file:/tmp/temperpaw-webhook-e2e-20260511-2120.db`,
    `TEMPER_API_KEY=webhook-e2e-key`, `PAW_TENANT=webhook_e2e`, and
    `PORT=4560`; the trigger listener bound on `0.0.0.0:4572`.
  - The first sandboxed start failed with `Operation not permitted`; the
    escalated run booted and reached API readiness.
  - The escalated environment had no default `rustup` toolchain, so
    startup-time WASM rebuild attempts failed with `rustup could not choose a
    version of cargo to run`; startup continued by registering existing WASM
    binaries. This E2E proves local runtime behavior with the available built
    artifacts, not a fresh WASM rebuild in that escalated environment.
  - `GET /healthz` returned HTTP 200.
  - `GET /readyz` returned HTTP 200 with
    `{"status":"ready","healthz":"/healthz","discord":{"status":"disconnected","configured":false,"connected":false}}`.
  - `POST /triggers/webhook/e2e-observability` returned HTTP 200 with
    `{"event_id":"en-019e1903-797a-76d0-9b83-4eafeae4a260","status":"received"}`.
  - The running server emitted the structured webhook signal:
    `observability_event=temperpaw.webhook`,
    `webhook.operation=receive`, `webhook.outcome=success`,
    `webhook.route_key=e2e-observability`,
    `webhook.event_id=en-019e1903-797a-76d0-9b83-4eafeae4a260`,
    `webhook.status=200`, `webhook.payload_bytes=74`, and empty
    `error.message`; the structured event did not include the raw payload body.
  - Authenticated OData lookup of the created `WebhookEvent` returned HTTP 200.
    The entity was expectedly `Rejected` because no `WebhookRoute` existed for
    the synthetic key; state history showed `Created`, `Received`, and
    `ValidationFailed` with `validation_error:"no matching route"`.
  - Stopped the local server and confirmed `pgrep -fl temperpaw-server`
    returned no process.
  - OTEL was disabled for this smoke, so it proves local runtime logs and OData
    state transitions, not live Datadog ingestion.
- Red/green 2026-05-11 `monty_repl` streaming upload build drift:
  - The local runtime smoke exposed a startup WASM build error:
    `cannot find value STREAM_CHUNK_BYTES in module temper_wasm_sdk::http_stream`
    in `os-apps/paw-agent/wasm/monty_repl/src/entity_ops.rs`. Startup continued
    only because an existing `monty_repl` WASM binary was present.
  - Reproduced the failure with
    `cargo build --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml --target wasm32-wasip1 --release`.
  - Patched `entity_ops.rs` so `monty_repl` owns the file-stream request chunk
    size locally behind `#[cfg(target_arch = "wasm32")]`, matching the existing
    provider-caller ownership pattern instead of depending on a removed SDK
    constant.
  - Re-ran the same WASM build successfully. It still emits the pre-existing
    `unused doc comment` warning in `src/lib.rs`.
  - `cargo test --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml -- --nocapture`
    passed: 50 tests.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed: 13 tests at that point; after later data/document service coverage
    tightening the same contract passed 14 tests.
  - `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture`
    passed: 1 test.
  - `cargo fmt --check` and `git diff --check` passed.
- Red/green 2026-05-11 targeted span-hint tests:
  - `cargo test tool_span_hints_use_datadog_tool_operation_semconv -- --nocapture`
    in `os-apps/paw-agent/wasm/monty_repl` first failed because tool span hints did
    not emit `gen_ai.operation.name=execute_tool`, then passed after patching.
  - `cargo test llm_span_hint_headers_use_datadog_genai_semconv -- --nocapture`
    in `os-apps/paw-agent/wasm/provider_caller` first failed because LLM span
    hints did not emit top-level `session_id`, then passed after patching.
  - Full affected suites then passed:
    `cargo test -- --nocapture` in `os-apps/paw-agent/wasm/monty_repl`
    (50 tests) and `os-apps/paw-agent/wasm/provider_caller` (24 tests).
- Red/green 2026-05-11 Datadog dashboard reconciliation:
  - Added `datadog_dashboard_deploy_reconciles_legacy_dashboards`; it first
    failed because `scripts/deploy_dashboard.py` had no `--reconcile` path.
  - Patched `scripts/deploy_dashboard.py` to detect dashboard ownership by
    desired title, `team:temperpaw`, or legacy migration terms and delete stale
    owned dashboards when `--reconcile` is used.
  - Re-ran the Datadog contract and identity contract successfully.
- Profiling coverage is now part of the Datadog contract. Tightening the test
  first exposed that the dashboard had profiler metrics without an explicit
  `Profiling` section label; `dd-dashboards/temperpaw-overview.json` now names
  the section `Profiling - Continuous Profiler (ADR-0055)`.
- Red/green 2026-05-11 Railway collector routing:
  - Added `railway_otel_collectors_preserve_llmobs_routing`; it first failed
    because `scripts/otel-collector-railway.yaml` and the generated
    `temperpaw deploy` collector config did not split GenAI spans to LLMObs.
  - Patched both the checked-in Railway collector config and
    `crates/temperpaw-cli/src/deploy.rs::otel_datadog_config()` to include
    `traces/llmobs`, `traces/apm`, `otlphttp/llmobs`, `dd-otlp-source: llmobs`,
    and the same `gen_ai.operation.name` / `gen_ai.system` routing predicates.
  - `cargo test -p temperpaw-cli otel_datadog_config -- --nocapture` passed:
    3 tests.
  - Ruby YAML parsing passed for `scripts/otel-collector-railway.yaml` and
    `scripts/otel-collector-datadog.yaml`.
- The collector routing contract was tightened after checking Datadog's OTLP
  mapping: LLMObs routing now keys on `gen_ai.operation.name` as well as the
  legacy `gen_ai.system`, so agent spans such as `invoke_agent` are not dropped.
- Red/green 2026-05-11 Railway profiling activation:
  - Added `datadog_runtime_variables_pin_temperpaw_identity_and_enable_profiling_when_active`;
    it first failed because the deploy path had no shared Datadog runtime
    variable builder.
  - Patched `crates/temperpaw-cli/src/deploy.rs` so Railway runtime variables
    explicitly include `DD_SERVICE=temperpaw`, `DD_ENV=prod`, and
    `DD_TAGS=team:temperpaw`, and include `TEMPER_PROFILING_ENABLED=true` plus
    `TEMPER_PROFILING_AUTO_UPLOAD=true` whenever `DD_API_KEY` is configured.
  - Updated `.env.example` and the observability guide with the same runtime
    identity/profiling contract.
  - `cargo test -p temperpaw-cli -- --nocapture` passed.
  - `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture`
    passed after the env/example guide update.
- Red/green 2026-05-11 OpenTelemetry DBM correlation gate:
  - Datadog's current OpenTelemetry DBM correlation docs require the collector
    to start with `datadog.EnableOperationAndResourceNameV2` for recent
    collector versions so database spans can be processed for DBM correlation:
    https://docs.datadoghq.com/opentelemetry/correlate/dbm_and_traces/
  - Added `otel_collector_entrypoint_enables_datadog_dbm_correlation_feature_gate`;
    it first failed because the generated Railway collector entrypoint was not
    testable and did not include the gate.
  - Patched `crates/temperpaw-cli/src/deploy.rs` to generate the collector
    entrypoint through a helper and start Datadog-enabled collector runs with
    `--feature-gates=datadog.EnableOperationAndResourceNameV2`.
  - `cargo test -p temperpaw-cli -- --nocapture` passed: 32 tests.
- Red/green 2026-05-11 DB span type enrichment:
  - Tightened the Datadog observability contract and CLI collector tests to
    require a `transform/dbm` processor and `span.type=sql` insertion for spans
    carrying `db.system`; the tests first failed because the collector configs
    only handled LLMObs/APM routing.
  - Patched `scripts/otel-collector-datadog.yaml`,
    `scripts/otel-collector-railway.yaml`, and
    `crates/temperpaw-cli/src/deploy.rs::otel_datadog_config()` so the APM
    trace pipeline applies `transform/dbm` before the GenAI exclusion filter.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed.
  - `cargo test -p temperpaw-cli -- --nocapture` passed.
  - Ruby YAML parsing passed for both checked-in collector YAML files.
- Red/green 2026-05-11 Postgres DBM Agent deploy:
  - Added CLI tests for a Railway Datadog Postgres Agent config/variable source
    of truth; they first failed because no DBM Agent helpers existed.
  - Patched `crates/temperpaw-cli/src/deploy.rs` to deploy a
    `datadog-postgres-agent` Railway service when Datadog and Railway Postgres
    are both enabled. The generated Agent image is based on `datadog/agent:7`,
    loads a Postgres integration config with `dbm: true`, and receives Railway
    Postgres variable references (`PGHOST`, `PGPORT`, `PGUSER`, `PGPASSWORD`,
    `PGDATABASE`) plus TemperPaw service/team tags.
  - Added `deploy_configures_postgres_dbm_agent_when_datadog_is_enabled` to the
    Datadog observability contract.
  - `cargo test -p temperpaw-cli -- --nocapture` passed: 34 tests.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed.
- Red/green 2026-05-11 managed agent session root hints:
  - Tightened `temperpaw_span_hints_expose_session_tool_and_llmobs_semconv` so
    the Datadog contract requires `temperpaw.agent.session` span hints with
    `gen_ai.operation.name=invoke_agent`, `session_id`, `managed_session_id`,
    `inner_session_id`, `agent_id`, `environment_id`, `entity_type`, and
    `action_name`.
  - The test first failed because `paw-managed-agents` had no
    `agent_session_span_hint_headers` helper and `session_orchestrator` used
    plain system headers for inner-session configure/steer calls.
  - Patched `os-apps/paw-managed-agents/wasm/common.rs` to build the session
    span hint headers while preserving the original auth/content headers, and
    patched `os-apps/paw-managed-agents/wasm/session_orchestrator/src/lib.rs`
    to apply those hints around `TemperPaw.Configure` and `TemperPaw.Steer`
    calls for `ManagedAgents.StartSession` / `ManagedAgents.ResumeSession`.
  - The patch deliberately does not add hints to the polling
    `CheckInnerSession` path, keeping the session trace high-signal instead of
    generating repetitive check spans.
  - `cargo test --manifest-path os-apps/paw-managed-agents/wasm/session_orchestrator/Cargo.toml -- --nocapture`
    passed: 8 tests.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed.
- Red/green 2026-05-11 managed SessionEvent chronology bridge context:
  - Tightened `managed_session_events_expose_queryable_bridge_context`; it first
    failed because `SessionEvent` did not expose `parent_session_id`, and
    terminal/derived chronology rows were not required to carry the bridge
    fields.
  - Patched `os-apps/paw-managed-agents/specs/session_event.ioa.toml` and
    `specs/model.csdl.xml` with `ParentSessionId` on `SessionEvent`.
  - Patched `os-apps/paw-managed-agents/wasm/common.rs` with shared
    `managed_session_event_context` and `with_session_event_context` helpers.
  - Patched `session_orchestrator`, `event_emitter`, and
    `session_terminator` so high-signal chronological rows carry
    `observability_event=temperpaw.agent.session`, `managed_session_id`,
    `inner_session_id`, `inner_agent_id`, `managed_agent_id`,
    `parent_session_id`, `environment_id`, and `action_name`. Covered rows:
    `session.status_running`, `agent.message`, `agent.thinking`,
    `agent.tool_use`, `agent.tool_result`, `session.status_idle`, and
    `session.status_terminated`. Poll/check events remain intentionally
    omitted to avoid repetitive trace noise.
  - Updated
    `os-apps/paw-managed-agents/adrs/003-session-event-observability-fields.md`
    and `docs/temperpaw-datadog-observability-guide.md` to document that the
    entity chronology now has queryable bridge context beyond only the
    start/resume boundary.
  - Hardened `os-apps/paw-managed-agents/tests/prove_paw_managed_agents.py` so
    the E2E proof asserts bridge fields on the managed-session chronology and
    waits for asynchronous `event_emitter` rows before checking them.
  - Verification passed:
    `cargo test -p temperpaw --test datadog_observability_contract managed_session_events_expose_queryable_bridge_context -- --nocapture`;
    `cargo test --manifest-path os-apps/paw-managed-agents/wasm/session_orchestrator/Cargo.toml -- --nocapture`
    (11 tests);
    `cargo test --manifest-path os-apps/paw-managed-agents/wasm/event_emitter/Cargo.toml -- --nocapture`
    (12 tests);
    `cargo test --manifest-path os-apps/paw-managed-agents/wasm/session_terminator/Cargo.toml -- --nocapture`
    (10 tests);
    `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    (18 tests);
    `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture`
    (1 test);
    `/Users/seshendranalla/Development/temper/target/debug/temper verify-ioa < os-apps/paw-managed-agents/specs/session_event.ioa.toml`;
    `cargo fmt --check`; and `git diff --check`.
  - WASM verification passed:
    `cargo build --manifest-path os-apps/paw-managed-agents/wasm/session_orchestrator/Cargo.toml --target wasm32-unknown-unknown --release`,
    `cargo build --manifest-path os-apps/paw-managed-agents/wasm/event_emitter/Cargo.toml --target wasm32-unknown-unknown --release`,
    `cargo build --manifest-path os-apps/paw-managed-agents/wasm/session_terminator/Cargo.toml --target wasm32-unknown-unknown --release`, and
    `bash os-apps/paw-managed-agents/wasm/build.sh`.
  - Local E2E at `2026-05-11T23:10:09Z`:
    started the server with `DD_SERVICE=temperpaw`, `DD_ENV=local`,
    `DD_TAGS=team:temperpaw`, `OTEL_ENABLED=false`,
    `TEMPERPAW_WASM_STARTUP_POLICY=build`,
    `TURSO_URL=file:/tmp/temperpaw-managed-events-e2e-20260511-2310.db`,
    `TEMPER_API_KEY=managed-events-e2e-key`,
    `PAW_TENANT=managed_events_e2e`, and `PORT=4565`.
    `GET /healthz` and `GET /readyz` returned HTTP 200.
    `python3 os-apps/paw-managed-agents/tests/prove_paw_managed_agents.py`
    then passed end to end with the mock provider, covering app install,
    managed-agent update, start, resume, tool-use/tool-result chronology,
    termination, archive gates, and the new bridge-field assertions. Observed
    event kinds included `user.message`, `session.status_running`,
    `agent.message`, `session.status_idle`, `agent.tool_use`,
    `agent.tool_result`, and `session.status_terminated`.
  - Two local E2E failures were useful and fixed/recorded:
    before `build.sh`, app install could not find `managed_agent_updater` in
    the WASM registry; after adding the chronology assertions, the proof script
    also needed to wait for asynchronous event rows and allow resumed sessions
    to produce more than one inner session id across the chronology.
  - Stopped the local server and confirmed `pgrep -fl temperpaw-server`
    returned no process. OTEL was disabled for this smoke, so this proves
    local state-machine/WASM/OData behavior, not live Datadog ingestion.
- Red/green 2026-05-11 managed-session structured Datadog logs:
  - Tightened `managed_session_events_expose_queryable_bridge_context` again so
    the Datadog contract requires `session_event.kind`,
    `session_event.sequence`, `session_event.stop_reason`, and
    `session_event.termination_reason` as log facets, and requires the
    managed-session WASM modules to emit structured
    `temperpaw.agent.session event` logs.
  - The test first failed on the missing `session_event.kind` facet.
  - Patched `dd-pipelines/facets.json` with the `session_event.*` facets.
  - Patched `os-apps/paw-managed-agents/wasm/common.rs` with
    `log_managed_session_event` and
    `managed_session_observability_log_fields`, then wired
    `session_orchestrator`, `event_emitter`, and `session_terminator` to emit
    one structured log per high-signal session chronology row.
  - Emitted fields are intentionally diagnostic, not message bodies:
    `observability_event=temperpaw.agent.session`, `session_id`,
    `managed_session_id`, `inner_session_id`, `inner_agent_id`,
    `managed_agent_id`, `agent_id`, `parent_session_id`, `environment_id`,
    `action_name`, nested `session_event.kind`,
    `session_event.sequence`, optional `session_event.stop_reason`, optional
    `session_event.termination_reason`, optional `tool.name`, and optional
    `tool.call_id`.
  - Local E2E at `2026-05-11T23:38:36Z`: started the server with
    `DD_SERVICE=temperpaw`, `DD_ENV=local`, `DD_TAGS=team:temperpaw`,
    `OTEL_ENABLED=false`, `TEMPERPAW_WASM_STARTUP_POLICY=build`,
    `TURSO_URL=file:/tmp/temperpaw-managed-logs-e2e-20260512-0001.db`,
    `TEMPER_API_KEY=managed-logs-e2e-key`,
    `PAW_TENANT=managed_logs_e2e`, and `PORT=4566`.
    `GET /healthz` and `GET /readyz` returned HTTP 200.
    `python3 os-apps/paw-managed-agents/tests/prove_paw_managed_agents.py`
    passed end to end with the mock provider.
  - Runtime logs emitted `temperpaw.agent.session event` entries for
    `session.status_running`, `agent.message`, `session.status_idle`,
    resumed `session.status_running`, `agent.tool_use`, `agent.tool_result`,
    final `agent.message`, final `session.status_idle`, and
    `session.status_terminated`. Observed fields included
    `parent_session_id=parent-proof-session`,
    `session_event.sequence` values from `2` through `11`,
    `tool.name=bash`, `tool.call_id=mock-tool-0-0`,
    `session_event.stop_reason=user_input_required`, and
    `session_event.termination_reason=cancelled`.
  - No message `Content` fields were emitted in these structured logs. OTEL
    was disabled for this smoke, so this proves local runtime log emission,
    not live Datadog ingestion.
  - Stopped the local server and confirmed `pgrep -fl temperpaw-server`
    returned no process.
- Red/green 2026-05-11 agent-operable Datadog diagnostics:
  - Added `agent_operating_guidance_teaches_complete_datadog_diagnostics`; it
    first failed because the SRE/TemperPaw agent instructions did not mention
    the `temperpaw.agent.session` diagnostic path.
  - Patched `os-apps/paw-agent/agents/sre/AGENT.md` and
    `os-apps/paw-agent/agents/paw/skills/temperpaw-agent/SKILL.md` so agents
    are taught the same Datadog vocabulary as humans: session root spans,
    `managed_session_id`, `inner_session_id`, `dd.trace_id`, `dd.span_id`, LLM
    Observability, `gen_ai.operation.name`, Postgres DBM, Database Monitoring,
    profiling, `get_llmobs_agent_loop`, and chronological non-redundant trace
    expectations.
  - The targeted test passed, then
    `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed: 13 tests.
  - `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture`,
    `cargo fmt --check`, and `git diff --check` passed after the guidance
    update.
- Red/green 2026-05-11 managed-session Datadog facets:
  - Tightened `datadog_facets_include_agent_session_diagnostic_fields`; it
    first failed because `managed_session_id` and related bridge fields were
    not registered as Datadog facets.
  - Patched `dd-pipelines/facets.json` so humans and agents can search/group
    logs by `observability_event`, `managed_session_id`, `inner_session_id`,
    `parent_session_id`, `inner_agent_id`, `managed_agent_id`, and
    `environment_id`.
  - Updated the observability guide query vocabulary with the same fields.
- Red/green 2026-05-11 sensitive-data scanner coverage:
  - Added `sensitive_data_scanner_covers_observability_and_agent_secret_shapes`;
    it first failed because the scanner did not cover Datadog API/application
    key assignments.
  - Patched `dd-pipelines/sensitive-data-scanner.json` with a
    `DD_API_KEY` / `DD_APP_KEY` / `DD_APPLICATION_KEY` redaction rule while
    preserving existing OpenAI/Anthropic, GitHub, Slack, email, and AWS
    redaction rules.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed.
  - `jq empty dd-pipelines/sensitive-data-scanner.json dd-pipelines/facets.json`
    passed.
- Red/green 2026-05-11 TemperFS/blob and document-service observability:
  - Added `datadog_covers_temperfs_blob_and_document_services`; it first failed
    because the Datadog contract did not explicitly require a
    `TemperFS Blob & Document Services` dashboard surface.
  - Patched `dd-dashboards/temperpaw-overview.json` so the blob/document group
    is explicitly named `TemperFS Blob & Document Services + Monty` while
    preserving the existing blob wait, local fast-path, remote transport, and
    prepared-context file widgets.
  - Patched `os-apps/paw-fs/wasm/blob_adapter/src/lib.rs` to emit structured
    WASM logs through `host_log_structured` with
    `observability_event=temperpaw.blob`, `workspace_id`, `file_id`,
    `content_hash`, `stream_id`, `content_type`, and nested `blob.operation`,
    `blob.outcome`, `blob.backend`, `blob.cache_hit`, `blob.status_code`, and
    `blob.size_bytes` fields.
  - Patched `dd-pipelines/temper-temperpaw.json` so Datadog parses
    `fields_json` from WASM structured logs; patched `dd-pipelines/facets.json`
    so those TemperFS/blob fields are searchable facets.
  - Added `[Temper] Blob Transport Wait Spike` to
    `dd-monitors/temperpaw-monitors.json`; Datadog
    `validate_monitor_definition` returned `is_valid: true`.
  - Updated the human guide and agent/SRE operating guidance with the
    `workspace_id` / `file_id` / `content_hash` diagnostic path and blob wait
    metrics.
  - `cargo test --manifest-path os-apps/paw-fs/wasm/blob_adapter/Cargo.toml -- --nocapture`
    passed: 3 tests.
  - `cargo build --manifest-path os-apps/paw-fs/wasm/blob_adapter/Cargo.toml --target wasm32-unknown-unknown --release`
    passed.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed: 14 tests.
  - `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture`
    passed: 1 test.
  - `jq empty dd-dashboards/temperpaw-overview.json dd-monitors/temperpaw-monitors.json dd-pipelines/facets.json dd-pipelines/temper-temperpaw.json dd-pipelines/sensitive-data-scanner.json dd-log-metrics/temper-log-metrics.json`
    passed.
  - `cargo fmt --check` and `git diff --check` passed after the change.
- Red/green 2026-05-11 TemperFS metadata-operation observability:
  - Tightened `datadog_covers_temperfs_blob_and_document_services` so document
    service coverage also requires structured `workspace_fs` logs and facets for
    `fs.operation`, `fs.path`, `fs.outcome`, and `fs.backend`.
  - The targeted contract first failed on missing `fs.operation` facet.
  - Patched `os-apps/paw-fs/wasm/workspace_fs/src/lib.rs` so mkdir,
    create_file, resolve_path, list_dir, delete_file, and rename emit
    `observability_event=temperpaw.fs`, `workspace_id`, and nested `fs.*`
    fields through `Context::log_structured`.
  - Patched `dd-pipelines/facets.json` and
    `dd-pipelines/temper-temperpaw.json` so those fields are parsed and
    searchable, and updated the human/SRE/agent guidance with
    `observability_event=temperpaw.fs` and `@fs.operation:create_file` pivots.
  - `cargo test -p temperpaw --test datadog_observability_contract datadog_covers_temperfs_blob_and_document_services -- --nocapture`
    passed.
  - `cargo test --manifest-path os-apps/paw-fs/wasm/workspace_fs/Cargo.toml -- --nocapture`
    passed: 1 test.
  - `cargo build --manifest-path os-apps/paw-fs/wasm/workspace_fs/Cargo.toml --target wasm32-unknown-unknown --release`
    passed.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed: 15 tests.
  - `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture`
    passed: 1 test.
  - `jq empty dd-dashboards/temperpaw-overview.json dd-monitors/temperpaw-monitors.json dd-pipelines/facets.json dd-pipelines/temper-temperpaw.json dd-pipelines/sensitive-data-scanner.json dd-log-metrics/temper-log-metrics.json`,
    `python3 -m py_compile scripts/deploy_dashboard.py scripts/deploy_monitors.py scripts/deploy_pipelines.py os-apps/paw-agent/modal-bridge/modal_bridge.py`,
    `cargo fmt --check`, and `git diff --check` passed.
  - Tightened the same contract again to require dashboard drilldown text and
    a dedicated `[TemperPaw] TemperFS Metadata Operation Errors` monitor for
    `observability_event:temperpaw.fs @fs.outcome:error`.
  - Patched `dd-dashboards/temperpaw-overview.json` with a TemperFS metadata
    diagnostic pivot note using `@workspace_id:<workspace id>`,
    `@fs.operation:create_file`, and `@fs.path:<path>`.
  - Patched `dd-monitors/temperpaw-monitors.json` with the metadata-operation
    log alert so operators distinguish File/Workspace metadata errors from
    later blob payload latency.
  - Re-ran the targeted contract:
    `cargo test -p temperpaw --test datadog_observability_contract datadog_covers_temperfs_blob_and_document_services -- --nocapture`
    passed.
  - Re-ran the full local guardrail set:
    `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed: 15 tests;
    `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture`
    passed: 1 test;
    `jq empty dd-dashboards/temperpaw-overview.json dd-monitors/temperpaw-monitors.json dd-pipelines/facets.json dd-pipelines/temper-temperpaw.json dd-pipelines/sensitive-data-scanner.json dd-log-metrics/temper-log-metrics.json`,
    `python3 -m py_compile scripts/deploy_dashboard.py scripts/deploy_monitors.py scripts/deploy_pipelines.py os-apps/paw-agent/modal-bridge/modal_bridge.py`,
    `cargo fmt --check`, and `git diff --check` passed.
  - Datadog `validate_monitor_definition` for
    `[TemperPaw] TemperFS Metadata Operation Errors` could not complete because
    the authenticated Datadog user is missing `logs_read_data`; the definition
    remains locally guarded and undeployed.
- Red/green 2026-05-11 Modal bridge and sandbox-operation observability:
  - Added `datadog_covers_modal_bridge_and_sandbox_operations`; it first failed
    because the Datadog dashboard did not expose a dedicated
    `Sandbox & Modal Bridge` surface.
  - Corrected the contract while implementing it so host HTTP metrics use the
    real Temper tag shape (`call_kind:text`) and sandbox-specific drilldown uses
    structured log facets (`observability_event=temperpaw.sandbox`,
    `sandbox_provider`, `sandbox_id`, and `sandbox.operation`).
  - Patched `os-apps/paw-agent/wasm/wasm-helpers/src/sandbox.rs` so sandbox
    create, health, read, write, delete, and bash operations emit structured
    `temperpaw.sandbox` fields: `sandbox_provider`, `sandbox_id`,
    `sandbox.operation`, `sandbox.outcome`, `sandbox.backend`,
    `sandbox.exit_code`, `sandbox.status_code`, and `sandbox.workdir`.
  - Patched `dd-dashboards/temperpaw-overview.json` with a
    `Sandbox & Modal Bridge` group covering host HTTP request/error/latency
    metrics and the Modal bridge diagnostic pivots.
  - Added `[TemperPaw] Sandbox Host HTTP Error Spike` to
    `dd-monitors/temperpaw-monitors.json`; initial Datadog validation failed
    because `default_zero` monitors cannot use `on_missing_data: resolve`, then
    passed after changing the monitor to `on_missing_data: default`.
  - Patched `dd-pipelines/facets.json`, `dd-pipelines/temper-temperpaw.json`,
    `docs/temperpaw-datadog-observability-guide.md`, the SRE manual, and the
    TemperPaw agent skill so humans and agents can diagnose Modal/sandbox
    failures by `sandbox_provider`, `sandbox_id`, `sandbox.operation`, bridge
    host HTTP metrics, and `modal_bridge_url` configuration.
  - Tightened the same contract to require bridge-side structured logging from
    `os-apps/paw-agent/modal-bridge/modal_bridge.py`; the test failed first on
    missing `modal_bridge.operation` facets.
  - Patched the Modal bridge so create, health, file read/write/delete, exec,
    and terminate endpoints print structured JSON events with
    `service=temperpaw`, `source=modal_bridge`,
    `observability_event=temperpaw.sandbox`, `sandbox_provider=modal`,
    `sandbox_id`, nested `sandbox.*` fields, and nested `modal_bridge.operation`,
    `modal_bridge.endpoint`, and `modal_bridge.duration_ms`. The bridge does not
    log auth tokens or command bodies.
  - Added `modal_bridge.operation`, `modal_bridge.endpoint`, and
    `modal_bridge.duration_ms` to `dd-pipelines/facets.json`, and updated the
    guide/SRE/agent instructions with those bridge-side pivots.
  - `cargo test -p temperpaw --test datadog_observability_contract datadog_covers_modal_bridge_and_sandbox_operations -- --nocapture`
    passed.
  - `cargo test --manifest-path os-apps/paw-agent/wasm/wasm-helpers/Cargo.toml -- --nocapture`
    passed: 25 tests.
  - `cargo build --manifest-path os-apps/paw-agent/wasm/wasm-helpers/Cargo.toml --target wasm32-unknown-unknown --release`
    passed.
  - `cargo test --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml -- --nocapture`
    passed: 50 tests; the pre-existing unused doc-comment warning remains.
  - `cargo build --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml --target wasm32-wasip1 --release`
    passed; the pre-existing unused doc-comment warning remains.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed: 15 tests.
  - `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture`
    passed: 1 test.
  - `python3 -m py_compile scripts/deploy_dashboard.py scripts/deploy_monitors.py scripts/deploy_pipelines.py os-apps/paw-agent/modal-bridge/modal_bridge.py`
    passed.
  - `jq empty dd-dashboards/temperpaw-overview.json dd-monitors/temperpaw-monitors.json dd-pipelines/facets.json dd-pipelines/temper-temperpaw.json dd-pipelines/sensitive-data-scanner.json dd-log-metrics/temper-log-metrics.json`
    passed.
  - `cargo fmt --check` and `git diff --check` passed.
- Red/green 2026-05-11 channel transport observability:
  - Added `datadog_covers_channel_transport_observability`; it first failed
    because the dashboard did not expose a `Channel Transports` surface.
  - Added `slack_ingress_logging_uses_structured_tracing_without_message_body`
    in `paw-transport`; it first failed because Slack transport had no
    structured transport event helper and still relied on `println!` /
    `eprintln!`.
  - Patched `crates/paw-transport/src/slack/transport.rs` so Slack Socket Mode
    health, webhook reply delivery, inbound messages, slash commands, approval
    interactions, and dispatch failures emit `tracing` fields with
    `observability_event=temperpaw.transport`, `transport.name=slack`,
    `transport.operation`, `transport.outcome`, `transport.channel_id`,
    `transport.message_id`, `transport.command`, `transport.webhook_port`,
    `slack.user_id`, and `message.length`.
  - Removed the old Slack message-body logger from
    `crates/paw-transport/src/slack/api.rs`; transport logs now expose message
    length and identifiers, not message bodies.
  - Tightened the same contract again for Slack Socket Mode lifecycle events;
    `slack_socket_logging_uses_transport_observability_fields` first failed
    because `crates/paw-transport/src/slack/socket.rs` still used plain
    `println!` / `eprintln!` for connected, closed, parse-error, and ack-failure
    events.
  - Patched `slack/socket.rs` so Socket Mode lifecycle emits the same
    `observability_event=temperpaw.transport` vocabulary with
    `transport.operation=socket_mode`, `transport.outcome`,
    `slack.envelope_id`, and `slack.envelope_type`.
  - Patched `dd-dashboards/temperpaw-overview.json` with a
    `Channel Transports` diagnostic surface; patched
    `dd-pipelines/facets.json` with channel transport and Slack envelope
    facets; patched
    `dd-monitors/temperpaw-monitors.json` with
    `[TemperPaw] Channel Transport Dispatch Failures`.
  - Updated the human guide plus SRE/TemperPaw agent guidance with
    `@observability_event:temperpaw.transport`, `@transport.name:slack`,
    `@transport.name:discord`, `@transport.operation:receive_message`, and
    `@transport.outcome:error` pivots.
  - `cargo test -p paw-transport -- --nocapture` passed: 30 tests after the
    Socket Mode lifecycle addition.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed: 16 tests.
  - `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture`
    passed: 1 test.
  - `jq empty dd-dashboards/temperpaw-overview.json dd-monitors/temperpaw-monitors.json dd-pipelines/facets.json dd-pipelines/temper-temperpaw.json dd-pipelines/sensitive-data-scanner.json dd-log-metrics/temper-log-metrics.json`,
    `python3 -m py_compile scripts/deploy_dashboard.py scripts/deploy_monitors.py scripts/deploy_pipelines.py os-apps/paw-agent/modal-bridge/modal_bridge.py`,
    `cargo fmt --check`, and `git diff --check` passed.
  - Datadog `validate_monitor_definition` for
    `[TemperPaw] Channel Transport Dispatch Failures` could not complete
    because the authenticated Datadog user is missing `logs_read_data`; the
    definition remains locally guarded and undeployed.
- Red/green 2026-05-11 governance approval observability:
  - Added `datadog_covers_governance_approval_observability`; it first failed
    because the dashboard did not expose a `Governance Approvals` surface.
  - Added
    `approval_observability_fields_are_structured_and_do_not_include_message_body`
    in `request_approval`; it first failed because no structured approval event
    builder existed.
  - Patched `os-apps/paw-agent/wasm/request_approval/src/lib.rs` so callback
    registration, out-of-band approval skips, channel-session resolution,
    decision lookup failures, webhook notification failures, and successful
    human notifications emit structured `temperpaw.approval` fields:
    `decision_id`, `session_id`, `agent_id`, `parent_session_id`,
    `active_plan_id`, nested `approval.operation`, `approval.outcome`,
    `approval.delivery`, `approval.reason`, `approval.action`, and
    `approval.http_status`. The structured event intentionally does not
    duplicate the human notification body.
  - Patched `dd-dashboards/temperpaw-overview.json` with a
    `Governance Approvals` diagnostic surface; patched
    `dd-pipelines/facets.json` with approval facets; patched
    `dd-monitors/temperpaw-monitors.json` with
    `[TemperPaw] Approval Notification Failures`.
  - Updated the human guide plus SRE/TemperPaw agent guidance with
    `@observability_event:temperpaw.approval`, `@decision_id:<decision id>`,
    `@approval.operation:notify_human`, and `@approval.outcome:error` pivots.
  - `cargo test --manifest-path os-apps/paw-agent/wasm/request_approval/Cargo.toml -- --nocapture`
    passed: 4 tests.
  - `cargo build --manifest-path os-apps/paw-agent/wasm/request_approval/Cargo.toml --target wasm32-unknown-unknown --release`
    passed.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed: 17 tests.
  - `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture`
    passed: 1 test.
  - `jq empty dd-dashboards/temperpaw-overview.json dd-monitors/temperpaw-monitors.json dd-pipelines/facets.json dd-pipelines/temper-temperpaw.json dd-pipelines/sensitive-data-scanner.json dd-log-metrics/temper-log-metrics.json`,
    `python3 -m py_compile scripts/deploy_dashboard.py scripts/deploy_monitors.py scripts/deploy_pipelines.py os-apps/paw-agent/modal-bridge/modal_bridge.py`,
    `cargo fmt --check`, and `git diff --check` passed.
  - Datadog `validate_monitor_definition` for
    `[TemperPaw] Approval Notification Failures` could not complete because the
    authenticated Datadog user is missing `logs_read_data`; the definition
    remains locally guarded and undeployed.
- Red/green 2026-05-11 webhook trigger observability:
  - Added `datadog_covers_webhook_trigger_observability`; it first failed
    because the dashboard did not expose a `Webhook Triggers` surface.
  - Added `webhook_logging_uses_structured_tracing_without_payload_body` in
    `paw-transport`; it first failed because the generic webhook trigger had no
    structured webhook event helper.
  - Patched `crates/paw-transport/src/webhook/trigger.rs` so listener readiness,
    WebhookEvent creation failures, missing event ids, Received-dispatch
    failures, and successful receives emit `observability_event=temperpaw.webhook`
    with `webhook.operation`, `webhook.outcome`, `webhook.route_key`,
    `webhook.event_id`, `webhook.status`, and `webhook.payload_bytes`. The
    structured event intentionally records payload length, not raw payload body.
  - Corrected `[TemperPaw] Webhook Receive Errors` so it queries the emitted
    signal (`observability_event:temperpaw.webhook @webhook.outcome:error`)
    instead of the previously imaginary `@webhook.status` field alone.
  - Patched `dd-dashboards/temperpaw-overview.json` with a `Webhook Triggers`
    diagnostic surface; patched `dd-pipelines/facets.json` with webhook facets;
    updated the human guide plus SRE/TemperPaw agent guidance with webhook
    route/event pivots.
  - `cargo test -p paw-transport -- --nocapture` passed: 29 tests.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed: 18 tests.
  - `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture`
    passed: 1 test.
  - `jq empty dd-dashboards/temperpaw-overview.json dd-monitors/temperpaw-monitors.json dd-pipelines/facets.json dd-pipelines/temper-temperpaw.json dd-pipelines/sensitive-data-scanner.json dd-log-metrics/temper-log-metrics.json`,
    `python3 -m py_compile scripts/deploy_dashboard.py scripts/deploy_monitors.py scripts/deploy_pipelines.py os-apps/paw-agent/modal-bridge/modal_bridge.py`,
    `cargo fmt --check`, and `git diff --check` passed.
  - Datadog `validate_monitor_definition` for
    `[TemperPaw] Webhook Receive Errors` could not complete because the
    authenticated Datadog user is missing `logs_read_data`; the definition
    remains locally guarded and undeployed.
- Red/green 2026-05-11 agent-queryable Datadog diagnostics:
  - Gap found: SRE/TemperPaw agent guidance had been expanded to say
    `temper.datadog_query` can inspect logs, traces, LLM Observability, Postgres
    DBM, and profiling, but `monty_repl/src/datadog.rs` only supported
    `monitor_status`, `recent_events`, and `metrics_query`.
  - Checked current Datadog primary docs before implementation:
    - Logs search API: `POST /api/v2/logs/events/search`
      (`https://docs.datadoghq.com/api/latest/logs/`).
    - APM spans search API: `POST /api/v2/spans/events/search`
      (`https://docs.datadoghq.com/api/latest/spans/`).
    - LLMObs Export API: `POST /api/v2/llm-obs/v1/spans/events/search`
      with `Content-Type: application/vnd.api+json`
      (`https://docs.datadoghq.com/llm_observability/evaluations/export_api/`).
    - DBM samples/plans API: `POST https://app.<DD_SITE>/api/v1/logs-analytics/list?type=databasequery`
      (`https://docs.datadoghq.com/database_monitoring/guide/build_apps_with_dbm_api/`).
    - Metrics query API used for profiling health metrics:
      `GET /api/v1/query`
      (`https://docs.datadoghq.com/api/latest/metrics/`).
  - Added failing request-builder tests for `logs_query`, `trace_query`,
    `llmobs_query`, `dbm_query`, and `profiling_query`; they first failed
    because `build_datadog_request` did not exist.
  - Patched `datadog.rs` to build GET/POST Datadog requests with the correct
    site-specific API/app base URLs, request bodies, and content types.
  - Added failing summary tests for logs, traces, LLMObs, and DBM; they first
    failed because unsupported query kinds returned raw JSON.
  - Patched response summarization so agents receive compact JSON with the
    fields they need first: session ids, trace/span ids, operation names,
    service/status, model/provider, span kind, sandbox/Modal bridge fields, DBM
    query signatures, wait events, plan signatures, and truncation flags.
  - Updated the tool catalog description and the human guide's agent query
    section with supported query kinds and examples.
  - `cargo test --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml datadog_ -- --nocapture`
    passed: 4 tests.
  - `cargo test --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml -- --nocapture`
    passed: 53 tests; the pre-existing unused doc-comment warning remains.
  - `cargo test --manifest-path os-apps/paw-agent/wasm/tool-catalog/Cargo.toml -- --nocapture`
    passed: 1 test.
  - `cargo build --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml --target wasm32-wasip1 --release`
    passed; the pre-existing unused doc-comment warning remains.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed: 15 tests.
  - `cargo test -p temperpaw --test temperpaw_identity_contract -- --nocapture`
    passed: 1 test.
  - `jq empty dd-dashboards/temperpaw-overview.json dd-monitors/temperpaw-monitors.json dd-pipelines/facets.json dd-pipelines/temper-temperpaw.json dd-pipelines/sensitive-data-scanner.json dd-log-metrics/temper-log-metrics.json`,
    `python3 -m py_compile scripts/deploy_dashboard.py scripts/deploy_monitors.py scripts/deploy_pipelines.py os-apps/paw-agent/modal-bridge/modal_bridge.py`,
    `cargo fmt --check`, and `git diff --check` passed.
- Red/green 2026-05-11 dashboard managed-session pivots:
  - Tightened `dashboard_exposes_session_llm_database_logs_and_trace_surfaces`
    to require `managed_session_id` and `inner_session_id` in the dashboard.
  - The targeted dashboard test first failed because the Agent Session Trace
    note only described `session_id` and `dd.trace_id` pivots.
  - Patched `dd-dashboards/temperpaw-overview.json` so the Agent Session Trace
    and Logs by Trace notes teach operators to search by `managed_session_id`
    and `inner_session_id` as well.
  - The targeted dashboard test then passed.
- Managed-session entity chronology 2026-05-11:
  - Added ADR `os-apps/paw-managed-agents/adrs/003-session-event-observability-fields.md`.
  - Patched `os-apps/paw-managed-agents/specs/session_event.ioa.toml` and
    `os-apps/paw-managed-agents/specs/model.csdl.xml` so `SessionEvent`
    exposes top-level queryable bridge fields: `observability_event`,
    `managed_session_id`, `inner_session_id`, `inner_agent_id`,
    `managed_agent_id`, `environment_id`, and `action_name`.
  - Patched `session_orchestrator` so `session.status_running` `SessionEvent`
    rows write those top-level fields and retain matching JSON `Content`.
  - This gives humans and agents a Temper-native timeline keyed by the managed
    bridge IDs even while live Datadog trace parenting is still blocked.
  - Added `managed_session_events_expose_queryable_bridge_context` and
    `running_event_content_records_bridge_observability_context`.
  - `cargo test --manifest-path os-apps/paw-managed-agents/wasm/session_orchestrator/Cargo.toml -- --nocapture`
    passed: 9 tests.
  - `cargo test -p temperpaw --test datadog_observability_contract -- --nocapture`
    passed: 12 tests.
  - `/Users/seshendranalla/Development/temper/target/debug/temper verify-ioa < os-apps/paw-managed-agents/specs/session_event.ioa.toml`
    passed with L0 symbolic verification, L1 model check, L2 simulation, and
    L3 property tests.
- Datadog `validate_monitor_definition` results for the new observability monitors:
  - `[TemperPaw] Agent Session Trace Missing`: valid.
  - `[TemperPaw] LLM Latency Regression`: valid.
  - `[TemperPaw] Postgres DBM Query Latency Regression`: valid.
  - `[TemperPaw] Postgres DBM Missing APM Correlation`: initial boolean `&&` query was invalid; patched to a numeric expression and revalidated successfully.
  - `[TemperPaw] Sandbox Host HTTP Error Spike`: initial missing-data option
    was invalid for `default_zero`; patched and revalidated successfully.
  - `[TemperPaw] LLM Error Rate Spike`: Datadog validation could not complete because the authenticated Datadog user is missing `logs_read_data`.
  - `[TemperPaw] TemperFS Metadata Operation Errors`: Datadog validation could
    not complete because the authenticated Datadog user is missing
    `logs_read_data`.
  - `[TemperPaw] Channel Transport Dispatch Failures`: Datadog validation could
    not complete because the authenticated Datadog user is missing
    `logs_read_data`.
  - `[TemperPaw] Approval Notification Failures`: Datadog validation could not
    complete because the authenticated Datadog user is missing
    `logs_read_data`.
  - `[TemperPaw] Webhook Receive Errors`: Datadog validation could not complete
    because the authenticated Datadog user is missing `logs_read_data`.

## Live Datadog identity inventory

Datadog dashboard search for `temperpaw OR openpaw` returned:

- Dashboard ID: `mn4-k3k-i66`
- Title: `TemperPaw - Platform Overview` / live title currently renders as `TemperPaw — Platform Overview`
- Live description still says: `Single pane of glass for OpenPaw agent orchestration health, runtime behavior, and Temper platform metrics.`
- Sample live queries still use `service:openpaw`, including:
  - `p99:temper_session_context_tokens{service:openpaw}`
  - `p99:temper_actor_registry_lock_wait_ms{service:openpaw,cold_start:true} by {entity_type}`
  - `avg:temper_dispatch_ask_latency_ms{service:openpaw} by {entity_type,action}`
- Refresh at `2026-05-11T16:49:19Z` still returned the same dashboard id
  and legacy service queries, including `p95:temper_dispatch_ask_attempts{service:openpaw}`,
  `sum:datadog.profiling.rust.upload_errors{service:openpaw} by {stage}.as_count()`,
  and `p99:temper_cedar_evaluation_duration{service:openpaw} by {decision}`.
- Refresh at `2026-05-11T19:00:00Z` still returned dashboard
  `mn4-k3k-i66` with live description `Single pane of glass for OpenPaw agent
  orchestration health, runtime behavior, and Temper platform metrics.` and
  legacy queries such as `avg:temper_dispatch_ask_latency_ms{service:openpaw}`,
  `sum:temper_state_timeout_reset_total{service:openpaw}`, and
  `sum:datadog.profiling.rust.upload_errors{service:openpaw}`.

Datadog monitor search for `TemperPaw OR OpenPAW OR openpaw OR temperpaw` returned live OpenPaw-named monitors, including:

- `[OpenPaw] No Traffic`
- `[OpenPaw] Error Rate Spike`
- `[OpenPaw] Webhook Receive Errors`
- `[OpenPaw] Session Phase Budget Exceeded`
- `[OpenPaw] Request Latency Spike (P95)`

Many `[Temper]` monitors also still query `service:openpaw` and notify `@slack-openpaw-alerts`.
Refresh at `2026-05-11T16:49:19Z` still returned the live `[OpenPaw]`
monitors and many `[Temper]` monitors querying `service:openpaw`; examples
include `[OpenPaw] Session Phase Budget Exceeded` in Alert and `[Temper]
Profiler Uploads Stalled` in Alert.
Refresh at `2026-05-11T19:00:00Z` still returned live OpenPaw/legacy monitors:
examples include `[OpenPaw] No Traffic`, `[OpenPaw] Error Rate Spike`,
`[OpenPaw] Webhook Receive Errors`, `[OpenPaw] Session Phase Budget Exceeded`,
and many `[Temper]` monitors querying `service:openpaw` and notifying
`@slack-openpaw-alerts`. Alerting examples at this refresh included `[Temper]
Profiler Uploads Stalled`, `[Temper] Integration Silent Exit (ADR-0056)`,
`[Temper] WASM Default Timeout Fallback Rate`, and `[Temper] State Timeout
Reset Rate Drop`.

Follow-up repo change: live dashboard and monitor searches showed stale legacy
identity can survive without exact `team:temperpaw` tagging. Dashboard deploy
now supports `--reconcile`, updates desired dashboards by title, and deletes
owned stale dashboards by desired title, `team:temperpaw`, or legacy migration
terms.

Follow-up repo change: live monitor searches for `tag:team:temperpaw` returned
no data, so `scripts/deploy_monitors.py --reconcile` no longer relies only on
that tag. It now treats monitors as TemperPaw-owned when they match a desired
monitor name, carry `team:temperpaw`, or still contain legacy OpenPaw identity
in name/query/message/notifications. This prevents duplicate monitor creation
and gives the live `[OpenPaw]` monitors an explicit deletion path.

Datadog service search returned both live services:

- `openpaw`
- `temperpaw`

Metric search for `temperpaw` over 30 days returned no matching metric names.

Metric search for `openpaw` over 30 days returned:

- `openpaw.logs.warns`
- `openpaw.logs.errors`
- `openpaw.logs.wasm.default_timeout_fallback`

Refresh at `2026-05-11T16:49:19Z` returned the same metric inventory: no
`temperpaw` metric names and the three `openpaw.logs.*` metric names above.
Refresh at `2026-05-11T19:00:00Z` again returned no `temperpaw` metric names
over 30 days and the same three legacy metric names:
`openpaw.logs.warns`, `openpaw.logs.errors`, and
`openpaw.logs.wasm.default_timeout_fallback`.

Follow-up repo change: `scripts/deploy_pipelines.py --reconcile` now deletes
legacy `openpaw.*` log metrics after creating/updating the `temperpaw.*` log
metrics, so these live residual metrics have an explicit cleanup path once
Datadog API credentials are available.

## Live telemetry

APM aggregate over the last 24h:

- `service:temperpaw`: zero buckets.
- `service:openpaw`: active telemetry. Top examples:
  - `GET /odata/{path}`: 184,682 spans at the `2026-05-11T16:49:19Z` refresh.
  - `GET /tdata/WorkCycles('wc-{guid}')`: 36,861 spans.
  - `GET /tdata/WorkerRuns('en-{guid}')`: 25,639 spans.
  - `entity.get_or_spawn_tenant_actor_with_fields`: 14,488 spans.
  - `dispatch.phase.ask_reply`: 7,639 spans.
  - `dispatch.dispatch_tenant_action_core`: 7,134 spans.
- Refresh at `2026-05-11T19:00:00Z` over the last 24h returned one APM
  service bucket only: `service=openpaw` with 602,659 spans. No
  `service=temperpaw` APM span bucket was returned. Top resources remained
  legacy-service traffic, including `GET /odata/{path}` (176,722 spans),
  `entity.get_or_spawn_tenant_actor_with_fields` (35,934 spans), and
  `GET /tdata/WorkCycles('wc-{guid}')` (33,641 spans).

Logs over the last 24h:

- `service:temperpaw`: zero rows at the `2026-05-11T16:49:19Z` refresh.
- `service:openpaw`: 293,214 info logs and 6,585 warn logs.
- Refresh at `2026-05-11T19:00:00Z` over the last 24h returned only
  `service=openpaw` logs: 497,358 info, 21,920 warn, and 1 error. No
  `service=temperpaw` log row was returned.

LLM Observability over the last 24h:

- `ml_app:temperpaw`: no root spans.
- `ml_app:openpaw`: active spans.
- Representative span:
  - `trace_id`: `70504111318816966202061081941351717511`
  - `service`: `openpaw`
  - `ml_app`: `openpaw`
  - `name`: `wasm:provider_caller`
  - `span_kind`: `llm`
  - `parent_id`: `undefined`
  - `session_id`: `ss-019e17d3-87a7-7f60-af11-f52de6eb5395`
  - provider/model: `openai` / `gpt-5.5`

The corresponding LLMObs trace has:

- total spans: 1
- tree depth: 1
- root kind: `llm`
- no agent/workflow/tool hierarchy

This does not satisfy the required agent-session trace contract.

Refresh after the local span-hint patch at `2026-05-11T16:35:34Z`:

- `search_llmobs_spans(ml_app=temperpaw, span_kind=agent,
  root_spans_only=true, from=now-24h)` returned no spans.
- `search_llmobs_spans(ml_app=openpaw, root_spans_only=true,
  from=now-24h)` still returned recent root spans named
  `wasm:provider_caller`, `span_kind=llm`, `parent_id=undefined`,
  `service=openpaw`, with `session_id:*` tags. This confirms the local repo
  patch is not yet deployed and that live LLMObs remains under legacy identity
  with broken session hierarchy.

Refresh at `2026-05-11T16:49:19Z` again returned no `ml_app:temperpaw`
agent-root spans. It returned fresh `ml_app:openpaw` root `llm` spans named
`wasm:provider_caller`, all with `parent_id: undefined`; examples included APM
trace ids `157557853153833054266551756065080510738`,
`53312603372056228339610528250990586492`, and
`275791804991005940842527709706604262096`.
Refresh at `2026-05-11T19:00:00Z` again returned no
`ml_app:temperpaw`, `span_kind:agent`, root spans. The legacy `ml_app:openpaw`
search returned fresh root `llm` spans named `wasm:provider_caller`, with
`parent_id: undefined`, `service=openpaw`, `session_id:*`, and GPT-5.5
provider metadata. Representative trace
`197540154110098807124655545518596679071` still had total spans `1`,
tree depth `1`, root kind `llm`, and service `openpaw`.

`get_llmobs_trace(157557853153833054266551756065080510738)` showed:

- total spans: 1
- tree depth: 1
- root kind: `llm`
- root name: `wasm:provider_caller`
- services: `openpaw`
- no agent/workflow/tool hierarchy

APM trace lookup for a correlated trace id shows a useful but not yet clean hierarchy:

- Root span: `POST /tdata/CurationJobs(...)/Katagami.Curation.ConfigureAndSubmit`
- Root duration reported as about 335ms.
- Hidden child spans include background WASM work lasting about 30s, state transitions, OData calls, and `wasm_guest.log` events.
- Internal logs include useful attributes such as `entity_id`, `entity_type`, `tenant`, and `trigger_action`.
- Several GenAI fields are empty or missing in the APM side of the trace, and the LLMObs span remains a separate single-span root.

At the `2026-05-11T16:49:19Z` refresh, APM lookup for
`157557853153833054266551756065080510738` showed an `openpaw` root service
entry span for `POST /tdata/CurationJobs('en-{guid}')/Default.Submit`, about
107ms root duration, with 2,390 hidden child spans. The corresponding LLMObs
trace was still a separate one-span `llm` root.

## Temper platform trace-parenting inspection

Read-only inspection of `/Users/seshendranalla/Development/temper` explains why
TemperPaw span hints are not yet sufficient by themselves:

- `crates/temper-observe/src/wide_event.rs` builds transition `WideEvent`
  records with a synthetic `temper.trace_id` attribute, but `emit_span` starts a
  new OpenTelemetry span with `span_builder(...).start(&tracer)` and does not
  attach the event to an extracted/active parent context.
- `from_wasm_invocation`, `from_authz_decision`, and `from_invariant_check` set
  `trace_id: String::new()`, so those event families do not even carry the
  actor-local correlation id as an attribute.
- `crates/temper-server/src/entity_actor/actor.rs` uses one actor-local
  `self.trace_id` for all successful and failed transition events, not a
  Datadog/OTel trace id rooted at an agent session.
- `crates/temper-server/src/state/dispatch/wasm/invocation_artifacts.rs`
  records WASM invocations with no trace context parameter.
- `crates/temper-wasm/src/engine/host_functions.rs` passes guest headers
  directly into `host.http_call`; `crates/temper-wasm/src/host_trait.rs` sends
  them directly through `reqwest`. No `X-Temper-Span-*` hint extraction,
  `traceparent` injection, response-capture handling, or span parenting was
  observed on this branch.

Temper repo code edits were not made in this proof because its local
`AGENTS.md` requires PM issue pickup before code changes, the available
`TEMPER_API_KEY` did not authorize read-only issue listing (`HTTP 403
Forbidden` for tenant `rita-agents`), and a later Temper MCP
`temper.list_apps()` call failed with a session time-limit error. This remains a
platform-side blocker for final completion: Temper must consume TemperPaw's
span-hint headers and create/parent `temperpaw.agent.session`, `tool.*`, and
LLM `gen_ai.*` spans inside the same OTel trace before live Datadog can satisfy
the requested trace contract.

## DBM and profiling

- `find_datadog_database_instances` with `service:temperpaw`: no results.
- `find_datadog_database_instances` with `service:openpaw`: no results.
- DBM sample/plan searches for both service identities failed with `No valid indexes specified`; this is either a DBM indexing/configuration gap or a Datadog permission/product availability gap.
- Profiling upload metric queries for `datadog.profiling.rust.profiles_uploaded` and `datadog.profiling.rust.upload_errors` returned no data over 24h.
- Live monitor `[Temper] Profiler Uploads Stalled` is in Alert for `service:openpaw`.
- Refresh at `2026-05-11T16:49:19Z` still found no PostgreSQL DBM instances
  for either `service:temperpaw` or `service:openpaw`, and scalar profiling
  metric queries for uploads/errors returned no data for both service identities.
- Refresh at `2026-05-11T19:00:00Z` again found no PostgreSQL DBM instances
  for either `service:temperpaw` or `service:openpaw`. The Rust profiler upload
  and upload-error metric queries were syntactically valid but returned no data
  over the last 24h for both service identities.

## Deployment blocker

The repo has scripts for deploying dashboards and monitors:

- `scripts/deploy_dashboard.py`
- `scripts/deploy_monitors.py`
- `scripts/deploy_pipelines.py`

The local environment has `.env`, but it does not contain `DD_API_KEY`,
`DD_APP_KEY`, or `DD_SITE`, and those variables are not present in the shell
environment. The Datadog MCP tools available in this session can search and
validate, but no dashboard/monitor/pipeline upsert tool is exposed. Live
Datadog assets therefore remain unmodified during this proof.

The environment does expose a `TEMPER_API_KEY` variable name in `.env`, but a
read-only request to `https://api.temper.build/tdata/Issues` for tenant
`rita-agents` returned `HTTP 403 Forbidden` earlier in the investigation.
Later Temper MCP calls initially failed with a session time-limit error. At the
`2026-05-11T19:00:00Z` refresh, a pure `mcp__temper__.execute` snippet
(`return {"ok": True}`) succeeded, but the configured local target
`http://localhost:4445` was not running, and a real tenant discovery call
against `default` / `rita-agents` returned `HTTP 401 Unauthorized`. Temper repo
edits therefore remain blocked by the Temper repo's PM issue-pickup rule and
unavailable governance/auth tooling.

Refresh at `2026-05-11T20:23:50Z`: another no-op
`mcp__temper__.execute` call returned
`TimeoutError: time limit exceeded: 4812.381998834s > 1800s`, and
`curl http://127.0.0.1:4445/health` still could not connect. Temper platform
edits remain blocked by unavailable governed issue-pickup tooling.

## Current conclusion

As of the 2026-05-13T03:45Z refresh, the active deployed system emits under
`service:temperpaw` and `ml_app:temperpaw`, uses the deployed image
`ghcr.io/nerdsane/temperpaw:sha-afeca72`, and proves ADR-0084 in Datadog:
the direct Session trace is rooted at long-lived `Session.workflow`, not at a
short OData HTTP request. The same live session has chronological Temper
action, WASM, log-event, and Postgres spans; LLMObs has an agent -> workflow ->
LLM tree on the same trace id; WASM guest logs correlate by `session_id`,
`trace_id`, and `otel.trace_id`; DBM has full-mode samples with `traceparent`
and `trace.caller.*` on version `sha-afeca721`; profiling uploads are visible in
logs and metrics; dashboard/monitor/log-pipeline/log-metric assets are
reconciled; and `monitor_groups_search(group_status:alert (TemperPaw OR
Temper))` returned zero active alert groups.

The goal is not globally complete because external Railway resource names and
some storage/database URLs still carry the old `openpaw` identity, the Datadog
LLMObs agent-loop helper is empty for direct Session traces, and Datadog
facet/scanner application still requires UI proof on this account.

## Current completion audit

| Requirement | Current evidence | Status |
| --- | --- | --- |
| Active repo surfaces use TemperPaw identity | Current branch and Datadog assets use `service:temperpaw`, `team:temperpaw`, `ml_app:temperpaw`, and `@slack-temperpaw-alerts`; remaining legacy strings are historical proof/ADR text, allowlist/reconcile logic, or known external resource names | Mostly complete |
| Datadog assets define TemperPaw dashboards, monitors, facets, pipelines, sensitive-data scanner rules, DBM deploy wiring, agent diagnostic guidance, data/document service diagnostics, Modal/sandbox bridge diagnostics, channel transport diagnostics, webhook trigger diagnostics, governance approval diagnostics, and legacy cleanup paths | `datadog_observability_contract` passed with 20 tests; dashboard `mn4-k3k-i66`, monitors, log pipeline, and log metrics were reconciled live; facets/scanner definitions remain source-of-truth files with UI application still to prove | Strong, UI facet/scanner gap |
| Live Datadog assets are reconciled | `scripts/deploy_dashboard.py --reconcile`, `scripts/deploy_monitors.py --reconcile`, and `scripts/deploy_pipelines.py --reconcile` succeeded; old false-positive monitors were deleted/recreated/replaced; final alert-group search returned zero active alerts | Complete for API-managed assets |
| Actual live service emits under `service:temperpaw` | Deployed Railway image `sha-afeca72` emits APM/log/LLMObs/DBM/profiling telemetry under `service:temperpaw`, `env:prod`, and `version:sha-afeca721` | Live proven |
| One coherent chronological agent-session trace | Direct Session proof `ss-019e1f59-41b4-7993-870f-9bf9ac7e4a18` has APM trace `00795a1c90435bf41a99f0a051f9d729` rooted at `Session.workflow` with chronological Temper action/WASM/DB/log-event path and LLMObs trace `630095599782866875251990789384427305` with agent -> workflow -> LLM tree | Live proven for direct Session; LLMObs agent-loop helper still empty |
| LLM/tool span attributes are ready in TemperPaw | LLMObs span includes provider/model/token metadata; APM/logs include session/entity/action/module fields; guest logs carry trace/span correlation | Live proven for direct Session |
| Postgres DBM/APM correlation | DBM sample for `temperpaw-postgres` includes full-mode SQLCommenter `traceparent`, `trace.caller.service:temperpaw`, and `trace.caller.version:sha-afeca721`; propagated trace `e5139e30de2db2af1cb696ab7a25d899` opens to matching APM SQL spans; DBM activity monitor is OK | Live proven; the sampled DBM trace came from a read-only OData burst because DBM sampling is sparse |
| Profiling | On-demand 5s CPU profile returned 40,450 bytes and uploaded to Datadog Agent intake; `datadog.profiling.rust.profiles_uploaded{service:temperpaw,env:prod}` returned `version:sha-afeca721,profile_type:cpu`; upload-error metrics/logs did not show failures | Live proven for on-demand profiling |
| Temper repo implementation | Temper commit `974b13bf02342a1b8faafdb1b762572933fe1c3e` is pinned by TemperPaw source and live deployment, and includes LLMObs hierarchy, DBM attribution, pprof upload envelopes, WASM span hints, guest-log trace/span correlation, and ADR-0084 long-lived workflow root spans; Temper full pre-push gates passed before pinning | Implemented, source-pinned, and live-proven |
| End-to-end runtime verification | Docker image built and deployed to Railway; `/readyz` returned HTTP 200; live proof Session completed through real OData/LLM/DB/profiling paths; separate raw blob ingest returned HTTP 201 and retained APM trace `993e74a7129a8c286ce53d8c5b1e9f8a`; Datadog APM, logs, LLMObs, DBM, profiling, dashboard, and monitor status were queried directly | Live proven |
| Human/agent guide | `docs/temperpaw-datadog-observability-guide.md` teaches the shared query vocabulary, session trace workflow, agent query surface, logs, channel transports, webhooks, approvals, TemperFS/doc services, sandbox/Modal bridge, LLMObs, metrics/monitors, DBM, profiling, current verification state, and remaining gaps | Updated |
