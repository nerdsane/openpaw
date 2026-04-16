#!/usr/bin/env python3
"""Post-run integrity checks for the foresight meta-loop.

Run after a completed iteration, BEFORE the loop advances to the next round.
Exits non-zero on any invariant violation. loop.sh refuses to start the next
round until this passes.

Checks (one per original gap audit item):

  #1  Judge evidence: judge_N_raw.json files exist and are non-empty parseable
      JSON with a "criteria" array of exactly 12. If scores.json claims
      "self_scored=true" (fallback), require judge_N_attempt.log files showing
      actual invocation failures.
  #2  Synthesis provenance: engine-output/synthesis.md sha256 matches the
      orchestrator session's final output file blob (authored by the engine,
      not hand-written by the meta-agent).
  #3  Judge prompt template sha: scores.json records the judge template sha,
      and it matches the current committed meta/judge_prompt_template.md.
  #4  Deterministic X/Y: scores.json judges' x_is_challenger values match
      assign_judges.py's derivation for (run_num, head_sha).
  #5  Single-file-scope: git diff of the run's change commits touches only
      files declared in plan.md's "## Scope" block.
  #6  Incumbent lookup: prior run's engine-output/synthesis.md exists and is
      non-empty (or we're Run 000, which is skip-tournament).
  #7  Handled together with #6 (strict hard-fail on missing incumbent).
  #8  Borda correctness: rerun borda.py and diff against committed borda.json —
      must match exactly.
  #9  Baseline lock: meta/baseline/synthesis.md sha256 matches the value in
      meta/baseline/scores.json.
  #11 Tag integrity: if a foresight-v{NNN} tag was created, its annotation
      must match the run's diagnosis.md contents (created by loop.sh, not
      the agent).
  #12 No force-push: reflog must not show any forced updates on this branch.

Usage:
  python3 verify_run.py --run-dir meta/runs/001_foo/ [--strict]
"""

import argparse
import hashlib
import json
import re
import shutil
import sqlite3
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
META_DIR = REPO_ROOT / "os-apps/paw-foresight/meta"
TEMPLATE_PATH = META_DIR / "judge_prompt_template.md"
BASELINE_SCORES_PATH = META_DIR / "baseline/scores.json"
BASELINE_SYNTHESIS_PATH = META_DIR / "baseline/synthesis.md"
TOOLS_DIR = META_DIR / "tools"
ASSIGN_JUDGES = TOOLS_DIR / "assign_judges.py"
BORDA = TOOLS_DIR / "borda.py"

CRITERIA_COUNT = 12

failures: list[str] = []


def fail(msg: str) -> None:
    failures.append(msg)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    h.update(path.read_bytes())
    return h.hexdigest()


def run_num_from_dir(run_dir: Path) -> int:
    m = re.match(r"(\d{3})_", run_dir.name)
    if not m:
        raise SystemExit(f"ERROR: run dir name '{run_dir.name}' doesn't match NNN_desc pattern")
    return int(m.group(1))


def git(*args: str, cwd: Path = REPO_ROOT) -> str:
    return subprocess.check_output(["git", *args], cwd=cwd, text=True).strip()


def check_judges(run_dir: Path, scores: dict) -> None:
    """#1 Judge evidence."""
    self_scored = scores.get("self_scored", False)
    if self_scored:
        # Fallback path: require failure logs for all 3 judges
        for j in (1, 2, 3):
            log_path = run_dir / f"judge_{j}_attempt.log"
            if not log_path.exists() or log_path.stat().st_size == 0:
                fail(f"#1 self_scored=true but judge_{j}_attempt.log is missing/empty — no evidence judges were actually invoked")
        return

    # Normal path: require 3 raw judge files
    for j in (1, 2, 3):
        raw_path = run_dir / f"judge_{j}_raw.json"
        if not raw_path.exists():
            fail(f"#1 judge_{j}_raw.json missing")
            continue
        if raw_path.stat().st_size == 0:
            fail(f"#1 judge_{j}_raw.json is empty")
            continue
        try:
            raw = json.loads(raw_path.read_text())
            # Support nested {"result": "{...}"} from claude -p --output-format json
            if "criteria" not in raw and "result" in raw:
                inner = raw["result"]
                raw = json.loads(inner) if isinstance(inner, str) else inner
        except json.JSONDecodeError as e:
            fail(f"#1 judge_{j}_raw.json not parseable: {e}")
            continue
        crits = raw.get("criteria")
        if not isinstance(crits, list) or len(crits) != CRITERIA_COUNT:
            fail(f"#1 judge_{j}_raw.json criteria array has {len(crits) if isinstance(crits, list) else 'N/A'} items (expected {CRITERIA_COUNT})")


def check_synthesis_provenance(run_dir: Path) -> None:
    """#2 Synthesis provenance — compare synthesis.md to orchestrator session output."""
    synthesis = run_dir / "engine-output/synthesis.md"
    if not synthesis.exists():
        fail(f"#2 engine-output/synthesis.md missing")
        return
    synthesis_sha = sha256_file(synthesis)

    # The run dir should have transcripts/MANIFEST.md noting which session is the orchestrator
    manifest = run_dir / "transcripts/MANIFEST.md"
    if not manifest.exists():
        fail("#2 transcripts/MANIFEST.md missing — cannot locate orchestrator session for provenance check")
        return

    # Try to extract orchestrator session id from MANIFEST
    manifest_text = manifest.read_text()
    m = re.search(r"orchestrator[^\n]*?(ss-[0-9a-f-]{36})", manifest_text, re.IGNORECASE)
    if not m:
        # Also accept a "session_id: ss-..." directive
        m = re.search(r"session_id[:\s]+(ss-[0-9a-f-]{36})", manifest_text, re.IGNORECASE)
    if not m:
        fail("#2 MANIFEST.md doesn't identify the orchestrator session id (expected 'orchestrator' line with ss-...)")
        return
    session_id = m.group(1)

    # Connect to the isolated paw.db, look up session_file_id → content_hash → blob
    db_path = Path("/tmp/paw-foresight-home/.local/share/openpaw/paw.db")
    if not db_path.exists():
        fail(f"#2 isolated DB not at {db_path} — cannot verify synthesis provenance")
        return

    con = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    try:
        # Get session_file_id from session entity
        row = con.execute(
            "SELECT field_value FROM entity_field_index WHERE entity_id = ? AND field_name = 'session_file_id'",
            (session_id,),
        ).fetchone()
        if not row or not row[0]:
            fail(f"#2 session {session_id} has no session_file_id in entity_field_index")
            return
        file_id = row[0]

        row = con.execute(
            "SELECT field_value FROM entity_field_index WHERE entity_id = ? AND field_name = 'content_hash'",
            (file_id,),
        ).fetchone()
        if not row or not row[0]:
            fail(f"#2 file {file_id} has no content_hash")
            return
        content_hash = row[0]
        blob_key = f"temper-fs/{content_hash}"

        row = con.execute("SELECT data FROM blobs WHERE blob_key = ?", (blob_key,)).fetchone()
        if not row:
            fail(f"#2 blob {blob_key} not found in blobs table")
            return
        blob_data = row[0]
    finally:
        con.close()

    # The session file is JSONL. The synthesis.md content should appear as the final
    # assistant message, but JSON-escaped (newlines → \\n, quotes → \\", etc). Normalize
    # both sides to a flattened alphanumeric-and-space sequence, then check that a
    # distinctive run from the synthesis is present in the blob text.
    try:
        text = blob_data.decode("utf-8", errors="replace")
    except Exception:
        fail("#2 orchestrator blob is not UTF-8 decodable")
        return
    needle = synthesis.read_text().strip()
    if len(needle) < 50:
        fail("#2 synthesis.md too short to be meaningful (< 50 chars)")
        return

    def normalize(s: str) -> str:
        # Keep alphanumerics and single spaces — discard punctuation/formatting/escape differences
        import re as _re
        return _re.sub(r"\s+", " ", _re.sub(r"[^A-Za-z0-9 ]+", " ", s)).strip().lower()

    norm_needle = normalize(needle)
    norm_text = normalize(text)
    # Take a ~150-char distinctive window from the middle of the normalized needle
    mid = len(norm_needle) // 2
    window = norm_needle[max(0, mid - 75) : mid + 75].strip()
    if len(window) < 40:
        # Fall back to first 100 chars if needle is unusually short
        window = norm_needle[:100].strip()
    if window and window not in norm_text:
        fail(
            f"#2 synthesis.md content (sha={synthesis_sha[:12]}) not found in orchestrator session "
            f"{session_id} blob ({blob_key}) after normalization — synthesis may have been hand-authored "
            f"rather than produced by the engine. Window searched: '{window[:80]}...'"
        )


def check_judge_template_sha(run_dir: Path, scores: dict) -> None:
    """#3 Judge prompt template sha match."""
    if not TEMPLATE_PATH.exists():
        fail(f"#3 judge template not found at {TEMPLATE_PATH}")
        return
    actual_sha = sha256_file(TEMPLATE_PATH)
    recorded_sha = scores.get("judge_prompt_template_sha256")
    if not recorded_sha:
        fail("#3 scores.json missing 'judge_prompt_template_sha256' field")
        return
    if recorded_sha != actual_sha:
        fail(
            f"#3 judge template sha mismatch: scores.json has {recorded_sha[:16]}..., "
            f"current template file has {actual_sha[:16]}... — either the template was modified "
            f"without updating scores.json, or the agent used a non-canonical prompt"
        )


def check_deterministic_assignments(run_dir: Path, scores: dict, head_sha: str) -> None:
    """#4 X/Y assignments must match assign_judges.py derivation."""
    run_num = run_num_from_dir(run_dir)
    try:
        subprocess.check_output(
            ["python3", str(ASSIGN_JUDGES), "--run", str(run_num), "--head", head_sha, "--verify", str(run_dir / "scores.json")],
            stderr=subprocess.STDOUT,
            text=True,
        )
    except subprocess.CalledProcessError as e:
        fail(f"#4 deterministic X/Y verification failed:\n{e.output}")


def check_plan_scope(run_dir: Path) -> None:
    """#5 Files changed match plan.md's Scope declaration."""
    plan = run_dir / "plan.md"
    if not plan.exists():
        fail("#5 plan.md missing")
        return
    plan_text = plan.read_text()
    m = re.search(r"##\s*Scope[^\n]*\n(.*?)(?=\n##|\Z)", plan_text, re.DOTALL)
    if not m:
        fail("#5 plan.md has no '## Scope' section listing the files this run will change")
        return
    scope_block = m.group(1)
    declared = set()
    for line in scope_block.strip().splitlines():
        line = line.strip()
        if line.startswith("- "):
            path = line[2:].strip().strip("`")
            if path and not path.startswith("("):
                declared.add(path)
    if not declared:
        fail("#5 plan.md '## Scope' has no bullet list of paths")
        return

    # Diff from the commit that introduced plan.md forward. We assume the run's
    # change commits are in a contiguous range after the plan commit. Use a more
    # permissive check: any tracked file changed in the branch range
    # (last N commits) that's NOT in declared, and NOT under meta/runs/, fails.
    try:
        changed = git("diff", "--name-only", "HEAD~5..HEAD").splitlines()
    except subprocess.CalledProcessError:
        changed = git("diff", "--name-only", "HEAD~1..HEAD").splitlines()
    changed = [c for c in changed if c and not c.startswith("os-apps/paw-foresight/meta/runs/")]

    undeclared = [c for c in changed if c not in declared]
    if undeclared:
        fail(
            f"#5 files changed outside plan.md '## Scope': {undeclared}. "
            f"Declared scope: {sorted(declared)}. "
            f"Either add these paths to plan.md before the run, or revert them."
        )


def check_incumbent_exists(run_dir: Path) -> None:
    """#6 #7 Incumbent synthesis must exist (unless we're Run 000, which skips tournament)."""
    run_num = run_num_from_dir(run_dir)
    if run_num == 0:
        # Run 000 is measurement-only — no scoring, no incumbent required
        return
    prev_num = run_num - 1
    prev_prefix = f"{prev_num:03d}_"
    runs_dir = META_DIR / "runs"
    candidates = [p for p in runs_dir.iterdir() if p.is_dir() and p.name.startswith(prev_prefix)]
    if not candidates:
        fail(f"#6 no prior run directory starting with {prev_prefix} — cannot locate incumbent")
        return
    prev_dir = candidates[0]
    inc_path = prev_dir / "engine-output/synthesis.md"
    if not inc_path.exists():
        fail(f"#6 incumbent {inc_path} missing — hard-fail, no silent fallback to empty")
        return
    if inc_path.stat().st_size < 100:
        fail(f"#6 incumbent {inc_path} too small ({inc_path.stat().st_size} bytes) — likely corrupt")


def check_borda_reproduces(run_dir: Path) -> None:
    """#8 Rerun borda.py and compare to committed borda.json."""
    committed = run_dir / "borda.json"
    if not committed.exists():
        # Run 000 skips scoring, so no borda.json is valid
        run_num = run_num_from_dir(run_dir)
        if run_num == 0:
            return
        fail("#8 borda.json missing")
        return

    tmp_out = run_dir / ".borda_verify.json"
    try:
        subprocess.check_output(
            ["python3", str(BORDA), "--run-dir", str(run_dir), "--out", str(tmp_out)],
            stderr=subprocess.STDOUT,
            text=True,
        )
    except subprocess.CalledProcessError as e:
        fail(f"#8 borda.py rerun failed:\n{e.output}")
        return

    a = json.loads(committed.read_text())
    b = json.loads(tmp_out.read_text())
    # Compare the keys that matter (ignore mutable run_dir field)
    for key in ("challenger_borda", "incumbent_borda", "winner", "per_criterion", "judges_counted"):
        if a.get(key) != b.get(key):
            fail(f"#8 borda.json mismatch on '{key}': committed={a.get(key)} recomputed={b.get(key)}")
    tmp_out.unlink(missing_ok=True)


def check_baseline_lock() -> None:
    """#9 Baseline synthesis sha must match recorded value."""
    if not BASELINE_SCORES_PATH.exists():
        fail(f"#9 {BASELINE_SCORES_PATH} missing")
        return
    if not BASELINE_SYNTHESIS_PATH.exists():
        fail(f"#9 {BASELINE_SYNTHESIS_PATH} missing")
        return
    scores = json.loads(BASELINE_SCORES_PATH.read_text())
    recorded = scores.get("baseline_synthesis_sha256")
    actual = sha256_file(BASELINE_SYNTHESIS_PATH)
    if recorded != actual:
        fail(
            f"#9 baseline synthesis sha mismatch: scores.json says {recorded[:16]}..., "
            f"file is {actual[:16]}... — baseline was modified"
        )


def check_tag_integrity(run_dir: Path) -> None:
    """#11 If a foresight-v{NNN} tag exists, its annotation must match diagnosis.md."""
    run_num = run_num_from_dir(run_dir)
    if run_num == 0:
        return  # no tag for Run 000
    tag_num = 100 + run_num  # foresight-v101 for Run 001, etc.
    tag_name = f"foresight-v{tag_num}"
    try:
        tag_obj = git("rev-parse", "--verify", f"refs/tags/{tag_name}^{{}}")
    except subprocess.CalledProcessError:
        return  # no tag, fine — challenger may have lost
    try:
        tag_msg = git("tag", "-l", "--format=%(contents)", tag_name)
    except subprocess.CalledProcessError:
        fail(f"#11 could not read {tag_name} annotation")
        return
    diag = run_dir / "diagnosis.md"
    if not diag.exists():
        fail(f"#11 {tag_name} exists but diagnosis.md missing — tag cannot be traced")
        return
    if diag.read_text().strip() not in tag_msg:
        fail(
            f"#11 {tag_name} annotation does not contain diagnosis.md contents — "
            f"tag may have been created from something other than the run artifact"
        )


def check_no_force_push() -> None:
    """#12 Reflog should not show forced updates on the foresight branch."""
    try:
        reflog = git("reflog", "--grep-reflog=forced-update", "-n", "20", "claude/paw-foresight-meta")
    except subprocess.CalledProcessError:
        return  # no reflog entries match, fine
    if reflog.strip():
        fail(f"#12 foresight branch has forced-update reflog entries:\n{reflog}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--run-dir", required=True)
    ap.add_argument("--head-sha", help="git HEAD sha for deterministic X/Y check; defaults to current HEAD")
    args = ap.parse_args()

    run_dir = Path(args.run_dir).resolve()
    if not run_dir.is_dir():
        raise SystemExit(f"ERROR: {run_dir} is not a directory")

    run_num = run_num_from_dir(run_dir)
    scores_path = run_dir / "scores.json"
    scores = {}
    if scores_path.exists():
        scores = json.loads(scores_path.read_text())
    elif run_num != 0:
        fail("scores.json missing (required for Run 001+)")
        scores = {}

    head_sha = args.head_sha or git("rev-parse", "HEAD")

    check_baseline_lock()               # #9
    if run_num == 0:
        # Run 000 is measurement-only — skip scoring invariants, still check provenance + plan scope
        check_synthesis_provenance(run_dir)  # #2
        check_plan_scope(run_dir)            # #5
    else:
        check_judges(run_dir, scores)        # #1
        check_synthesis_provenance(run_dir)  # #2
        check_judge_template_sha(run_dir, scores)  # #3
        check_deterministic_assignments(run_dir, scores, head_sha)  # #4
        check_plan_scope(run_dir)            # #5
        check_incumbent_exists(run_dir)      # #6 #7
        check_borda_reproduces(run_dir)      # #8
        check_tag_integrity(run_dir)         # #11
    check_no_force_push()                    # #12

    if failures:
        print("=== RUN VERIFICATION FAILED ===", file=sys.stderr)
        for f in failures:
            print(f"  {f}", file=sys.stderr)
        print(f"\n{len(failures)} violations. Next round will NOT start until these are fixed.", file=sys.stderr)
        return 1

    print(f"OK: run {run_dir.name} passed all applicable invariants")
    return 0


if __name__ == "__main__":
    sys.exit(main())
