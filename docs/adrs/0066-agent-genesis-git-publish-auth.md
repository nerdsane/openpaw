# ADR-0066: Agent Genesis Git Publish Auth

- Status: Proposed
- Date: 2026-06-18
- Deciders: TemperPaw maintainers

## Context

TemperPaw agents repair installed apps by editing a workspace copy, calling
`temper.publish_app(...)` or `temper.update_app(...)`, then installing the
returned pinned Genesis ref. The registry-side OData actions can be reached with
the existing agent headers, but Genesis smart HTTP git receive-pack rejects
anonymous pushes.

The previous publish path configured only `X-Tenant-Id` as a git extraHeader.
When Genesis required authentication, agents saw interactive git credential
failures such as "could not read Username" instead of a deterministic tool error.
That made app repairs easy to leave as local-only edits.

Genesis already accepts active GitToken secrets through `Authorization: Bearer`
on git smart HTTP requests.

## Decision

`temper.publish_app(...)` and `temper.update_app(...)` will resolve a Genesis
GitToken from a Temper secret and send it to git as an extraHeader:

- default secret names, tried in order: `GENESIS_GIT_TOKEN`, then the existing
  production-compatible `GENESIS_TOKEN`
- per-call overrides: `registry_token_secret`, `git_token_secret`,
  `genesis_git_token_secret`, `genesis_token_secret`, or `RegistryTokenSecret`
- agent config overrides: `genesis_registry_token_secret`,
  `GENESIS_REGISTRY_TOKEN_SECRET`, `genesis_git_token_secret`, or
  `GENESIS_GIT_TOKEN_SECRET`, `genesis_token_secret`, or `GENESIS_TOKEN_SECRET`

The git command also sets `GIT_TERMINAL_PROMPT=0` so missing or invalid auth
fails immediately with explicit evidence.

The token is not returned in tool results. Successful publish/update results
continue to return the pinned `owner/name@hash` ref and verified Genesis latest
hash metadata.

## Consequences

- Agent app repairs can push to Genesis without human credential prompts.
- Missing publish credentials fail before a local-only change can be mistaken
  for an installed repair.
- Deployments must provision an active Genesis GitToken in the configured Temper
  secret before agents can publish or update apps.
- The existing install step remains mandatory: pushing a new Genesis version is
  not the same thing as installing that pinned ref into a tenant.

## Non-Goals

- This ADR does not change Genesis GitToken validation semantics.
- This ADR does not add password-based git credentials.
- This ADR does not make unpinned app installs acceptable for agents.
