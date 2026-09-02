# ARN-456 decisions

**Decision:** One Ask type, three kinds (decide / do / fyi). Not three entities.
**Came up because:** Rita wanted an inbox for decide, do, and agent-only FYI.
**Options:** (1) three entities; (2) Effort fields only; (3) one Ask.
**Chose (3) because:** decide and fyi are the same noun; do is the same inbox. Three types would duplicate Stall and decisions.md.
**Where:** ask.ioa.toml.

---

**Decision:** Raise does not Stall. Answer does not Resume. The agent Stalls when idle. Resume names the stall Ask.
**Came up because:** A partial block must not freeze the Effort. Entity triggers cannot be conditional.
**Options:** (1) trigger Stall on every Raise; (2) agent sets Stall/Resume.
**Chose (2) because:** (1) would Stall on the first open do.
**Where:** effort.ioa.toml Stall/Resume.

---

**Decision:** Delete Adjudication. Resume is answering the stall Ask.
**Came up because:** Adjudication was the S0 name for an owner ruling. Ask.Answer is that fact.
**Options:** (1) keep both; (2) delete Adjudication.
**Chose (2) because:** two ruling types is the mess we were leaving.
**Where:** removed adjudication.ioa.toml.

---

**Decision:** Fix-it rubrics fail review. Risk rubrics only choose auto-merge vs a do Ask.
**Came up because:** Unused code and intent drift are not human merge questions.
**Options:** (1) all rubrics feed human-merge; (2) split fix-it vs risk.
**Chose (2) because:** a PR that failed fix-it is not mergeable.
**Where:** stack REVIEW.md; rubrics.py.

---

**Decision:** Cedar is a project risk flag, not a global one.
**Came up because:** Cedar is TemperPaw.
**Options:** (1) global REVIEW.md; (2) repo REVIEW.md.
**Chose (2).
**Where:** os-apps is not the home; temperpaw REVIEW.md.

---

**Decision:** The stuck-loop prompt is ASSESS-REVIEW-SPIRAL.md. Always an agent. Harness list in panel.json, Fable 5.1 first.
**Came up because:** "Arbiter" hid the job. File-count fallback was a no-agent path.
**Where:** stack ASSESS-REVIEW-SPIRAL.md; review/panel.json.

---

**Decision:** `chain_github_ready` probes `GET /repos/{owner}/{name}` before contents, and the temper-agent skill tells the implementer to re-get the row after Attach*.
**Came up because:** ARN-455 Intent `intent-arn-455-aya-ui-redesign` stayed Triaged. The file exists on `arni-labs/aya@claude/aya-redesign` (rita-aga can read it). Production retracted with `is not on GitHub` because GitHub returns 404 when the tenant `github_token` cannot see a private repo. Claude Code treated AttachIntentFile success as the door.
**Options:** (1) leave the 404 wording and tell agents in prose; (2) distinguish visibility vs missing path, and write the retract/re-get rule in stack `temper-agent`.
**Chose (2) because:** (1) keeps the next agent diagnosing a missing file. The credential fix (grant production `github_token` access to `arni-labs/aya`) is still required for Accept to pass.
**Where:** `os-apps/paw-patrol/wasm/chain_github_ready/src/lib.rs`; stack `skills/temper-agent/SKILL.md`.
