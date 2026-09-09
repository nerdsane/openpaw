# ARN-467 release plan

Full objective: deliver the software factory through Foundry and Temper, preserving the original intent. Rita's immediate slice is GitHub connection, per-session Computer copies with work linkage, then the installed Deep Sci-Fi representation visible in Foundry.

1. Connect the GitHub App to arni-labs/foundry, arni-labs/deep-sci-fi, arni-labs/katagami, nerdsane/temper and nerdsane/temperpaw. Foundry and Deep Sci-Fi are connected; Rita confirmed the full five-repository scope on 2026-09-09. Both owners must remain connected.
2. Release the existing Computer Copy repair from an isolated worktree on arni-big, under Rita's approved exception while Copy is broken. Verify native invariants and a real provider copy before shipping.
3. Review the isolated release diff, resolve findings, merge, publish the exact compute app revision to Genesis, install the pin in TemperPaw, and verify the governed Copy action live.
4. Continue the rest of the slice from a governed copy: connect Foundry session creation to Computer copies and Temper work records, then load and render the actual installed Deep Sci-Fi model.
5. Preserve the larger factory implementation and its unresolved review findings in PR504. Do not declare the full objective done when this dependency release lands.

Rita explicitly approved a separate Copy release PR on 2026-09-09, preserving PR504. This exception covers the dependency release; it does not mark the full objective complete.

Build Copy release WASMs with env -u CARGO_TARGET_DIR bash os-apps/paw-compute/wasm/build.sh. The script copies artifacts from per-module target directories; a shared root target can leave those copied artifacts stale. Run the packaged-WASM regression tests after the build to verify the bytes being shipped.
