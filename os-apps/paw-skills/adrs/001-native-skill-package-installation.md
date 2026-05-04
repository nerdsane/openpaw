# ADR-001: Native Skill Package Installation

## Status

Accepted

## Context

TemperPaw agents already consume skills from TemperFS path scopes. Production hot changes showed that agents can gain useful taste and anti-slop behavior from external skill packages, but installing those packages by editing app files directly loses provenance and is overwritten by redeploys.

## Decision

Add `paw-skills`, a Temper app that models native skill installation with `SkillInstall`, `SkillPackage`, and `SkillBinding` entities. A human or supervisor approves a `SkillInstall`; the `skill_installer` WASM integration fetches the external skill source, records package provenance, and writes the runtime `SKILL.md` copy to the existing TemperFS scope path.

The runtime contract remains file-backed:

- `/system/skills/{name}/SKILL.md`
- `/projects/{project-id}/skills/{name}/SKILL.md`
- `/agents/{agent-id}/skills/{name}/SKILL.md`

No bundled taste or anti-slop skill content is committed. E2E testing may install an external skill source such as `https://github.com/Leonxlnx/taste-skill/tree/main/skills/taste-skill`, but that URL is test provenance only and the source body is not vendored into this app.

## Consequences

Agents can request installable skills without bypassing Temper primitives. Reviewers can audit who requested a skill, where it came from, what digest was installed, and which runtime scope can see it.

Redeploys no longer erase approved runtime skill installs as long as the backing TemperFS and entity storage persist. If a deployment resets storage, the package provenance and bindings show what needs reinstalling.
