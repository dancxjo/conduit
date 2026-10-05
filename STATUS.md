# Current project status

Capability summary, with Face journey and documentation corrections reviewed on
**2 October 2026**. Other entries retain their existing proof scope.
This is a capability summary, not a claim that every check has been rerun today.
The [current-product truth surface](https://dancxjo.github.io/conduit/current-product.html)
shows the exact latest development commit, accepted release, published Pages
artifact, lag, and proof receipts. Published products follow accepted releases
and can lag development. The [recorded acceptance history](docs/history/accepted-milestones.md)
preserves older proof receipts and limitations.

**[Follow the ConduitOS journey](https://dancxjo.github.io/conduit/journeys/current/conduitos/x86_64/)**
for the ordered actions, captured screens, and exact emulator evidence.

## What exists

| Area | Available behavior and evidence | Boundary |
|---|---|---|
| **Language and composition** | Canonical `.conduit` parsing, located diagnostics, checking, recursive Plots through Fores/Backs, exact typed ports, open-ended numeric semantic ranges, and separate source/checked/expanded identities. Ordinary examples run through planning and the production kernel. | Every actual value and implementation resource envelope is finitely admitted even when a Type's semantic domain is open; a catalog entry does not promise an implementation on every Host. See [plots](plots/README.md). |
| **Planning and execution** | One port-aware kernel, explicit fan-out, bounded queues and Host Calls, plan-owned per-Step fuel, fair cooperative yielding, resource and authority admission, immutable plans, pressure, and correlated signs. Exact abnormal-terminal kinds, separate terminal-transduction profiles, containing-Plot propagation, typed semantic cancellation requests, and cancellation-driven deadlines remain checked and planned through execution. Shared code serves hosted, browser, and embedded paths. Confined Wasm instruction fuel forcibly returns control even from a non-cooperative infinite loop. | A cancellation request is not proof of cancellation, and successful fallback does not manufacture semantic abnormal truth. Native in-process, browser, ConduitOS, and firmware Backs remain cooperative unless their exact Host profile names and proves a hard containment mechanism. General SMP, preemption, and physical real-time guarantees are not established by cooperative execution. |
| **body lifecycle** | Zero/one/many initial plots, durable part membership, current/offline host presence, body-wide workload admission, one current plan and at most one active play. Multi-plot execution and workload changes have hosted and browser evidence. Live plots treat structural drain as quiescence; a canonical trailing full stop makes that drain semantic completion. | This is implemented, not merely the old #2062 proposal. Durable keep realization across Wake, Boot, and Body loss remains part of the open persistence vertical rather than an implicit lifecycle promise. |
| **Face and Masks** | One bounded renderer-neutral Face grammar carries exact basis, subjects, relationships, typed content, wording, actions, inputs, semantic order, context, and provenance. Ordinary Plots contribute Face truth; ordinary Plots serving the Mask role realize exact Face revisions as Shows through deterministic-linear, browser, and native graphical routes. Bounded hosted terminal and mechanical spoken Mask implementations also consume that Face. Installed screen-free access can announce zero-Body arrival, perform user-driven Birth, and stream an explicitly selected eSpeak reading to an observed ALSA device. Authored Body source admits an unordered `wear …, …` eligibility set and optional ordered `want … over …` policy while runtime wear/doff remains replacement-planning input rather than Plan mutation. | The Rust carrier type is still named `Presentation` during migration. These implementations do not yet establish one live Body with owner-sealed Mask routes across three hosts, a complete screen-free journey after Birth, or human listening. Browser permission, device availability, tab lifetime, and supported profile still apply. |
| **Patchbay** | A resident native projection shows the active plots on the present body with exact plan/play identities and Mask topology; broader native and browser inspection/editing, bounded Watches, observation replay, and scoped breakpoint/causal-trace support also exist. | Patchbay is a projection over authoritative body and execution truth, not a second scheduler. Replay is distinct from re-execution, and these features do not establish distributed stop-the-world debugging. |
| **Crèche and make** | Birth a body with reviewed plots, prepare target-native artifacts, inspect membership, and retain body evidence into Patchbay. Make packages cover hosted computers, browser, ConduitOS, and board families. | Building or downloading an artifact is distinct from installing, booting, admitting a part, and executing work. Consult each [target](targets/README.md). |
| **ConduitOS** | Five product targets: x86_64 and IA-32 PC, AArch64 and RISC-V64 virt, and LoongArch64 virt. The x86_64 graphical journey uses QMP keyboard input to arrive at Crèche, birth and wake one Body, plan and play it, open Patchbay, inspect its semantic Face and live graph, and stop. Ten screen captures and a correlated receipt come from the retained product image. Its graphical Mask moves one boot-discovered framebuffer resource through an exact planned resource Cord into renderer possession, authorizes one present operation, then revokes that possession at Play end; the other four targets have serial product media. | The illustrated journey is **freestanding-emulator** proof. Non-x86 targets still need interactive input and lifecycle loops for parity. The physical laptop campaign is open. A target's boot proof does not establish graphics, drivers, or hardware parity. |
| **Device protocols (development)** | Reviewed, host-independent Source owns the complete BME280 probe, initialization, calibration, timed sample capture and compensation over finite I2C and clock Backs. x86_64 ConduitOS can package additional checked Source over an existing capable product kernel; a retained freestanding run publishes exact Plan/Play identities, a typed controller refusal and retirement. Deterministic native conformance also covers successful observations, wrong identity and clock loss. | Physical BME280 compatibility remains unverified. Root separately admits the controller, attachment, authority and clock; Source grants none of them. See [device protocol boundary](docs/architecture/device-protocol-plots.md) and [#4833](https://github.com/dancxjo/conduit/issues/4833). |
| **lines and physical Pico work** | Recorded WebSocket/USB CDC execution and one-body Pico W control, including new-plan recovery and continuation over an already-admitted fallback line. | This is bounded, device-specific physical evidence. It does not imply arbitrary discovery, federation, public-Internet security, or a general reconnect policy. |
| **Remote Host rendezvous** | One bounded CBOR/CDDL descriptor carries ordered authenticated direct, WebRTC DataChannel, protected user-operated relay, loopback WebSocket, and attended-serial candidates. Browser, native std, and ConduitOS consume the same candidate meanings; each target attempts only supported Lines. Candidate expiry, attempt count, timeout, ordered failure evidence, direct-versus-relayed truth, and the existing invitation/admission session remain distinct. | Automated proofs establish the shared wire semantics, finite fallback scheduler, real browser/native WebRTC transport, protected relay, and ordinary Host admission above a selected Line. They do not substitute for the attended multi-machine, multi-network run owned by [#3711](https://github.com/dancxjo/conduit/issues/3711). ConduitOS truthfully skips WebRTC rather than pretending to implement it. |
| **Standard semantics and local tasks** | Executable text, time, state/flow, logic/math, input, Face composition, and other reviewed families; protected local file-copy operations exist. Catalog and target gap reports derive availability from code. | The old “copy a file is disabled” record describes a retired prototype. ConduitOS storage and physical file-copy remain separate unfinished work. |

For implementation owners, start with the [repository map](docs/repository-layout.md)
and [architecture references](docs/architecture/README.md). The table above
summarizes capabilities; the historical ledger retains the detailed evidence
for earlier accepted slices without turning every past limitation into a
present limitation.

## Recent implementation and unfinished integration

These source-level capabilities are present in the reviewed development tree.
Their presence is not an additional physical or release acceptance claim:

- **Owner presentation routes (#4922, development):** The installed owner can
  present its actual lulled clock Body through an attached terminal Mask without
  inventing a workload Wake or Play. The ordinary Mask Plan/Play returns an
  acknowledged Show and typed interval action to the same owner's workset.
  Finite owner presentation Plans retain exact child routes and distinguish
  already-sealed alternative selection from replacement and fresh Show
  acknowledgement. `cargo xtask check owner-presentation` exercises actual
  Unix-provider attachment/actions and deterministic seal, loss, cancellation,
  bound and wardrobe contracts. The [route contract](docs/architecture/owner-presentation-routes.md)
  records the browser carrier's narrower evidence boundary. This does not prove
  presentation during active installed workload execution, a distributed Mask
  Plot, the live multi-host journey or its publication under #4807.

- **Typed claims and resolution (#4950, development):** Core admits immutable,
  bounded domain assertions, lifecycle history, exact score contracts, and
  policy-owned selection or abstention. Deterministic core/language fixtures
  retain sensor/model alternatives and manual correction, revisioned dependency
  alternatives with conservative shared facts, and unresolved runtime diagnoses.
  The [contract](docs/architecture/claims.md) separates Signs, claims, resolution,
  and authority. This establishes a library seam for #4907, not its complete
  parser, a global claim store, or focused Patchbay integration.

- **Static browser applications (#4804):** `cargo xtask make body static`
  packages the Handbook or another application as ordinary static files. The
  Handbook uses the public Browser SDK to birth/recover an application-scoped
  local body, run checked examples, edit with native live syntax highlighting,
  and inspect the current resident Patchbay graph. Local Chromium acceptance
  covers source replacement, reload/restart, independent browser profiles,
  same-origin application isolation, tab ownership, and reset. New body identity
  derivation includes its birth sign; retained legacy identities remain valid.
  Protected integration, stable publication, and public verification completed
  on 2 October 2026; [#4804 is accepted](https://github.com/dancxjo/conduit/issues/4804#issuecomment-5960026693).
  The live shared-Body journey remains separate work under #4807.

- **Native semantic Types and compact Forms (#4382, #4428):**
  Conduitese owns nominal scalar, record, variant, optional, data-reference,
  bounded-sequence, and refined primitive meaning. Rust bindings are generated
  from the checked graph. Generic native type families now specialize to exact
  finite concrete types; `DataGenerationValue<T>` and its 4096-byte text
  specialization are a current example. This does not close every remaining
  domain migration. A named compact `u8` Form derives iota
  tags from authored variant order, typed malformed-input refusal, finite
  extent/work bounds, and a mechanical compatibility identity. The Data Text
  terminal family has no handwritten Rust tag table, and generated Rust plus
  ECMAScript consume the same checked declaration. #4431 and #4382 completed
  the payload-rich Type and repository-audit foundations; reviewed follow-up
  migrations continue under #4375. A Type owns meaning while a Form owns one
  concrete portable representation; their identities remain distinct and a
  Type may have multiple Forms. Bounded each/select/fold/scan and collection
  are complete under [#4378](https://github.com/dancxjo/conduit/issues/4378), with
  std and browser runtime/WASM proof. Current admitted ConduitOS resident plots
  do not exercise those combinators; this is not universal embedded or browser
  interaction/E2E proof. The [language reference](wiki/Current-language-surface.md)
  links exact checking and execution evidence.
  HTTP scheme, transaction identity, target and header meaning are likewise
  native declarations; hosted and ConduitOS HTTP code consumes generated
  bindings while its HTTP/1.1 bytes remain an external adapter contract.
  Process Job request, executable, argument/environment, output, pressure,
  usage, lifecycle, refusal and terminal meaning are native declarations too.
  The std realization consumes those generated values and will execute only
  through a current planned host/boot/provider, exact executable
  identity/version/content contract, resource pool and authority grant matched
  to trusted host-local executable state; the OS path remains provider-local
  machinery rather than portable identity.

- **Record laws (#4638):** native record types own pure Boolean `where` laws,
  include them in checked identity, and enforce them at generated construction
  and decode boundaries. General propagation into consuming-plot arithmetic
  proofs and erasure of proved-safe checks remains open in
  [#4639](https://github.com/dancxjo/conduit/issues/4639).

- **Conduitese terminal contracts (#4109):** a Fore port retains its ordinary
  value kind, temporal modality, and optional exact abnormal-terminal kind as
  independent checked truth. Normal close, abnormal termination, and semantic
  cancellation have separate transduction contracts through Plot expansion,
  Plan identity, Back preparation, local and remote Cords, and kernel execution.
  Unhandled abnormal truth reaches one exact containing-Plot boundary; typed
  recovery must actually complete before that obligation is discharged.
  `gear~` is admitted only for a declared cancellation control, and deadline
  composition preserves the distinction between request, observed disposition,
  and scheduler cancellation. Named type parameters, `with` imports, body wardrobe, and `resource T`
  have canonical authored surfaces documented in the
  [language reference](wiki/Current-language-surface.md).

- **Three-Body semantic journey (#3529):** ConduitOS, pinned Chromium/WASM,
  and a hosted generative Tongues Mask produce independent 15-action tracks for
  one shared tutorial contract. Each track carries its own exact Body, Host,
  Boot, Plan, Play, Face revision, Show, and lifecycle signs; the
  ConduitOS track additionally proves an admitted two-host Line and distributed
  Plan. Release promotion retains the native and browser producer evidence but
  neither creates the hosted generative documentary track, makes
  gallery-only evidence, nor requires or renders the documentary. After the
  accepted software carrier is deployed, a separate downstream workflow runs
  the deterministic hosted generative producer, creates the optional gallery
  evidence, verifies the three tracks, and may stage the resulting browsable
  collection for Pages. The required
  provider boundary crosses real HTTP through a deterministic Ollama-compatible fixture;
  separately retained real Ollama/Gemma conformance proves actual inference
  without making runner/model latency a release condition. Until that exact
  candidate is accepted and published, this is development-tree and local
  execution proof, not a stable-release claim.

  This retained evidence does not yet establish the canonical repaired
  Three-Bodies claim: one ordered action journey looped over three materially
  different embodiments, each yielding honest comparable evidence. That
  human-facing repair remains open. Retained real-model recordings remain
  historical evidence only; they never substitute for a current producer track
  from the exact source commit. Offline documentary voicing is not runtime
  speech or physical-speaker proof, and retained historical evidence keeps the
  mechanism it actually recorded.

- **Runtime speech artifacts:** an explicitly selected local eSpeak NG provider
  realizes `speech/synthesize` through ordinary checked plots, planning, and
  kernel Host Calls. Its executable, engine library, voice data, resource, and
  process authority are bound before execution. A spoken Mask can retain its
  actual WAV and correlate its PCM digest with the acknowledged Show. The
  existing text and PCM bounds still apply; oversized speech refuses rather
  than truncating. This is produced-audio evidence, not speaker playback,
  human hearing, or a refreshed Three Bodies documentary. Separately, an
  opt-in installed screen-free path has drained a multi-chunk reading through
  a selected ALSA device without underrun; that is device playback evidence,
  not attended listening or an owner-selected spoken wardrobe route. See
  [runtime speech](docs/proof/runtime-speech.md) for the supported proof entrance.

- **Startup plots (#3152):** the optional browser Startup Chime and separate
  First wake Chime run through the ordinary planner and kernel. The generic
  first-wake source uses retained body biography across later wakes and fresh
  boots; it has no chime-specific flag. Browser proof covers sound-only idle,
  denied/unavailable playback, saving Started before effects, and retained
  installation/removal. See [Startup Chime](plots/startup-chime/README.md) for
  the exact lifetime and runnable proof. Deterministic synthesis and browser
  execution do not establish physical speaker or subjective listening acceptance.

- **body continuity and supporting contracts:** surviving-part continuity,
  atomic workload transition, typed failure disposition, bounded resource
  collections, and causal evidence have dedicated implementation and tests.
  See [body lifecycle boundaries](docs/architecture/body-lifecycle-waists.md).
- **Portable bounded navigation (#2232):** finite goal, pose, traversability,
  route, trajectory, and local-control contracts compose as one ordinary plot.
  The std host runs that plot through the production planner and kernel to emit
  an expiring portable body-motion request. Deterministic routing and control do
  not claim Pete/Create attachment, physical movement, or attended safe-stop
  proof; those belong to the continuous Pete capstone in #2234.
- **ConduitOS compositor:** retained surfaces, damage, and input routing exist
  under [the native compositor](targets/conduitos/src/native_compositor.rs).
  The x86 product route renders the current Face and its Patchbay graph; its
  QMP journey verifier checks the ordered keyboard actions and actual screens.
  The broader shell proof and interaction polish remain tracked in
  [#3043–#3049](docs/roadmap.md#conduitos-shell).
- **Retained chapter corpus (#3359):** the seven-chapter Tour model and its
  focused exercise proofs remain available as historical implementation and
  conformance material. They are not the current ConduitOS product journey and
  do not prove the browser Workspace's living Tutorial. Native Workspace parity
  must honor the canonical
  `application = "tutorial"` configuration, project actual Body biography,
  and expose the same lifecycle actions. Zero-Body arrival and user-driven
  Birth now have an installed screen-free path; completing the remaining
  screen-free journey remains part of
  [#4807](https://github.com/dancxjo/conduit/issues/4807). Produced speech alone
  does not establish interactive accessibility.
- **House speech:** recorded-audio recognition, address detection, model context,
  and conversation components exist. The attended, live named-house speech
  experience remains open in [#2297](https://github.com/dancxjo/conduit/issues/2297).
  Recorded PCM and a model response do not prove a microphone-to-speaker household.
- **body Chat:** the checked `body-chat` plot composes portable text interaction,
  bounded Conduit-owned history, a canonical current-body projection, and a
  replaceable stateless model flow. WebSocket remains an explicit Webchat
  adapter rather than chat semantics. This source-level slice does not yet
  establish the five-host, voice, or ConduitOS experiences described in the
  [body Chat guide](plots/body-chat/README.md).
- **Learned realization lifecycle (#3709):** checkpoint-bearing hosted model
  execution, bounded effect-free shadow comparison, explicit evaluation and
  operator promotion/rollback authority, ordinary replacement-Plan selection,
  and Patchbay lifecycle projection exist as shared contracts. A Pete-scoped
  deterministic conformance fixture exercises a harmless interpretation
  candidate and return to its retained baseline without granting wheel effects.
  This is deterministic source-level proof; it does not claim autonomous model
  improvement, physical shadow evidence, or an attended operator decision.
- **Reusable applications:** plots-as-gears is implemented, while several
  complete reusable application compositions still have open acceptance work.
  [The roadmap](docs/roadmap.md#reusable-plots) names those remaining slices.

The current `cargo xtask make conduitos std-gap` report also identifies missing
ConduitOS Host Calls for `math/map-quantity` and
`structured-info/wrap-quantity`, alongside the missing storage base for
`file/copy`. Run the report for the current profile instead of relying on a
frozen catalog count.

## What is missing

The major remaining gaps are product integration and breadth of real-world
proof, as well as specific architectural work:

- Complete durable keep/data realization across Wake, Boot, and Body lifetimes,
  including insufficient-durability refusal, hard-loss recovery, and exact
  disk-residence proof. Typed immutable save/load alone does not prove those
  lifecycle promises.
- Make the graphical ConduitOS shell a usable set of independently retained
  surfaces with inspection, transients, resizing, scrolling, and clear focus.
- boot and qualify a real old laptop, then implement its storage, network,
  audio, input, and house integration in the staged hardware campaign.
- Complete a persistent multi-host House with live speech and supported
  smart-home adapters. House admission work is currently marked paused.
- Complete reusable application capstones and the larger Pete robotics body;
  deterministic robot motion selection is not physical motion proof.
- Extend implementation confinement where required. Cooperative admission
  and recorded authority are not proof of hostile-code isolation.

See the [roadmap](docs/roadmap.md) for issue owners, dependencies, and paused
work. These are not prerequisites for contributing a small, useful change.

## Reading proof correctly

| Evidence | What it establishes |
|---|---|
| Contract tests and deterministic simulation | Identities, bounds, transitions, and refusal behavior in the tested model |
| Hosted or browser execution | The actual implementation running in that environment |
| Emulator execution | A freestanding image running against the specified emulated machine |
| Live transport | Actual messages crossing the named transport/session |
| Physical/HIL evidence | The named firmware and device behavior under the recorded physical conditions |
| Human enactment | A person completed the specified interaction; automation cannot supply this claim |
| Released-product proof | An exact accepted commit crossed the protected release boundary; it does not upgrade browser, emulator, physical/HIL, or human evidence |

Use [current product truth](https://dancxjo.github.io/conduit/current-product.html)
for current identities and receipts, [visual evidence](docs/visual-evidence.md)
for screenshots with provenance, and the [proof-class reference](docs/architecture/proof-classes.md)
for precise definitions. This documentation review creates no new boot,
browser, physical, or human acceptance receipt.

CI now separates inexpensive PR admission, combined `dev` integration, and
exhaustive release proof. The current procedure is in the
[CI guide](docs/contributing/ci.md); old workflow names in historical receipts
are not commands to reproduce the present gate.
