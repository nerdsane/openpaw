# TemperPaw Legacy Identity Allowlist

Status: Active allowlist for external resources only.
Last verified: 2026-05-13.

This file is the reviewed allowlist for legacy-named external resources that
remain in use after the TemperPaw identity cleanup. These names are accepted
only because renaming them requires an external resource cutover, DNS/storage
migration, or Railway service migration outside the code-only path.

Observed product identity must remain `service:temperpaw` in Datadog, runtime
logs, APM spans, DBM caller metadata, LLMObs spans, dashboards, monitors,
pipelines, scripts, Docker image metadata, and active repository docs.

Do not create new resources with legacy names.

## Allowlisted Resources

Railway project slug: `openpaw-seshendranalla`

Reason: Railway-created project identity predates the TemperPaw consolidation.
The running application exports `service:temperpaw`, `team:temperpaw`, and
`env:prod`; this external project slug is not used as the Datadog service
identity.

Migration requires a planned cutover window because project-level renaming can
affect operator links, deployment history, and any Railway-integrated resource
references.

Railway service name: `openpaw`

Reason: The existing production Railway service hosts the current TemperPaw
runtime. The service's exported telemetry and image metadata use TemperPaw
identity even though the external service object still has its older name.

Migration requires a planned cutover window because service renaming can affect
CLI commands, deployment scripts, generated domains, and Railway references.

Railway generated domain: `openpaw-production.up.railway.app`

Reason: Railway generated this public domain from the older service/project
names. It remains useful for health checks while the custom domain is being
stabilized.

Migration requires a planned cutover window because callers and proof scripts
must move to a stable TemperPaw domain before the generated domain can be
retired.

R2 bucket: `openpaw-fs-seshendranalla`

Reason: This bucket contains production blob/document content and is referenced
by the live TemperFS/blob path. Renaming or replacing it requires a storage
migration and validation that existing content hashes and file streams remain
readable.

Migration requires a planned cutover window because the bucket contains live
content, and the migration must prove old and new blob paths, R2 credentials,
and Datadog trace/log correlation before the older bucket can be retired.

## Exit Criteria

- The replacement Railway project/service/domain or storage bucket exists and
  is tagged and documented with TemperPaw identity.
- Production deployment, readiness checks, Discord connectivity, file/blob
  reads, file/blob writes, agent sessions, and Datadog telemetry are verified
  against the replacement resource.
- Datadog queries for product/service identity continue to return only
  `service:temperpaw` and `team:temperpaw` for active telemetry.
- The retired resource is deleted or removed from all runtime configuration.
- This allowlist entry is deleted in the same change that proves the migration.
