## D27: Reconcile uncertain copies by their complete recorded request

**Decision:** Preserve uncertain copies in CopyUnknown and reconcile the exact provider name derived from the full child and source IDs, without repeating the copy request.

**Came up because:** The old helper put unsupported timeout and count fields in a JSON body and never sent the intended name. Tensorlake used the source's default copy name, leaving an uncertain 502 followed by a name collision. CopyFailed marked the child Destroyed even though a provider copy might exist and its machine_id still identified the source.

**Options:** Retry the POST, truncate identifiers into a shorter name, adopt a loose name match, or retain one recorded request and reconcile it through GETs.

**Chose the recorded request because:** The full child and source IDs give an exact correlation key within Tensorlake's 63-character name limit; unsupported identifiers fail before any provider request. Recovery requires the exact name, a destination different from the source, and matching provider project namespaces. A received partial copy response must also report the exact source_sandbox_id and the same destination found by name. After a completely lost response, the name proves correlation with the recorded request; it does not independently prove provider-reported source lineage, which GET does not expose. ReconcileCopy only runs on an existing CopyUnknown child and never sends another POST. Destroy remains unavailable while the child still holds the source machine ID. This retains uncertain resources for investigation instead of claiming cleanup occurred.

The CI build list includes paw-compute before native tests because its generated WASMs are ignored by Git. D31 removes the Docker change: this release installs the Genesis compute app without replacing the runtime image.

**Where:** os-apps/paw-compute/specs/computer.ioa.toml; os-apps/paw-compute/wasm/computer_copy_start/src/lib.rs; os-apps/paw-agent/wasm/wasm-helpers/src/sandbox.rs; crates/temperpaw/tests/computer_copy_reconciliation.rs; Dockerfile; .github/workflows/ci.yml.

## D28: Use the verified kernel for the isolated application release (superseded by D31)

**Decision:** Pin all application and worker Temper dependencies to the already merged a82410bd51915204406955d46d0f2bc5d09db8fa used by the successful native and live Copy proof.

**Came up because:** Current main still pins the older d7a48b92 kernel, while ARN-467's verified application build and Copy recovery proof use a82410bd.

**Options:** Keep the older runtime and qualify it separately, or reuse the already verified published runtime and check the application and worker against it.

**Chose the verified runtime because:** It uses the same evaluator and WASM host as the successful proof. This advances an existing dependency; it adds no kernel implementation. The clean release's native reconciliation tests and worker check pass.

**Where:** crates/temperpaw/Cargo.toml; crates/paw-codex-worker/Cargo.toml; Cargo.lock.

## D29: Prepare an isolated dependency release without discarding the full factory work

**Decision:** Prepare the Copy repair as a separate branch from current main and request Rita's exception before opening another PR for ARN-467.

**Came up because:** Rita explicitly prioritized GitHub, copied sessions and the installed model visualization; existing PR504 also contains unfinished operational delivery and experiment work.

**Options:** Make completion of all existing work a prerequisite, discard that work, or preserve PR504 and prepare an isolated dependency release.

**Chose isolated preparation because:** It keeps the original objective and implementation intact while making the required Copy fix independently reviewable. Rita explicitly approved the separate Copy release PR on 2026-09-09. The original factory effort remains open; this release removes its sandbox-copy dependency.

**Where:** docs/efforts/ARN-467/spec.md; docs/efforts/ARN-467/plan.md; branch codex/arn467-copy-release.


## D30 — Distinguish a rejected first copy from an uncertain provider result

**Decision:** Close a first copy attempt without teardown only when it fails before submitting a provider copy or finding an existing destination; keep every reconciliation failure uncertain.

**Came up because:** The first release panel found that missing credentials, invalid identifiers and a failed source lookup could leave a child permanently waiting for a copy that was never submitted.

**Options:** Preserve all failures as CopyUnknown; classify every pre-POST error as terminal; distinguish initial preflight from submission and reconciliation.

**Chose the third option because:** The native ProvisionFromCopy action can run only from Created and its integration is dispatched once; no background WASM retry is scheduled by the pinned kernel. An error before that first request can safely close the child. ReconcileCopy is always GET-only and never closes the record on failure, even if its credentials or source lookup fail. An already-found named destination also preserves uncertainty if validation fails. This adds one callback, no retry loop, and never terminates the stored source machine.

**Where:** os-apps/paw-compute/specs/computer.ioa.toml; os-apps/paw-compute/wasm/computer_copy_start/src/lib.rs; os-apps/paw-agent/wasm/wasm-helpers/src/sandbox.rs; https://github.com/nerdsane/temperpaw/pull/507.

## D31: Keep the Copy repair on the deployed kernel

**Decision:** Restore the existing d7a48b92 kernel dependencies and ship the repair as a Genesis compute app, without a TemperPaw image deployment.

**Came up because:** Rita challenged the unnecessary kernel upgrade, vendored libSQL source and legacy temper-agents work. D28 reused a newer tested runtime without first demonstrating that Copy required it. The old parser loads the spec but ignores the new CopyStarted constraints; claiming it could not load the app was incorrect.

**Options:** Upgrade the kernel, install unenforced constraints, or keep identity validation at the existing provider/WASM boundary with Cedar restricting callbacks and qualify the old runtime.

**Chose the old runtime because:** Its WASM implementation is identical to the newer revision. Native transition and Cedar compatibility tests pass against d7a48b92 with the added constraints removed. The real provider proof is recorded separately. This retains source/destination validation in the Copy helper and removes the unsupported native-constraint assertions; trusted callback code remains responsible for valid callback values. No legacy agent migration or database dependency project is a prerequisite for Foundry.

**Where:** crates/temperpaw/Cargo.toml; crates/paw-codex-worker/Cargo.toml; Cargo.lock; os-apps/paw-compute/specs/computer.ioa.toml; crates/temperpaw/tests/computer_copy_reconciliation.rs; https://github.com/nerdsane/temperpaw/pull/507.
