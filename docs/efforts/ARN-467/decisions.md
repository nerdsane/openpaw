# Decisions and tradeoffs

## D1: Keep the operational model in a separate DSF factory app

**Decision:** Add `os-apps/dsf-factory` and extend existing SDLC deployment linkage where required.

**Came up because:** DSF resources and observations outlive individual Efforts, while the user explicitly excluded migrating DSF product logic into Temper.

**Options:** Put operational records on each Effort. migrate DSF product entities. use a separate operational app linked to existing Efforts.

**Chose the operational app because:** It preserves stable resource identity and existing product implementation. It adds one published app and explicit cross-app dependencies.

**Where:** docs/efforts/ARN-467/spec.md, Operational records and Resource operations and delivery.

## D2: Preserve Foundry's run system and Temper's work records

**Decision:** Link Foundry runs to Temper Efforts and render existing Asks through a narrow integration.

**Came up because:** Foundry already provides direct computer chat, durable commands and transcripts, but currently declines MCP elicitation and overwrites harness configuration during bootstrap.

**Options:** Replace Foundry orchestration. duplicate Effort/Ask records in Foundry. retain each system's existing records and add explicit linkage and request/reply handling.

**Chose explicit linkage because:** It reuses working computer/chat behavior and gives both interfaces one authoritative decision record. The integration must handle synchronous elicitation and recovery deliberately.

**Where:** docs/efforts/ARN-467/spec.md, Foundry and agent access. private fork arni-labs/foundry at 6ca87a793df711c79e609111560ee0c7491b0c1b.

## D3: Use subscriptions for agents and bound additional spend

**Decision:** Use existing agent subscriptions without automatic API-key fallback and cap additional overnight costs at $200.

**Came up because:** Rita approved $200 and clarified that agents should run on subscriptions. DSF product verification can independently call metered APIs.

**Options:** Allow metered agent fallback. stop all paid product verification. keep agent auth subscription-only and account for necessary product API calls plus hosting/compute under the cap.

**Chose the third option because:** It matches the user's authorization while permitting real application proof. Unavailable subscription consent remains a concrete blocker. It does not authorize a different billing mode.

**Where:** docs/efforts/ARN-467/intent.md and ARN-467 authorization record.

## D4: Use separate storage for experiments

**Decision:** Isolated experiments use a disposable database and a separate media bucket.

**Came up because:** The current staging service uses production data and media, and a prefix in that bucket would not meet the declared isolation contract.

**Options:** Reuse staging. restrict a prefix in the production bucket. provision separate disposable data and storage.

**Chose separate storage because:** Provider identity gives a direct isolation check and cleanup target. It requires a small additional resource whose cost counts against the cap.

**Where:** docs/efforts/ARN-467/spec.md, Isolated exploration.

## D5: Start with Codex subscription auth in Foundry

**Decision:** Use Codex with the existing ChatGPT subscription for the first factory harness.

**Came up because:** Current Foundry supports ChatGPT subscription credentials, while its Claude adapter currently requires a metered API key. The existing arni-big computer has a Codex auth file whose validity still needs a real probe.

**Options:** Add Claude subscription support first. use metered model keys. verify and connect existing Codex subscription auth.

**Chose Codex because:** It uses Foundry's implemented subscription path and the user's intended billing mode. Unavailable auth remains visible and cannot trigger a paid fallback.

**Where:** docs/efforts/ARN-467/spec.md, Foundry and agent access.

## D6: Fix the shared action boundary before installing the factory

**Decision:** Add strict IOA parameter contracts in the Temper kernel and pin that version before installing DSF factory specifications.

**Came up because:** Real actor and HTTP tests showed undeclared action inputs and generic writes could change protected operational fields.

**Options:** Depend on agent instructions; add DSF-specific endpoint checks; enforce the specification in the shared actor boundary.

**Chose the shared boundary because:** It protects the same contract through MCP, HTTP, reactions and simulation. It adds a kernel dependency that must be reviewed and deployed before app installation.

**Where:** nerdsane/temper branch codex/dsf-factory-boundaries, docs/efforts/ARN-467/spec.md.

## D7: Put behavior on typed resources

**Decision:** Replace the generic resource and operation dispatcher with resource-specific Temper contracts whose actions own configuration, deployment, observation and rollback sequences.

**Came up because:** The user clarified that the production Railway API must be a resource with its own configuration, telemetry and deploy/rollback behavior. The current branch's generic provider switch did not express that model.

**Options:** Keep `DsfResource` and `DsfOperation` with a shared executor. Split only the executor WASMs. Define resource-specific contracts and attach narrow provider integrations to their actions.

**Chose resource-specific contracts because:** The graph exposes what each resource can do and how it changes. Production and staging can reuse a type and its integrations without sharing identity. This requires replacing the uninstalled generic contracts and updating their callers and tests.

**Where:** docs/efforts/ARN-467/spec.md, Operational records and Resource operations and delivery; docs/efforts/ARN-467/plan.md, steps 2 and 4.

## D8: Use unique app-specific names and provider-scoped identities

**Decision:** Prefix new DSF entity types and sets with `Dsf`, validate their ownership before publication, and derive resource instance IDs from full provider scope.

**Came up because:** The user reminded us that Temper cannot distinguish different entity types with the same name. The registry strips CSDL namespaces, and the live default tenant already contains `DsfDeploy`.

**Options:** Rely on app or CSDL namespace separation. Add kernel namespace support. Use accurate app-specific names with collision checks against existing definitions and installed ownership.

**Chose explicit names and checks because:** They work with the current registry without a kernel namespace change. Names become longer, but the modeled object and owning app remain clear. A matching prefix alone never authorizes replacing an installed type.

**Where:** docs/efforts/ARN-467/spec.md, Names and identities and invariants 11–12; docs/efforts/ARN-467/plan.md, step 2. Evidence: Temper registry entity-set mapping and relation target lookup, plus production `temper.specs('default')` on 2026-09-06.

## D9: Compile each provider action into its own integration

**Decision:** Give each resource action statically typed target and change inputs, with separate validation, execution, observation and verification modules.

**Came up because:** The previous common executor selected a provider at runtime and its operation records required a Git revision even for configuration changes.

**Options:** Keep a universal provider switch behind renamed modules; duplicate all proof and transport code; share proof and transport checks while binding each module to one typed action at compilation.

**Chose compilation binding because:** The module cannot deploy another provider, shared checks stay consistent, and the resource keeps its own operation sequence. A configuration change is bound to the exact requested configuration in the proof artifact; its proof commit identifies the tested source, not an invented deployment revision.

**Where:** os-apps/dsf-factory/wasm/dsf_resource_common and the provider action modules.

## D10: Resume exhausted reads without repeating the provider write

**Decision:** Add explicit resource actions to resume reconciliation or verification for the current operation key and sequence.

**Came up because:** Bounded automatic reads must stop, but exhausted counters would otherwise strand a resource after credentials or provider availability recover.

**Options:** Poll indefinitely; start a new operation and risk another write; reset only the current operation's read counter through a governed action.

**Chose explicit read resumption because:** It preserves the accepted request, provider execution and write count while allowing recovery. Resumption requires another recorded action and remains bounded.

**Where:** os-apps/dsf-factory/specs/generate.py and crates/temperpaw/tests/dsf_factory_contract.rs.

## D11: Preserve uncertainty when a media receipt is unavailable

**Decision:** A selected media job that already carries this resource operation's derived attempt UUID remains uncertain when its durable receipt is unavailable.

**Came up because:** A receipt 404 alone does not prove that no attempt was claimed; replaying recovery after a lost or removed receipt could schedule another attempt or unlock while prior work remains active.

**Options:** Treat 404 as sufficient evidence for another POST; read the selected job states and retain uncertainty when this attempt UUID is present.

**Chose the state check because:** It prevents duplicate work without creating another recovery mechanism. Reconciliation requires the receipt or an explicit investigation when receipt and job state disagree.

**Where:** os-apps/dsf-factory/wasm/dsf_media_actions/src/lib.rs and src/tests.rs.

## D12: Check live short names before installation

**Decision:** Refuse new DSF names that overlap the target tenant, and permit upgrade reuse only with the previous pinned installed bundle's ownership record and matching model digest.

**Came up because:** Production already has DsfDeploy, while its metadata also repeats unrelated dependency names across namespaces. A Dsf prefix alone does not establish ownership.

**Options:** Trust the namespace or prefix; reject all pre-existing metadata duplication; check the candidate's complete type and entity-set names against live metadata and explicit prior ownership.

**Chose candidate ownership checks because:** They protect existing apps without making this installation repair unrelated metadata. A duplicate live candidate name remains ambiguous even when a prior bundle claims it. The publication operator must export ownership from the running installation; repository state cannot substitute for that export.

**Where:** os-apps/dsf-factory/check_names.py and test_names.py. The first-install check found no overlap for the eleven introduced type and entity-set names.

## D13: Generate deployable modules from the executable contracts

**Decision:** Generate the resource entry points and app module manifest from the IOA module contract, and package each binary in the directory Temper actually loads.

**Came up because:** The app still declared the deleted generic operation modules, and the first generated source layout did not match Temper's module loader.

**Options:** Maintain the manifest, source wrappers and packaging list separately; generate them from the executable declarations.

**Chose generation because:** A declared action cannot silently lose its binary or keep a retired dispatcher. The build checks generated drift, requires every declared module's source, and verifies the resulting WASM header before packaging.

**Where:** os-apps/dsf-factory/wasm/generate_modules.py; os-apps/dsf-factory/wasm/build.sh; Dockerfile; crates/temperpaw/tests/dsf_resource_wasm.rs.

## D14: Keep recurring observations on the resource

**Decision:** Each active resource refreshes its own observations every five minutes through a native state timeout.

**Came up because:** The new typed ModelSync proxies scheduled only one refresh and added another row for behavior the resource already owns.

**Options:** Add recurring scheduling to each proxy; rearm the resource's existing observation action directly.

**Chose the resource timer because:** Returning to Active rearms observation after collection failure or operation completion. Operations and retirement suspend the timer naturally. ModelSync remains responsible for code, flow and participant sources. The five-minute reconciliation recovers missed external events without creating a second resource scheduler.

**Where:** os-apps/dsf-factory/specs/generate.py; crates/temperpaw/tests/dsf_resource_scheduling.rs; crates/temperpaw/tests/dsf_factory_contract.rs. The missing recurrence was reproduced before the change, then native timer tests proved repeated collection after failure and cancellation on retirement.

## D15: Run experiments through native Exec with disposable local services

**Decision:** Use the existing governed Computer and Exec entities for experiment phases, with a pinned runner archive, disposable PostgreSQL and MinIO data, and a fresh network namespace for every command.

**Came up because:** The staging API points at production data and media. Experiment validation, execution, selection and cleanup had declared transitions but no working integrations. The existing computer supports sudo and Linux network namespaces.

**Options:** Reuse staging. Provision another hosted database and media account. Start a separate orchestration service. Run bounded local services through native Exec and retain only owned data and receipts.

**Chose local services through Exec because:** Actual cluster and bucket identities can be checked without a new paid service or provider credentials. Native reactions and timers retain the lifecycle in Temper. Each immutable variant has a deterministic Exec identity per phase; resume reads existing execution before another start. A file lock and fsynced receipt preserve completed results across process failure. This first runner supports the agreed CORS variants and full product HTTP checks with all external calls disabled; it does not evaluate generated media quality.

**Where:** os-apps/dsf-factory/specs/experiment.ioa.toml; os-apps/dsf-factory/experiments/; os-apps/dsf-factory/wasm/dsf_experiment_common/; crates/temperpaw/tests/dsf_experiment_runtime.rs. Selection validates an Answered same-Effort Ask and an ordinary delivery Effort; the experiment never deploys directly.

## D16: Keep Effort delivery on exact resource results

**Decision:** Keep resource delivery as a separate Effort path that aggregates exact durable resource results after the agent invokes resource actions.

**Came up because:** One Effort must deliver API and frontend changes without requiring a TemperPaw image, while the existing ConfigureDeploy path must remain available.

**Options:** Reuse a generic deployment dispatcher; overload ConfigureDeploy; add separate resource configuration, merge and verification gates to Effort.

**Chose separate gates because:** They preserve the existing delivery path and keep provider writes on their owning resources. Captured plan bytes, head and sequence fence every asynchronous result. A third merge wrapper reuses the existing review/proof validators and adds correlation to failure callbacks; it does not copy those gates.

**Where:** `os-apps/paw-patrol/specs/effort.ioa.toml`; `os-apps/paw-patrol/wasm/effort_resource_delivery/`; `crates/temperpaw/tests/effort_resource_delivery.rs`.

## D17: Audit strict Effort producers

**Decision:** Enforce the Effort action boundary with strict parameters and audit every existing declared producer in the same change.

**Came up because:** An aggregate checker cannot protect delivery state if generic create/PATCH/PUT/DELETE can fabricate its fields.

**Options:** Protect only the new callbacks; opt Effort into strict parameters and verify existing callers.

**Chose strict parameters because:** The existing named actions already describe its writes. The caller audit and native Intent-to-Effort-to-TemperDeploy fixture preserve the working legacy path. HTTP tests demonstrate both policy rejection and strict rejection even after an explicit generic-write grant.

**Where:** `os-apps/paw-patrol/wasm/effort_resource_delivery/audit_callers.py`; `crates/temperpaw/tests/effort_resource_delivery.rs`.

## D18: Generate scoped factory permissions

**Decision:** Generate DSF command and runtime permissions from the declared contracts, and restrict module secret access to exact named credentials.

**Came up because:** The app had no Cedar policy, and allowing ordinary agents to report operation success would defeat its evidence checks.

**Options:** Require approvals for every action; permit all app actions; separate registered member commands from native callbacks.

**Chose separate permissions because:** Registered factory agents and humans can operate resources and maintain Flow/Participant models, while only kernel service principals can publish observations or operation results. Explicit callback forbids survive unrelated permits. Each compiled module receives only its required Temper/provider credentials; dynamic source config cannot request unrelated tenant secrets.

**Where:** `os-apps/dsf-factory/policies/`; `crates/temperpaw/tests/dsf_factory_policy.rs`.

## D19: Investigate material model changes through existing workers

**Decision:** Queue material model changes through the existing PatrolRun and WorkerRun types, using native reactions and a subscription agent in paw-codex-worker.

**Came up because:** Resource and ModelSync refreshes preserved observations but did not wake an agent to maintain the application/user model or investigate drift. The old PatrolRun WASM dispatches WorkCycle transitions imperatively.

**Options:** Reuse the WorkCycle dispatcher; keep the setup Effort open indefinitely; add an operations entity; or use native PatrolRun/WorkerRun actions with ordinary Intent/Effort/Ask for resulting repairs.

**Chose native PatrolRun/WorkerRun actions because:** They preserve worker ownership without another entity or a perpetual setup Effort. A material fingerprint includes the subject, desired configuration/revision and observed source facts. It excludes collection timestamps and derived age noise but retains participant activity, job identities and eligibility. Startup reconciliation reuses deterministic run IDs and restores interrupted child assignment or terminal evidence. Completed investigations do not rerun; failures remain visible for recovery.

**Where:** `os-apps/paw-patrol/specs/{patrol_run,worker_run}.ioa.toml`; their CSDL declarations; `crates/paw-codex-worker/src/dsf_model_{patrol,investigation}.rs`; `crates/temperpaw/tests/dsf_model_patrol.rs`.

## D20: Separate resident worker and reasoning-agent credentials

**Decision:** Give the daemon and reasoning agent separate registered Temper credentials, and launch Temper through its actual stdio MCP transport.

**Came up because:** The existing worker invocation ignores user configuration and exposes only Datadog MCP. An OData base URL is not an HTTP MCP endpoint. The current standalone temper-mcp binary reads TEMPER_API_KEY and resolves the registered identity.

**Options:** Assume an HTTP MCP route; inherit all user configuration; or configure the existing stdio binary explicitly.

**Chose explicit stdio configuration because:** It makes the resident binding inspectable. The child uses ChatGPT subscription authentication and the OpenAI provider. API billing keys and the daemon WORKER_TOKEN are removed; DSF_FACTORY_AGENT_TOKEN becomes the MCP child's TEMPER_API_KEY. The daemon rereads returned model/work references before recording completion. Cedar reserves runtime reactions and assigned-worker results even when another policy permits them. This requires a separately provisioned agent credential and current binary.

**Where:** `crates/paw-codex-worker/src/{dsf_model_investigation,execution}.rs`; `os-apps/dsf-factory/policies/model_investigation.cedar`.

## D21: Exercise the actual application catalog contract

**Decision:** Add APP.md so the platform's app catalog can load dsf-factory.

**Came up because:** A real bundle-loading test returned no dsf-factory bundle: the loader skips directories without APP.md. README.md does not satisfy that contract.

**Options:** Change the shared catalog's loading rules; provide the app guide it already requires.

**Chose the existing catalog contract because:** It fixes this app without changing other applications. The loader already enumerates every top-level policies/*.cedar file. A catalog test proves the generated provider policy and investigation policy are both loaded.

**Where:** `os-apps/dsf-factory/APP.md`; `crates/temperpaw/tests/dsf_model_patrol.rs`.

## D22: Revise resource models without changing provider identity

**Decision:** Add an Active-only ReviseModel action with a model sequence check and provenance reference, while retaining immutable provider identity and delivery history.

**Came up because:** Register was the only way to set configuration bindings, dependencies and operation availability. Strict generic writes correctly refused subsequent changes, so agents could not keep the resource graph current or bind an observation-only discovery later.

**Options:** Permit generic updates; replace the row and lose its history; or declare the exact model-edit action on each typed resource.

**Chose the named action because:** It allows agents to maintain labels, source repository, dependencies, pinned configuration and available operations without changing provider identity, environment, observed facts or desired delivery state. A sequence check refuses stale edits. Revision clears current operation verification flags so earlier completion evidence cannot satisfy delivery against a changed binding. Only Active resources can be rebound; no provider integration runs during this action.

**Where:** `os-apps/dsf-factory/specs/generate.py`; generated resource contracts and policy; `crates/temperpaw/tests/dsf_resource_model_revision.rs`.
