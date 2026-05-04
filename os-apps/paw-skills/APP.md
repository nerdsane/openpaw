# Paw Skills

Paw Skills installs external `SKILL.md` packages into TemperPaw's existing
file-backed skill scopes. Agents request a `SkillInstall`, a human or
supervisor approves it, and the `skill_installer` WASM integration writes the
runtime skill file into TemperFS while recording `SkillPackage` provenance and
`SkillBinding` scope state.

This app does not include bundled taste or anti-slop skill content. External
skill bodies are fetched at install time and tracked by source URL and digest.
