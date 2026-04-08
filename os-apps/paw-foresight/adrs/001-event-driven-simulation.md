# ADR-001: Event-Driven Simulation for Projections

**Status:** Accepted
**Scope:** state-machine
**Author:** OpenPaw maintainers
**Date:** 2026-04-08

## Context

Projection workflows originally relied on polling and repeated probe respawns, which created redundant work and made simulated time advancement opaque. The foresight engine needs each step to build on the previous projected state rather than re-reading a static snapshot.

## Decision

`paw-foresight` advances simulations through event-driven state transitions. Probes self-report step completion, convergence runs as its own explicit phase, and the resulting projected state becomes the next step's input. The simulation evolves because each step consumes a new artifact rather than polling the same source model again.

## Consequences

### Positive
- Every projection step and convergence boundary is visible in the entity history.
- Probe work compounds instead of resetting to the same static model.
- The app can scale by coordinating probe entities and convergence actions rather than long-lived polling loops.

### Negative
- Convergence becomes a blocking dependency that must be reliable for the loop to continue.
- Projection artifacts have to be stored and handed off carefully between steps.
