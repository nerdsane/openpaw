# Native Skill Installation Proof — 2026-05-04

## Scope

Implemented `paw-skills` as a Temper-native app for approved skill package
installation. No taste or anti-slop skill body is bundled, seeded, or vendored
in this repository; the E2E proof fetched the external test skill live from:

`https://github.com/Leonxlnx/taste-skill/tree/main/skills/taste-skill`

## Static Verification

Passed:

- `cargo test -p temperpaw --test native_skill_installation`
- `cargo test` in `os-apps/paw-skills/wasm/skill_installer`
- `cargo check -p temperpaw`
- `bash os-apps/paw-skills/wasm/build.sh`

The first live boot exposed a missing CSDL surface: `paw-skills` IOA specs were
persisted, but `/tdata/SkillInstalls` was absent. Added
`os-apps/paw-skills/specs/model.csdl.xml` and a regression assertion for the
`SkillInstalls`, `SkillPackages`, and `SkillBindings` OData entity sets.

## Live E2E Environment

Started a fresh local TemperPaw instance:

- `HOME=/tmp/temperpaw-native-skill-e2e2`
- `PORT=4587`
- `PAW_TENANT=default`
- `TEMPER_API_KEY=e2e-key`
- `LLM_PROVIDER=mock`
- `LLM_MODEL=mock`
- `TEMPERPAW_WASM_STARTUP_POLICY=load-only`

Readiness:

- `GET /healthz` returned `200`
- `GET /readyz` returned `200`
- `/tdata` exposed `SkillInstalls`, `SkillPackages`, and `SkillBindings`

## External Skill Install

Katagami discovery:

- `katagami-curation` was installed as a startup app.
- The app bootstrapped `sl-bootstrap-agent-soul-curator`.
- Created an E2E Katagami Curator `Agent` using that discovered soul:
  `aj-019df4bc-c6dc-7541-a3d6-1c634fa4b8d1`.

Created `SkillInstall`:

- `SkillInstall`: `en-019df4bc-fcbb-70a0-9262-0273be0890f8`
- Source: `https://github.com/Leonxlnx/taste-skill/tree/main/skills/taste-skill`
- Target scope: `agent`
- Target id: `aj-019df4bc-c6dc-7541-a3d6-1c634fa4b8d1`

Approved with:

`POST /tdata/SkillInstalls('en-019df4bc-fcbb-70a0-9262-0273be0890f8')/Paw.Skills.Approve?await_integration=true`

Observed final install state:

- `SkillInstall.Status`: `Installed`
- Installed skill name: `design-taste-frontend`
- Runtime path:
  `/agents/aj-019df4bc-c6dc-7541-a3d6-1c634fa4b8d1/skills/design-taste-frontend/SKILL.md`
- Digest:
  `sha256:a23a9e4e74a87e3f458d53cbff60adbc666e7a1e8633d43f9e213111e48f69b0`
- Runtime file id: `fl-019df4bd-021f-73f3-920e-8315db33f1a4`
- Content bytes fetched into TemperFS: `21140`

Provenance and binding:

- `SkillPackage`: `en-019df4bd-026f-70b1-89ee-c00fbc0a9c77`
- `SkillPackage.Status`: `Available`
- Main file path:
  `https://raw.githubusercontent.com/Leonxlnx/taste-skill/main/skills/taste-skill/SKILL.md`
- `SkillBinding`: `en-019df4bd-027e-70c3-b11a-45ec5bd96796`
- `SkillBinding.Status`: `Active`
- `File.Status`: `Ready`

The proof checked only frontmatter markers and byte count. It did not embed the
external skill body in this report.

## Katagami Context Verification

Started mock-provider session:

- `Session`: `ss-019df4bd-ed29-7941-9c60-866f7370af4d`
- `Agent`: `aj-019df4bc-c6dc-7541-a3d6-1c634fa4b8d1`
- `Soul`: `sl-bootstrap-agent-soul-curator`
- Final status: `Completed`
- `prepared_context_bytes`: `4377`

The prepared context included this skill index entry:

```xml
<available_skills mode="index">
  <skill name="design-taste-frontend" path="/agents/aj-019df4bc-c6dc-7541-a3d6-1c634fa4b8d1/skills/design-taste-frontend/SKILL.md" workspace_id="os-app-docs" />
</available_skills>
```

This proves Katagami consumed the installed package through the existing
path-scoped TemperFS skill discovery mechanism. The provider was `mock`, so the
session verifies context inclusion without spending tokens or depending on a
live LLM critique.
