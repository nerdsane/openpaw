#!/usr/bin/env python3
"""Convert paw-agent session JSONL transcripts to OTS-structured JSON.

Reads raw JSONL session transcripts (as stored in TemperFS) and produces
structured trajectory data compatible with Temper's Open Trajectory Spec.

Usage:
    # Single file
    python3 jsonl_to_ots.py transcript.jsonl

    # Whole run directory (reads all .jsonl in transcripts/)
    python3 jsonl_to_ots.py meta/runs/010_decision_analyst_session/transcripts/

    # Output to file
    python3 jsonl_to_ots.py transcripts/ -o trajectories.json

    # Diagnostic summary only (no full OTS)
    python3 jsonl_to_ots.py transcripts/ --summary
"""

import json
import sys
import os
from pathlib import Path


def parse_jsonl(filepath):
    """Parse a JSONL file into a list of entries."""
    entries = []
    with open(filepath) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                entries.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return entries


def extract_tool_calls(content):
    """Extract tool_use blocks from an assistant message's content."""
    if not isinstance(content, list):
        return []
    return [c for c in content if isinstance(c, dict) and c.get("type") == "tool_use"]


def extract_tool_results(content):
    """Extract tool_result blocks from a user/tool message's content."""
    if not isinstance(content, list):
        return []
    return [c for c in content if isinstance(c, dict) and c.get("type") == "tool_result"]


def extract_text(content):
    """Extract plain text from message content (string or content blocks)."""
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = []
        for c in content:
            if isinstance(c, dict):
                if c.get("type") == "text":
                    parts.append(c.get("text", ""))
                elif c.get("type") == "tool_use":
                    code = c.get("input", {}).get("code", "")
                    parts.append(f"[tool_use: {c.get('name', '?')}] {code[:200]}")
                elif c.get("type") == "tool_result":
                    result_text = c.get("content", "")
                    if isinstance(result_text, str):
                        parts.append(f"[tool_result] {result_text[:200]}")
            elif isinstance(c, str):
                parts.append(c)
        return "\n".join(parts)
    return str(content)


def jsonl_to_ots(entries, session_id="", role="unknown"):
    """Convert parsed JSONL entries into OTS-structured trajectory."""
    trajectory = {
        "trajectory_id": f"ots-{session_id}" if session_id else "ots-unknown",
        "version": "1.0",
        "metadata": {
            "agent_id": role,
            "session_id": session_id,
            "outcome": "unknown",  # filled by caller from session status
            "turn_count": 0,
            "total_tokens": 0,
        },
        "turns": [],
    }

    # Group entries into turns: assistant message + optional tool results
    # A "turn" = one LLM call cycle: user context -> assistant response -> tool execution
    i = 0
    turn_num = 0
    total_tokens = 0

    while i < len(entries):
        entry = entries[i]
        entry_type = entry.get("type", "")
        entry_id = entry.get("id", "")
        role_field = entry.get("role", "")

        # Skip header
        if entry_type == "header":
            i += 1
            continue

        # User message (system prompt or steering)
        if role_field == "user":
            # Check if next entry is an assistant response
            if i + 1 < len(entries) and entries[i + 1].get("role") == "assistant":
                turn_num += 1
                user_entry = entry
                assistant_entry = entries[i + 1]

                user_tokens = user_entry.get("tokens", 0)
                asst_tokens = assistant_entry.get("tokens", 0)
                total_tokens += user_tokens + asst_tokens

                # Extract tool calls from assistant response
                tool_calls = extract_tool_calls(assistant_entry.get("content", []))
                decisions = []

                for tc in tool_calls:
                    code = tc.get("input", {}).get("code", "")
                    # Parse temper.* calls from the code
                    temper_calls = []
                    for line in code.split("\n"):
                        line = line.strip()
                        if line.startswith("temper.") or line.startswith("sandbox."):
                            temper_calls.append(line[:150])

                    decisions.append({
                        "decision_type": "ToolSelection",
                        "tool": tc.get("name", "unknown"),
                        "tool_use_id": tc.get("id", ""),
                        "choice": {
                            "action": tc.get("name", "unknown"),
                            "code_snippet": code[:500],
                            "temper_calls": temper_calls,
                        },
                        "consequence": None,  # filled from tool_result
                    })

                # Look ahead for tool results
                j = i + 2
                while j < len(entries):
                    next_entry = entries[j]
                    next_content = next_entry.get("content", [])
                    tool_results = extract_tool_results(next_content)

                    if tool_results:
                        total_tokens += next_entry.get("tokens", 0)
                        for tr in tool_results:
                            # Match to decision by tool_use_id
                            for d in decisions:
                                if d["tool_use_id"] == tr.get("tool_use_id"):
                                    result_content = tr.get("content", "")
                                    if isinstance(result_content, str):
                                        result_text = result_content[:1000]
                                    else:
                                        result_text = str(result_content)[:1000]
                                    d["consequence"] = {
                                        "outcome": "error" if tr.get("is_error") else "success",
                                        "result_preview": result_text,
                                    }
                        j += 1
                    elif next_entry.get("role") == "assistant":
                        # Next assistant message — part of same turn if it's
                        # continuing after tool results
                        break
                    else:
                        j += 1
                        break

                turn = {
                    "turn_id": turn_num,
                    "messages": [
                        {
                            "role": "user",
                            "tokens": user_tokens,
                            "has_content_file": bool(user_entry.get("content_file_id")),
                            "content_preview": extract_text(user_entry.get("content", ""))[:300] if not user_entry.get("content_file_id") else "[content in TemperFS file]",
                        },
                        {
                            "role": "assistant",
                            "tokens": asst_tokens,
                            "content_preview": extract_text(assistant_entry.get("content", ""))[:300],
                        },
                    ],
                    "decisions": decisions,
                    "tool_call_count": len(tool_calls),
                }
                trajectory["turns"].append(turn)
                i = j
                continue

        i += 1

    trajectory["metadata"]["turn_count"] = turn_num
    trajectory["metadata"]["total_tokens"] = total_tokens
    return trajectory


def trajectory_summary(traj):
    """Produce a concise diagnostic summary of a trajectory."""
    meta = traj["metadata"]
    turns = traj["turns"]

    # Count tool calls and errors
    total_tool_calls = 0
    total_errors = 0
    temper_call_counts = {}

    for turn in turns:
        for d in turn.get("decisions", []):
            total_tool_calls += 1
            consequence = d.get("consequence")
            if consequence and consequence.get("outcome") == "error":
                total_errors += 1
            for tc in d.get("choice", {}).get("temper_calls", []):
                method = tc.split("(")[0] if "(" in tc else tc
                temper_call_counts[method] = temper_call_counts.get(method, 0) + 1

    # Find the most-called temper methods
    top_methods = sorted(temper_call_counts.items(), key=lambda x: -x[1])[:10]

    summary = {
        "session_id": meta.get("session_id", ""),
        "agent_role": meta.get("agent_id", ""),
        "outcome": meta.get("outcome", "unknown"),
        "turns": meta.get("turn_count", 0),
        "total_tokens": meta.get("total_tokens", 0),
        "tool_calls": total_tool_calls,
        "errors": total_errors,
        "error_rate": f"{total_errors/total_tool_calls*100:.0f}%" if total_tool_calls > 0 else "n/a",
        "top_temper_methods": top_methods,
    }

    # Extract what the agent actually did (first and last turns)
    if turns:
        first_decisions = turns[0].get("decisions", [])
        if first_decisions:
            summary["first_action"] = first_decisions[0].get("choice", {}).get("code_snippet", "")[:200]
        last_decisions = turns[-1].get("decisions", [])
        if last_decisions:
            summary["last_action"] = last_decisions[-1].get("choice", {}).get("code_snippet", "")[:200]

    return summary


def process_directory(dirpath, summary_only=False):
    """Process all JSONL files in a transcripts directory."""
    results = {}
    dirpath = Path(dirpath)

    # Read MANIFEST.md if it exists for session metadata
    manifest = {}
    manifest_path = dirpath / "MANIFEST.md"
    if manifest_path.exists():
        for line in manifest_path.read_text().splitlines():
            # Parse: - **role** (session_id): N turns, status=X, N bytes
            if line.startswith("- **"):
                parts = line.split("**")
                if len(parts) >= 3:
                    role = parts[1]
                    rest = parts[2]
                    sid = ""
                    status = "unknown"
                    if "(" in rest and ")" in rest:
                        sid = rest.split("(")[1].split(")")[0]
                    if "status=" in rest:
                        status = rest.split("status=")[1].split(",")[0].split(")")[0]
                    manifest[role] = {"session_id": sid, "status": status}

    for jsonl_file in sorted(dirpath.glob("*.jsonl")):
        role = jsonl_file.stem
        entries = parse_jsonl(jsonl_file)
        if not entries:
            continue

        # Get session metadata from manifest
        meta = manifest.get(role, {})
        session_id = meta.get("session_id", "")
        status = meta.get("status", "unknown")

        traj = jsonl_to_ots(entries, session_id=session_id, role=role)
        traj["metadata"]["outcome"] = status.lower()

        if summary_only:
            results[role] = trajectory_summary(traj)
        else:
            results[role] = traj

    return results


def main():
    import argparse
    parser = argparse.ArgumentParser(description="Convert paw-agent JSONL to OTS")
    parser.add_argument("path", help="JSONL file or transcripts/ directory")
    parser.add_argument("-o", "--output", help="Output file (default: stdout)")
    parser.add_argument("--summary", action="store_true", help="Diagnostic summary only")
    args = parser.parse_args()

    path = Path(args.path)

    if path.is_dir():
        results = process_directory(path, summary_only=args.summary)
    elif path.is_file() and path.suffix == ".jsonl":
        entries = parse_jsonl(path)
        traj = jsonl_to_ots(entries, session_id=path.stem, role=path.stem)
        results = trajectory_summary(traj) if args.summary else traj
    else:
        print(f"Error: {path} is not a .jsonl file or directory", file=sys.stderr)
        sys.exit(1)

    output = json.dumps(results, indent=2)

    if args.output:
        Path(args.output).write_text(output)
        print(f"Wrote {len(output)} bytes to {args.output}", file=sys.stderr)
    else:
        print(output)


if __name__ == "__main__":
    main()
