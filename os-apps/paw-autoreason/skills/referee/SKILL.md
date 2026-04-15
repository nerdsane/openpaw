# Referee

How to run a tournament round: spawn critic, author, synthesizer, judges in sequence. Enforce context firewalls by controlling what each sub-agent receives.

## Round Protocol

### Step 1: Critique
- Read Version A from paw-fs
- Spawn a critic session with ONLY Version A
- Wait for critique output (stored in paw-fs)
- Save critique_file_id on the Round

### Step 2: Author Revision (Version B)
- Spawn an author session with Version A + critique
- Author produces Version B
- Create a Version entity (label=B), write to paw-fs
- Save version_b_file_id on the Round
- **Context firewall**: author does NOT see previous rounds

### Step 3: Synthesis (Version AB)
- Spawn a synthesizer session with Version A + Version B
- Synthesizer merges both into Version AB
- Create a Version entity (label=AB), write to paw-fs
- Save version_ab_file_id on the Round
- **Context firewall**: synthesizer does NOT see the critique

### Step 4: Judging
- Generate randomized label mappings for each judge (e.g., judge 1 sees A->X, B->Y, AB->Z; judge 2 sees A->Z, B->X, AB->Y)
- Each judge gets a DIFFERENT random mapping
- Spawn N judge sessions, each with all three versions under their randomized labels
- Create Judgment entities with the label mappings
- **Context firewall**: judges cannot see each other

### Step 5: Completion
- Wait for all judges to submit (poll Judgment entities)
- When all submitted, dispatch `AllJudged` on the Tournament
- The tally_votes WASM handles the rest (Borda count, convergence)

## Randomized Label Generation

For each judge, create a random permutation mapping real labels to randomized labels:
- Use letters like X, Y, Z (or numbers 1, 2, 3) — anything that doesn't hint at A/B/AB
- Each judge MUST get a different mapping to prevent positional bias
- Store the mapping in the Judgment entity for later decoding
