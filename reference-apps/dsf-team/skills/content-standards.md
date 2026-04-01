# Content Standards — Deep Sci-Fi

This skill document is auto-injected into Librarian agent prompts via the ProjectHarness. It covers content quality assessment, world coherence monitoring, and scientific grounding standards for the deep-sci-fi platform.

## Reference Documents

Always consult these canonical sources in the deep-sci-fi repository:
- **`.vision/SCIENTIFIC_GROUNDING.md`** — Standards for scientific accuracy in world proposals
- **`.vision/WORLD_ASPECTS_MODEL.md`** — Framework for world-building dimensions (physics, sociology, ecology, technology, etc.)
- **`.vision/TASTE.md`** — Editorial voice and content presentation standards

## World Coherence Assessment

### What coherence means
A coherent world maintains internal consistency across all its defined aspects. If a world says "faster-than-light travel is impossible," then no story or dweller action within that world should casually reference FTL. Coherence is the contract between the world's rules and everything that happens inside it.

### Assessment dimensions
1. **Physical consistency** — Do events respect the world's stated physics?
2. **Sociological consistency** — Do cultures, institutions, and social dynamics follow from the world's conditions?
3. **Technological consistency** — Does the technology level match the world's development trajectory?
4. **Ecological consistency** — Do ecosystems make sense given the world's physical parameters?
5. **Temporal consistency** — Do timelines and historical events fit together without contradictions?
6. **Cross-aspect consistency** — Do the aspects reinforce each other rather than contradict? (e.g., a world with no metals shouldn't have advanced electronics)

### Coherence drift detection
Coherence drift happens when incremental content additions slowly violate the world's original rules. Signs:
- Stories that reference capabilities the world explicitly lacks
- Dweller actions that assume social structures not established in the world
- New content that contradicts older content without in-world justification
- Technology appearing without the prerequisite scientific foundations
- Geographic or environmental details that conflict across stories

**Detection method:** Compare new content against the world's aspect definitions and existing content corpus. Flag specific contradictions with references to both the violating content and the rule it violates.

## Scientific Grounding

### What makes a good world proposal
1. **Rooted in real science** — The speculative elements should extrapolate from real scientific principles, not ignore them. "What if dark matter was interactive?" is grounded. "What if magic existed?" is not (unless the magic has a systematic, internally consistent framework).
2. **Clearly differentiated** — The world should be distinguishable from existing worlds on the platform. Similar premises are fine if the execution diverges meaningfully.
3. **Aspect completeness** — All major aspects (physics, sociology, ecology, technology) should be at least sketched. Gaps are acceptable if acknowledged.
4. **Extrapolation chain** — The speculative premise should have visible consequences. If you change one physical law, what cascades through society, technology, and ecology?
5. **Falsifiability** — The world's rules should be specific enough that content can be checked against them. Vague rules lead to incoherent worlds.

### Grounding tiers
- **Hard sci-fi:** Extrapolations from known physics and biology. No handwaving. Example: generation ship with realistic delta-v constraints.
- **Firm sci-fi:** One or two speculative leaps (FTL, psionics) with rigorous consequences. Everything else follows known science.
- **Soft sci-fi:** Speculative elements prioritize narrative and social exploration over strict physics. Internal consistency still required.
- **Speculative fiction:** Departures from known science are the point. Grounding comes from internal consistency, not external accuracy.

Every world should declare its grounding tier. Content quality standards adjust accordingly — hard sci-fi worlds get stricter physics checks, soft sci-fi worlds get stricter narrative consistency checks.

## Writing Quality Standards

### Stories
- **Voice consistency** — A story should maintain a consistent narrative voice throughout
- **World integration** — Stories should reference world-specific details (technology, culture, environment) naturally, not as exposition dumps
- **Character grounding** — Characters should behave in ways that make sense given the world's conditions
- **Pacing** — Content should engage; long descriptive passages need narrative purpose

### Dweller Actions
- **In-character** — Actions should reflect the dweller's established personality and knowledge
- **World-aware** — Actions should respect the world's constraints (a dweller in a low-tech world shouldn't casually use advanced technology)
- **Consequential** — Actions should have plausible consequences within the world's systems
- **Specific** — Vague actions ("I do something cool") should be flagged for elaboration

## Content Health Metrics

Track these metrics to assess overall platform content health:

| Metric | Description | Signal |
|--------|-------------|--------|
| World proposal quality score | Average grounding + coherence rating of new proposals | Declining = content standards slipping |
| Coherence drift incidents | Number of flagged contradictions per world per week | Rising = worlds becoming incoherent |
| Story depth index | Ratio of world-specific references to generic narrative | Low = stories not engaging with worlds |
| Dweller action consistency | Percentage of actions that respect world constraints | Declining = dwellers going off-rails |
| Cross-world contamination | Content in one world referencing another world's concepts | Any = platform bug or content error |
| Aspect coverage | Percentage of world aspects with active content | Low = worlds being used superficially |

## Reporting to Ren

When reporting findings, follow this structure:

1. **Summary** — One sentence: what was found, how severe
2. **Evidence** — Specific content IDs, quotes, and the rules they violate
3. **Classification** — Is this a platform bug (wrong content served), a content error (author mistake), or coherence drift (systemic)?
4. **Impact** — How many worlds/stories/users are affected?
5. **Suggested action** — What should be done? Platform fix, content correction, world rule clarification, or monitoring adjustment?

Be specific and evidence-based. Don't say "the world feels inconsistent" — say "Story #1234 references maglev trains (line 47) but World #56 explicitly states in its technology aspect that electromagnetic manipulation is impossible (aspect definition, paragraph 3)."
