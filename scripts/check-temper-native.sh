#!/bin/bash
# Temper-native architecture review
# Claude Code PreCommit hook — spawns Architect Reviewer agent to review Rust diffs
#
# Configured in .claude/settings.json:
#   "hooks": { "PreCommit": [{ "command": "scripts/check-temper-native.sh" }] }

DIFF=$(git diff --cached -- 'crates/**/*.rs')
[ -z "$DIFF" ] && exit 0  # No relevant Rust changes

RESULT=$(claude --print \
  --system-prompt "$(cat .claude/agents/temper-native-reviewer.md)" \
  -p "Review this staged diff for Temper-native compliance:

$DIFF")

echo "$RESULT" | grep -q "^PASS" && exit 0
echo "Architect Reviewer: $RESULT"
exit 1
