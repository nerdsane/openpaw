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

**Decision:** `chain_github_ready` fails if tenant `github_token` is missing, then probes the repo before contents. temper-agent does not teach the SDLC or this door.
**Came up because:** Rita asked what I had encoded in temper-agent, and whether the file-ready miss was a missing file or a missing Temper token. Production vault has no `github_token`. Railway `openpaw` has no `GITHUB_TOKEN`. The check was anonymous GitHub. `arni-labs/aya` is private (404). `nerdsane/temperpaw` is public (works). The agent's `gh` is a different token.
**Options:** (1) teach the door in temper-agent; (2) fail with an honest WASM error and put a token in the Temper vault.
**Chose (2) because:** temper-agent is how to use Temper. The factory order stays in AGENTS.md. Agents stumble when the error lies.
**Where:** `os-apps/paw-patrol/wasm/chain_github_ready/src/lib.rs`; stack `skills/temper-agent/SKILL.md`.
