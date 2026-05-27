# Directed Evolution Public Runtime Prompt Proof

Date: 2026-05-27

## Scope

TemperPaw local Codex worker prompt normalization for Directed Evolution
reviewer and simulated-user brain runs.

## Verification

```text
cargo fmt -p paw-codex-worker
cargo test -p paw-codex-worker directed_evolution --quiet
running 25 tests
25 passed

cargo check -p paw-codex-worker --quiet
passed

git diff --check
passed
```

## Live Evidence That Drove The Change

Tenant:

```text
de-live-repair-cycle-20260527081922
```

The cycle proved real worker execution through observer, auto-started repair
episode, variant generation, hot-loaded variant tenants, reviewer and simulated
user evaluation, and elimination.

It also exposed the prompt gap:

```text
Evaluation prompts used TemperApiBase: http://127.0.0.1:8080 even though the
variant RuntimeRef pointed at a hot-loaded Genesis tenant.
```

One simulated-user brain started `temper serve --port 8080` and left it in the
foreground. Terminating that server let the brain return its structured result.
The worker now rewrites loopback `TemperApiBase` lines to the configured public
runtime URL and adds explicit cleanup/fail-fast runtime discipline.

## Deployment Note

This is trusted local worker behavior. It does not require a Railway deployment.
