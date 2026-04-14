# Proof Report: 042 — Isolated Memory, Entity, and Actor Investigation

## Date
2026-04-14

## Workspace
- **openpaw**: `/Users/seshendranalla/Development/openpaw-codex`
- **temper**: `/Users/seshendranalla/Development/temper`

## Objective
Run a clean end-to-end investigation that isolates OpenPaw to a single process, drives a meaningful workload, and determines whether high memory and high entity counts are caused by actor hydration or by some other source.

## Why This Investigation Was Needed
Datadog graphs on the development machine were showing:

- very large `temper_indexed_entities`
- roughly `~1 GB` process RSS

The earlier dashboard state was difficult to trust because multiple local `openpaw` processes were running at the same time and all exported under the same `service:openpaw` tag.

The investigation goal was to answer four concrete questions:

1. are entity counts real, or are they cross-process pollution?
2. do high entity counts mean we are hydrating the full corpus as actors?
3. does a heavy workload cause actors to stick around?
4. how much of the memory problem is workload-driven versus baseline process footprint?

## Initial Cleanup

Before the isolated run, local process inspection showed multiple OpenPaw servers:

- `target/release/openpaw`
- `target/debug/openpaw`
- another `target/debug/openpaw`

These were listening on different ports but all reported as `service:openpaw`, which polluted Datadog views.

Those processes were stopped before the isolated run.

## Isolated Environment

Disposable environment used:

- `HOME=/tmp/openpaw-investigation.rw9x65`
- `PORT=61011`
- `OTEL_ENABLED=true`
- `DD_ENV=isolated`
- `TEMPER_API_KEY=benchmark-secret`
- `OPENPAW_WASM_STARTUP_POLICY=load-only`
- `TEMPER_RUNTIME_METRICS_INTERVAL_SECS=2`
- `TEMPER_ACTOR_IDLE_TIMEOUT=20`
- `TEMPER_PASSIVATION_CHECK_INTERVAL=5`
- `RUST_LOG=info`

Single process launched:

- binary: `target/debug/openpaw`
- pid: `63877`

Health verification:

```bash
curl http://127.0.0.1:61011/healthz
```

Actual:

- returned `200`

## Baseline Before Workload

### Local DB state

The isolated database at:

- `/tmp/openpaw-investigation.rw9x65/.local/share/openpaw/paw.db`

contained a very small corpus before the workload.

Baseline counts:

- `entity_catalog.total = 136`
- `observe/metrics total = 137`

Main entity types:

- `File = 45`
- `Directory = 38`
- `Taxonomy = 15`
- `App = 15`
- `Soul = 10`

This confirmed that the previously seen `18k+` entity counts were not coming from the normal local store.

### Baseline memory

Process RSS before the heavy workload:

- `671744 KB` (`~656 MB`)

This is important: the process already had a large memory floor before load.

## Heavy End-to-End Workload

The workload was run against the live server through the real API surface:

- `1000` `File` entity creates through `POST /tdata/Files`
- then real content uploads through `PUT /tdata/Files('{id}')/$value`
- `32` concurrent workers
- `32768` byte payload per file
- `60` second idle wait after the workload to force passivation

Run metadata from `investigation_run.json`:

- `workload_duration_secs = 5.02`
- `file_count_target = 1000`
- `file_count_created = 1000`
- `idle_wait_secs = 60`
- `peak_rss_mb = 702.61`
- `end_rss_mb = 692.45`

No workload errors were recorded.

## Entity Results

### Baseline

- `metrics.total = 137`
- `db.total = 136`

### Immediately after workload

- `metrics.total = 1137`
- `db.total = 1136`

### After idle period

- `metrics.total = 1137`
- `db.total = 1136`

### Interpretation

The corpus increased by almost exactly the number of entities we intentionally created:

- `File` count moved from `45` to `1045`

This means the entity growth was expected and explained by the workload itself. The isolated run did not reproduce any mysterious `18k` entity explosion.

## Datadog Confirmation

Datadog metrics were queried for the isolated run window:

- from `2026-04-14T03:34:20Z`
- to `2026-04-14T03:35:40Z`

Scoped to:

- `service:openpaw`
- `host:Mac`

### Returned values

- `temper_active_actors`:
  - `1001`, `0`, `1`
- `temper_indexed_entities`:
  - `1137`, `1137`, `1137`
- `temper_projected_entities`:
  - `1136`, `1136`, `1136`
- `process_resident_memory_bytes`:
  - `736739328`, `726089728`, `726089728`

Converted RSS:

- peak: `~702.6 MB`
- post-idle: `~692.5 MB`

### Interpretation

Datadog confirmed the exact runtime shape we wanted to verify:

1. active actors spiked to about `1001` during the hot path
2. active entities stayed at the new corpus size (`1137`)
3. projected entities stayed aligned with durable state (`1136`)
4. actors drained back down to essentially zero / one after idle

This is the key conclusion:

- we are **not** hydrating the full known corpus as actors
- actor residency rose with the hot working set, then collapsed after passivation

## Passivation Evidence

### Local logs

The isolated server log showed:

- `passivated idle actors count=550 timeout_secs=20`
- later `passivated idle actors count=1 timeout_secs=20`

### Datadog logs

Searching Datadog logs for:

- `service:openpaw host:Mac "passivated idle actors"`

returned entries during the same isolated window, confirming that the passivation behavior was visible in observability as well as in local logs.

## Memory Interpretation

### What the run proved

- baseline RSS before the workload was already about `656 MB`
- the heavy workload pushed peak RSS to about `702.6 MB`
- after `60s` idle, RSS settled around `692.5 MB`

So the workload itself added only about `~46 MB` of peak RSS over baseline.

That means the remaining memory concern is **not** primarily an actor explosion problem.

### vmmap check

During the isolated run, `vmmap -summary` on the process reported approximately:

- `Physical footprint: 289.8M`
- `Physical footprint (peak): 304.7M`
- `TOTAL resident: 1.0G`

Large resident contributors included:

- binary and library text / linkedit mappings
- readonly shared library pages
- malloc zones with substantial reserved resident pages

### Interpretation

On macOS, raw RSS overstated the unique memory cost of the process. The physical footprint was materially lower than the raw resident set.

This does **not** mean memory is solved. It means:

- the earlier `~1 GB` alarm was directionally useful
- but raw RSS alone is not the right way to reason about true unique process cost on this host

## Final Conclusion

The investigation got to the bottom of the actor / entity question:

1. the earlier very large entity counts were heavily polluted by multiple local OpenPaw processes reporting under the same Datadog service
2. in a clean isolated run, entity count growth matched the intentional workload exactly
3. active actors rose with the hot working set and then passivated back down after idle
4. high memory is still a real concern, but it is now better understood as:
   - a high baseline process / runtime footprint
   - allocator / mapping behavior
   - not a runaway "hydrate the whole corpus as actors" bug

## Bottom Line

- `indexed_entities` means query-plane / discovered corpus, not live actors
- `active_actors` is the real hot residency signal
- the bad startup/query-plane hydration problem did not reproduce in the isolated run
- the remaining work is baseline memory reduction, not actor-leak hunting

## Evidence Files

- run log:
  - `/tmp/openpaw-investigation.rw9x65/openpaw.log`
- structured run result:
  - `/tmp/openpaw-investigation.rw9x65/investigation_run.json`
