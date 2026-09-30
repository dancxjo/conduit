# Current project status

Reviewed against the development tree and open issues on **28 September 2026**.
This is a capability summary, not a claim that every check has been rerun today.
The [current-product truth surface](https://dancxjo.github.io/conduit/current-product.html)
shows the exact latest development commit, accepted release, published Pages
artifact, lag, and proof receipts. Published products follow accepted releases
and can lag development. The [recorded acceptance history](docs/history/accepted-milestones.md)
preserves older proof receipts and limitations.

**[Watch the current ConduitOS visual journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)**
for the clearest demonstration of the system so far.

## What exists

| Area | Available behavior and evidence | Boundary |
|---|---|---|
| **Language and composition** | Canonical `.conduit` parsing, located diagnostics, checking, recursive Forms through Fores/Backs, exact typed ports, and separate source/checked/expanded identities. Ordinary examples run through planning and the production kernel. | Supported semantics and implementations are finite; a catalog entry does not promise an implementation on every Host. See [forms](forms/README.md). |
| **Planning and execution** | One port-aware kernel, explicit fan-out, bounded queues and Host Calls, plan-owned per-Step fuel, fair cooperative yielding, resource and authority admission, immutable plans, pressure, and correlated signs. Exact abnormal-terminal kinds, separate terminal-transduction profiles, containing-Form propagation, typed semantic cancellation requests, and cancellation-driven deadlines remain checked and planned through execution. Shared code serves hosted, browser, and embedded paths. Confined Wasm instruction fuel forcibly returns control even from a non-cooperative infinite loop. | A cancellation request is not proof of cancellation, and successful fallback does not manufacture semantic abnormal truth. Native in-process, browser, ConduitOS, and firmware Backs remain cooperative unless their exact Host profile names and proves a hard containment mechanism. General SMP, preemption, and physical real-time guarantees are not established by cooperative execution. |
| **body lifecycle** | Zero/one/many initial forms, durable part membership, current/offline host presence, body-wide workload admission, one current plan and at most one active play. Multi-form execution and workload changes have hosted and browser evidence. Live forms treat structural drain as quiescence; a canonical trailing full stop makes that drain semantic completion. | This is implemented, not merely the old #2062 proposal. Durable keep realization across Wake, Boot, and Body loss remains part of the open persistence vertical rather than an implicit lifecycle promise. |
| **Face and browser Masks** | One bounded renderer-neutral Face grammar carries exact basis, subjects, relationships, typed content, wording, actions, inputs, semantic order, context, and provenance. Ordinary Forms contribute Face truth; ordinary Forms serving the Mask role realize exact Face revisions as Shows through deterministic-linear and browser routes. | The Rust carrier type is still named `Presentation` during migration. Richer authored Body/wardrobe source syntax remains deliberately unfrozen. Browser permission, device availability, tab lifetime, and supported profile still apply; browser, hosted, and emulator proofs establish different environments. |
| **Patchbay** | A resident native projection shows the active forms on the present body with exact plan/play identities and Mask topology; broader native and browser inspection/editing, bounded Watches, observation replay, and scoped breakpoint/causal-trace support also exist. | Patchbay is a projection over authoritative body and execution truth, not a second scheduler. Replay is distinct from re-execution, and these features do not establish distributed stop-the-world debugging. |
| **Crèche and fabrication** | Birth a body with reviewed forms, prepare target-native artifacts, inspect membership, and retain body evidence into Patchbay. Fabrication packages cover hosted computers, browser, ConduitOS, and board families. | Building or downloading an artifact is distinct from installing, booting, admitting a part, and executing work. Consult each [target](targets/README.md). |
| **ConduitOS** | Five product targets: x86_64 and IA-32 PC, AArch64 and RISC-V64 virt, and LoongArch64 virt. The x86_64 graphical journey births a body, exercises the retained seven-chapter corpus, switches resident forms, opens simplified Patchbay, replans Masks, and exercises keyboard, pointer, timer, and USB line paths; the other four targets have serial product media. | The illustrated journey is **freestanding-emulator** proof. The physical laptop campaign is open. A target's boot proof does not establish graphics, drivers, or hardware parity. |
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

- **Native semantic Types and compact representations (#4382, #4428):**
  Conduitese owns nominal scalar, record, variant, optional, data-reference,
  bounded-sequence, and refined primitive meaning. Rust bindings are generated
  from the checked graph. A named compact `u8` representation derives iota
  tags from authored variant order, typed malformed-input refusal, finite
  extent/work bounds, and a mechanical compatibility identity. The Data Text
  terminal family has no handwritten Rust tag table, and generated Rust plus
  ECMAScript consume the same checked declaration. Broader semantic algebra
  and migration of the audited handwritten domain codecs remain open under
  #4431 and #4382.
  HTTP scheme, transaction identity, target and header meaning are likewise
  native declarations; hosted and ConduitOS HTTP code consumes generated
  bindings while its HTTP/1.1 bytes remain an external adapter contract.

- **Conduitese terminal contracts (#4109):** a Fore port retains its ordinary
  value kind, temporal modality, and optional exact abnormal-terminal kind as
  independent checked truth. Normal close, abnormal termination, and semantic
  cancellation have separate transduction contracts through Form expansion,
  Plan identity, Back preparation, local and remote Cords, and kernel execution.
  Unhandled abnormal truth reaches one exact containing-Form boundary; typed
  recovery must actually complete before that obligation is discharged.
  `gear~` is admitted only for a declared cancellation control, and deadline
  composition preserves the distinction between request, observed disposition,
  and scheduler cancellation. Generic parameter spelling, import syntax,
  richer Body/wardrobe source, and authored resource-type spelling remain
  deliberately unfrozen rather than being supplied by private fixture syntax.

- **Three-Body semantic journey (#3529):** ConduitOS, pinned Chromium/WASM,
  and a hosted generative Mask currently produce independent 13-step tracks for
  one shared tutorial contract. Each track carries its own exact Body, Host,
  Boot, Plan, Play, Face revision, Show, and lifecycle signs; the
  ConduitOS track additionally proves an admitted two-host Line and distributed
  Plan. Release promotion retains the native and browser producer evidence but
  neither creates the hosted generative documentary track, fabricates
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

- **Startup forms (#3152):** the optional browser Startup Chime and separate
  First wake Chime run through the ordinary planner and kernel. The generic
  first-wake source uses retained body biography across later wakes and fresh
  boots; it has no chime-specific flag. Browser proof covers sound-only idle,
  denied/unavailable playback, saving Started before effects, and retained
  installation/removal. See [Startup Chime](forms/startup-chime/README.md) for
  the exact lifetime and runnable proof. Deterministic synthesis and browser
  execution do not establish physical speaker or subjective listening acceptance.

- **body continuity and supporting contracts:** surviving-part continuity,
  atomic workload transition, typed failure disposition, bounded resource
  collections, and causal evidence have dedicated implementation and tests.
  See [body lifecycle boundaries](docs/architecture/body-lifecycle-waists.md).
- **Portable bounded navigation (#2232):** finite goal, pose, traversability,
  route, trajectory, and local-control contracts compose as one ordinary form.
  The std host runs that form through the production planner and kernel to emit
  an expiring portable body-motion request. Deterministic routing and control do
  not claim Pete/Create attachment, physical movement, or attended safe-stop
  proof; those belong to the continuous Pete capstone in #2234.
- **ConduitOS compositor:** retained surfaces, damage, and input routing exist
  under [the native compositor](targets/conduitos/src/native_compositor.rs).
  Tour now reports distinct retained workspace/status surfaces, and its journey
  verifier checks their identities. The broader shell proof and interaction
  polish remain tracked in [#3043–#3049](docs/roadmap.md#conduitos-shell).
- **Shared hosted/native chapter corpus (#3359):** the development tree carries
  one portable seven-chapter Form corpus through browser, Linux, Windows, and
  ConduitOS Masks. Deterministic model tests and the pinned local
  x86_64 QEMU journey cover every current page and runnable exercise, including
  direct/recursive comparison, standing timer lifecycle, explicit fan-out, and
  two-host execution. This source-level record does not claim accepted release,
  physical hardware, or unattended human usability evidence.
- **House speech:** recorded-audio recognition, address detection, model context,
  and conversation components exist. The attended, live named-house speech
  experience remains open in [#2297](https://github.com/dancxjo/conduit/issues/2297).
  Recorded PCM and a model response do not prove a microphone-to-speaker household.
- **body Chat:** the checked `body-chat` form composes portable text interaction,
  bounded Conduit-owned history, a canonical current-body projection, and a
  replaceable stateless model flow. WebSocket remains an explicit Webchat
  adapter rather than chat semantics. This source-level slice does not yet
  establish the five-host, voice, or ConduitOS experiences described in the
  [body Chat guide](forms/body-chat/README.md).
- **Learned realization lifecycle (#3709):** checkpoint-bearing hosted model
  execution, bounded effect-free shadow comparison, explicit evaluation and
  operator promotion/rollback authority, ordinary replacement-Plan selection,
  and Patchbay lifecycle projection exist as shared contracts. A Pete-scoped
  deterministic conformance fixture exercises a harmless interpretation
  candidate and return to its retained baseline without granting wheel effects.
  This is deterministic source-level proof; it does not claim autonomous model
  improvement, physical shadow evidence, or an attended operator decision.
- **Reusable applications:** forms-as-gears is implemented, while several
  complete reusable application compositions still have open acceptance work.
  [The roadmap](docs/roadmap.md#reusable-forms) names those remaining slices.

The current `cargo xtask fabricate conduitos std-gap` report also identifies missing
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
- Repair Three Bodies as one ordered action journey across three materially
  different embodiments with honest, comparable Face/Show evidence.
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
