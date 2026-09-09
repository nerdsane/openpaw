# ARN-467: Computer Copy repair release

ARN-467's full objective remains in [the existing factory PR](https://github.com/nerdsane/temperpaw/pull/504). The current delivery order is GitHub connection, copied Foundry sessions linked to Temper work, then the installed Deep Sci-Fi model displayed in Foundry. Operational delivery and experiments are retained in that existing work and are not prerequisites for this release.

This release repairs the Computer Copy primitive needed by the session path.

Copy starts at most one provider request for a child Computer. Its exact name includes the full child and source IDs. A failed or lost response retains CopyUnknown; ReconcileCopy reads the provider by the recorded name and never creates another copy. The returned destination must differ from the source, retain the source binding and belong to the same provider project. The source cannot be destroyed while an uncertain child still refers to it. Readiness moves a known copy to Leased.

The executable contract is os-apps/paw-compute/specs/computer.ioa.toml. Its model and invariants are exercised by computer_copy_reconciliation.rs. The explicitly invoked live test drives the actual compiled WASMs against Tensorlake, verifies a file copied from the source, and verifies asynchronous cleanup. Ordinary CI never provisions provider resources.

This is a Genesis compute-app release on the deployed kernel d7a48b92f7caf724067972640c0cfc302f6a350e. Provider identity validation belongs to the existing Copy WASM boundary, and Cedar restricts callbacks to trusted integrations. The app does not use the newer kernel parameter constraints. No kernel upgrade, vendored dependency change, legacy temper-agents change or runtime image deployment belongs to this slice.


### First-attempt rejection and uncertain recovery

A failed initial Computer copy closes without teardown only before any provider copy submission or existing destination is observed. Once submission may have occurred, CopyUnknown retains the source binding and permits GET-only reconciliation. Every failed reconciliation remains CopyUnknown. CopyRejected is an integration-only callback from Provisioning to Destroyed, with no termination effect; ordinary agents cannot invoke it. The native actor simulation and packaged-WASM failure cases enforce this same contract.
