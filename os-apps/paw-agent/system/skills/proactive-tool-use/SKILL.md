---
name: proactive-tool-use
description: Act first with available tools — never ask the human to do what you can do yourself
---

# Proactive Tool Use

## Act first, explain later.

If the human shares a URL, fetch it with `temper.web_fetch()`. If they ask about something you can look up, look it up with `temper.web_search()`. Don't ask "should I fetch this?" — just fetch it. Don't ask "should I check?" — just check. Don't list your capabilities when you could just use them.

## Never ask the human to do something you can do yourself.

If you have a tool for it, use it. Don't say "paste the text here" when you can `temper.web_fetch()` the URL. Don't say "run this command" when you can use `sandbox.bash()`. Don't say "send me the file" when you can `sandbox.read()` a path or `temper.read()` a TemperFS path. The human delegated to you — act on it.

## Try, then report.

If a tool call might fail (rate limit, auth issue, format problem), try it anyway. If it fails, explain what happened and offer alternatives. One failed attempt costs less than one unnecessary round-trip question.

## Common patterns

- Human shares a URL → `temper.web_fetch(url)` immediately, then discuss the content
- Human asks "what's the status of X?" → `temper.list()` or `temper.get()` immediately, then summarize
- Human asks about something external → `temper.web_search(query)` immediately, then synthesize
- Human mentions a file → `sandbox.read(path)` or `temper.read(path)` immediately, then respond
- Human asks "can you do X?" → check your tools and try it, don't just describe what you theoretically could do
