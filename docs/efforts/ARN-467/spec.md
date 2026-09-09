# ARN-467: Computer Copy repair release

ARN-467's full objective remains in [the existing factory PR](https://github.com/nerdsane/temperpaw/pull/504). The current delivery order is GitHub connection, copied Foundry sessions linked to Temper work, then the installed Deep Sci-Fi model displayed in Foundry. Operational delivery and experiments are retained in that existing work and are not prerequisites for this release.

This release repairs the Computer Copy primitive needed by the session path.

Copy starts at most one provider request for a child Computer. Its exact name includes the full child and source IDs. A failed or lost response retains CopyUnknown; ReconcileCopy reads the provider by the recorded name and never creates another copy. The returned destination must differ from the source, retain the source binding and belong to the same provider project. The source cannot be destroyed while an uncertain child still refers to it. Readiness moves a known copy to Leased.

The executable contract is os-apps/paw-compute/specs/computer.ioa.toml. Its model and invariants are exercised by computer_copy_reconciliation.rs. The explicitly invoked live test drives the actual compiled WASMs against Tensorlake, verifies a file copied from the source, and verifies asynchronous cleanup. Ordinary CI never provisions provider resources.

The kernel dependency is the already merged and independently verified a82410bd51915204406955d46d0f2bc5d09db8fa used by the application proof. There is no new kernel implementation in this release.
