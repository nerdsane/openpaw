# Decision log — ARN-462 (image break)

**Decision:** The Docker death on #500/#501 is `chain_github_ready`, not `chain_file_ready`.
**Came up because:** The build.sh echo prints `Building chain_file_ready` immediately before `Building chain_github_ready`. The getrandom `compile_error!` is in the second crate's unit graph (`rsa` → `rand_core` → getrandom 0.2.17). `chain_file_ready` has the same `temper-wasm-sdk` pin and does not pull getrandom; it compiles on rust 1.94.
**Options:** (1) add getrandom `custom` to every new chain crate; (2) add it only to crates that pull getrandom; (3) enable getrandom `js`.
**Chose (2) over (1) and (3) because:** (1) forces every crate to register a backend or fail to link. (3) pulls wasm-bindgen, which this host forbids. What we gave up: a crate that later grows an accidental getrandom dep still needs the stub; the rsa lint catches the known class, not every future transitive.
**Where:** `os-apps/paw-patrol/wasm/chain_github_ready/Cargo.toml`; Docker run 33900344300.

---

**Decision:** Same crate-local getrandom `custom` stub as `release_run_lifecycle`. Do not put it on `wasm-helpers`. Do not migrate patrol to wasip1 in this change.
**Came up because:** ARN-460 already chose this stub and recorded that the door crate omitted it. ARN-443 is the regression from putting getrandom on helpers. ARN-447 is the wasip1 migration.
**Options:** (1) helpers; (2) wasip1 for all patrol modules; (3) copy the existing stub onto the door.
**Chose (3) over (1) and (2) because:** (1) is the known break. (2) is a different issue with its own Linear. What we gave up: patrol stays on the forbidden-by-policy unknown-unknown target until ARN-447.
**Where:** `chain_github_ready/src/lib.rs`; `release_run_lifecycle/src/github_app.rs`.

---

**Decision:** Encode the class on the PR `checks` path with a lint plus `cargo check` of the two rsa crates, not a full os-app `build.sh`.
**Came up because:** ARN-429 keeps full wasm builds off PRs so checks stay under five minutes. That is why #500/#501 CI was green and Docker was red.
**Options:** (1) run patrol `build.sh` on every PR; (2) lint only; (3) lint plus `cargo check` of the rsa crates on `wasm32-unknown-unknown`.
**Chose (3) over (1) and (2) because:** (1) blows the PR budget. (2) would miss a stub that is declared but does not link. What we gave up: a non-rsa crate that newly pulls getrandom 0.2 still only fails in Docker/`full`.
**Where:** `scripts/check-wasm-rsa-getrandom.sh`; `.github/workflows/ci.yml`.
