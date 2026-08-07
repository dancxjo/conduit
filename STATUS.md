# Conduit salvage status

This file is the repository-level claim boundary. A green check proves only the
rows and commands named here; it does not promote a simulation into a platform
adapter or physical proof.

| Surface | Contract | Simulation | Executable hosted implementation | Actual browser adapter | Actual firmware | Live transport | Physical/HIL proof |
|---|---:|---:|---:|---:|---:|---:|---:|
| Port-aware salvage kernel | port protocol, fixed scheduler, Operation adapter, retained state, node-scoped correlated host operations, cancellation, and exact local/remote-plan numeric lowering | fixed/hosted full adapter form, atomic join, real latest, host-lifecycle, cancellation, exact signal/multi-value lowering, remote pressure/delivery, and zero-allocation activation vectors | same scheduler with preallocated hosted storage; installed Signal pair, local and remote three-target fan-out, typed multi-value std profiles, distributed Signal, and admitted activation/toggle sources execute through it; unsupported forms fail closed without a production legacy pump | browser/WASM Signal and toggle sinks execute exact remote-ingress fragments through the same scheduler before bounded DOM presentation | Pico W firmware consumes generated fixed kernel tables for exact local, std→Pico, and final three-host fragments; its active kernel/transport storage remains finite | bounded loopback WebSocket and physical USB CDC remote cords; providers own carrier I/O only | exact Pico-local, std↔Pico, and final std/browser/Pico board runs recorded |
| Exact plan, play, evidence, and presentation identities | S2 planning plus S3/S4 runtime identity acceptance: separate source/checked/expanded/plan types; boot-scoped active-play issuance; host-issued evidence identities; exact active-play/presentation correlation at platform and remote-cord boundaries | semantic/spelling, cycle, mutation/resealed-lie, host-operation admission/bounds, resource reservation/release, authority/link denial, observation-overflow, boot/activation mutation, unique evidence, wrong-presentation identity, wrong session identity, generated-image mutation, firmware-build mismatch, and runtime-identity mutation vectors | yes, std preparation enforces S2 truth and distributed sources bind exact plan/fragment/play/link/connection identities | browser sinks independently reconstruct and lower exact fragments and reject stale/wrong session facts | generated image and manifest bind source/checked/expanded/plan/fragment identities and clean firmware build identity; runtime-generated boot/play plus presentation/evidence identities are carried in USB records and checked across physical sessions | live sessions verify exact provider instance, endpoints, limits, host/boot, fragments, plays, connection, and value kind | matching physical receipts retain exact plan/fragment/play/presentation/evidence/link/provider/boot/build identity |
| Lossless form and composite boundary | S3 plus #398/#399 corrections: exact source, bounded lossless CST, located diagnostics, inline checked forms, recursively bound expansion identity, and checked named input/output faces with exact endpoint, value-kind, direction, and independent-terminal contracts | round-trip/recovery/limits, expansion and face mutation denial, standalone/nested equality, two-input/two-output typed control/data execution, input-only/output-only planning, exact pressure/retry, independent closure, cancellation/failure, parent terminal evidence, and topology hiding | parser/checker and planner are general for the checked face contract; the hosted composite compatibility façade routes exact named ports atomically while production std execution remains on `conduit-kernel` | no | no | fixture in-memory links only; face mappings are not transport | no |
| Portable planner capability | optional boot-scoped planner profile/limit offers are part of the `no_std` host advertisement contract; planner identity and scratch state are excluded from plan construction and identity | full and browser-profile equivalence across different planner host/boot identities, bounded pre-planning refusal, missing/ambiguous-offer denial, and a non-planner Pico target with a verified lowerable fragment | the standard reference host advertises `conduit.planner/full@1` and invokes the shared deterministic planner; non-planner hosts remain ordinary complete targets | the actual browser/WASM host advertises `conduit.planner/browser-wasm@1` and plans locally in its WASM start path before lowering and kernel execution | Pico advertises no planner profile; existing generated firmware consumes its externally planned exact fragment, with no general Pico planner claim | no new transport or delegation service | no new physical/HIL claim |
| Connection envelope and session wire formats | allocating fixture envelope plus allocation-stable borrowed exact session protocol; framed sessions preserve provider-specific exact identity and bounds | deterministic envelope corpus and session lifecycle/mutation/provider-eligibility vectors | native binary-only RFC 6455 and USB CDC providers with fixed frame/buffer bounds | real browser WebSocket API with one-message inbox and explicit send bounds | dual-CDC Pico carrier keeps session frames on CDC0 and evidence on CDC1 | actual loopback WebSocket plus physical USB CDC; fixture providers remain synthetic conformance only | reciprocal physical Hello/Ready/value/pressure/failure/terminal lifecycles recorded |
| Portable Signal | yes | multi-value fixtures | one std kernel pulse source atomically fans each value to stdout plus two exact remote egresses | browser/WASM kernel show sink with sixteen exact DOM receipts | unchanged Signal forms generate exact local and remote Pico images that drive the CYW43 LED | sixteen ordered values over bounded WebSocket and USB CDC remote cords | sixteen matching ordered stdout, DOM, and physical LED receipts from one exact three-host run |
| Browser manifestation | local and remote-ingress Signal profiles | yes, `conduit-browser-sim` | actual Rust/WASM planner plus exact-plan-lowered `conduit-kernel` execution for local and distributed sink fragments | thin DOM adapter with exact fixed-frame completion correlation and sixteen receipts | no | actual loopback WebSocket to the std kernel source | included in the accepted three-host physical run with matching cross-host receipts |
| Interactive activation/toggle | typed `interaction/activate -> state/toggle -> presentation/show` contract with admitted input and exact remote planning | deterministic activation/toggle lifecycle and identity negatives | native std source services one admitted stdin activation through the kernel before realizing the corresponding remote offer | pinned Chromium proves the first Enter causes exactly one sequence-0 DOM update before later inputs, then completes sixteen exact presentations with one real pressure retry | no | actual bounded loopback WebSocket with structured link-break failure | no |
| Pico-shaped manifestation | exact Pico-local and remote-ingress advertisements with reviewed fixed-image bounds | yes, `conduit-pico-sim` | host-side unchanged-form planning/lowering/image generation, exact std source, and verifier tests | no | RP2040 images generated from exact local/remote fragments, CYW43 GPIO 0 LED driver, pinned radio assets, clean firmware-build identity, runtime boot/play receipt identity, and bounded dual CDC | exact bounded std↔Pico USB CDC and final three-host sessions | recorded local, exact std↔Pico success/failure, and final three-host success/broken-link runs |
| Realm membership | retired prototype | deterministic table tests | no production body model | no | no | no | no |
| Observatory | versioned neutral host/capability/link/plan/Play/pressure/evidence/retention reports with exact identity and bound validation | synthetic fleet retained only as an explicitly labeled integration test | actual std execution can write a bounded report artifact; the read-only `observatory-report` command validates and renders complete structured tables without runtime control | no browser UI or browser-owned runtime truth | no firmware-side inspector or report store | no new transport; observed links are report facts only | no new physical/HIL claim |
| `conduit.std` | prototype contracts | one-value demonstrations | incomplete semantics | no | no | no | no |
| Copy a file | unsafe prototype disabled | tests removed from default tree | no admitted host operation | no chooser | no | no | no |

## Required CI claims

The `check` workflow requires:

- workspace formatting, Clippy, and tests;
- no-std checks for the salvage kernel, semantic, wire, and std-catalog contracts;
- hosted/fixed salvage-kernel protocol, storage, scheduler, pressure, atomic
  join, retained-state/latest, host lifecycle, closure, and cancellation vectors;
- exact semantic-contract/profile/port, host-operation/resource/authority/link, and
  policy/budget planning with cycle, mutation-negative, action/completion and
  authority/link admission, reservation/release, and executable
  mandatory-evidence storage tests;
- optional portable planner-profile advertisements with exact finite input
  limits, equivalent full/browser planning across distinct planner identities,
  bounded refusal without delegation, and browser-local WASM planning;
- exact local signal-plan lowering into numeric kernel node, directional port,
  cord, route, host-operation, resource, and evidence tables, with fail-closed
  mutation, fan-in, concurrency, remote-link, and capacity boundaries;
- real std-host execution of the exact signal pair, local three-sink fan-out,
  and typed multi-value profile through installed tables, with virtual
  timer/stdout completions, node-scoped request identity, exact
  play/presentation correlation, and measured zero-allocation activation;
- bounded lossless form-source/CST round trips, located recoverable diagnostics,
  source/checked/expanded identity separation, named face checking, typed
  multi-face and zero-sided contracts, and face mutation rejection;
- boot-scoped active-play issuance, runtime-issued evidence identities, exact
  presentation completion correlation, and Observatory identity projection;
- named composite face planning/execution with two value kinds, exact
  parent-to-child input/output routing, retry pressure, independent closure,
  cancellation/failure translation, terminal evidence, and mutation denial;
- deterministic wire and simulated-host conformance vectors;
- carrier-neutral framed-session eligibility vectors proving `FixtureFrame` and
  `WebSocket` preserve exact provider/provider-instance/link/endpoint identity,
  while `Local`, `InMemory`, and `FixtureDatagram` remain invalid for the exact
  remote session contract;
- one actual Chromium browser-local kernel proof with two independent WASM
  instances, exact source/checked/expanded/plan/fragment/play/request/
  presentation/evidence identities, stable sealed capacity, sixteen ordered
  receipts per host, and bounded failure negatives;
- one actual Chromium distributed kernel proof from the unchanged Signal form,
  with exact std-source/browser-sink fragments, kernel execution on both ends,
  one binary loopback WebSocket, sixteen ordered DOM receipts, complete session
  identity, one real receiver-`Full` retry of the same sequence, terminal
  evidence, stable sealed capacities, zero retained/in-flight values, and
  bounded lifecycle/identity/frame failure negatives;
- exact final-three-host planning and source-kernel vectors proving one atomic
  stdout/WebSocket/USB-CDC fan-out with fixed item/byte/frame bounds, exact
  session identities, stable capacity, reciprocal terminal state, and
  fail-closed missing capability, stale boot/provider, malformed-frame, and
  browser-sink cases; the attached-board Playwright cases remain explicitly
  hardware-gated and do not run in ordinary CI;
- one actual Chromium distributed activation/toggle proof in which the first
  admitted Enter produces exactly one sequence-0 DOM update before any later
  activation is sent, followed by the complete sixteen-value terminal path,
  one real pressure retry, exact receipt correlation, and structured link-break
  failure;
- direct unchanged `examples/signal-demo.form` to Pico-local plan, selected
  fragment, lowered fragment, and generated fixed-image conformance with exact
  reviewed bounds and fail-closed identity/lowering/remote-connection negatives;
- Pico verifier tests rejecting static identity mutation, firmware-build
  mismatch, missing/reused/changed runtime boot/play identity, reordered
  receipts, and invalid terminal evidence;
- WASM compilation of the browser-shaped simulation;
- Thumb compilation of allocator-free contracts, the Pico-shaped simulation,
  and the real generated Pico W firmware package.

WASM compilation is not browser execution by itself. Thumb compilation proves
that the firmware builds; it is not board execution or physical acceptance. A
generated fixed image and a valid USB verifier are also not a board transcript.
The previously accepted Chromium proof is browser-local and not a live link; the
suite also includes narrow live loopback std-to-browser Signal and toggle links.
Those links are not public networks, TLS, discovery, reconnection, or general
transport claims. A carrier-neutral session contract is likewise not a new
carrier implementation. Frame/datagram fixtures are not WebSocket or UDP
sockets.

## Salvage stop line

S0 restores truth. S1 now includes the port-aware protocol, bounded storage,
deterministic fixed-capacity scheduling, transactional fanout, per-port closure,
correlated bounded host operations, late-completion rejection, and matching
hosted/fixed lifecycle vectors. The capacity-one conformance graph now includes
a stateful latest operation, atomic joins, and a stable hosted allocation shape.
The published `OperationInput`/`OperationAction` contract runs the complete
four-value tick/tee/filter/latest/show graph through the same fixed-capacity
scheduler in fixed and hosted profiles. S1 is accepted. The first S2 slice
removes `CapabilityLimits.value_kind` and binds exact semantic contract
revisions, execution profiles, and complete per-port contracts through form
identity, planning, preparation, and Observatory projection. Source-document,
checked-form, and expanded-form identities are now distinct and all participate
in fragment and plan identity. Startup dependencies, cancellation and terminal
policies, mandatory evidence, and independent evidence item/byte budgets are
also sealed and validated during preparation. The hosted reboot runtime now
allocates fixed mandatory-evidence slots from that plan and preserves them
independently of its lossy observation ring. Installed local `PlanFragment`
profiles lower before activation into numeric kernel nodes, directional ports,
cords, direct route ranges, host-operation admission, resource references,
evidence targets, exact cord queue budgets, and mandatory-evidence budgets. The
mapping is reversible to plan identities and rejects unsupported remote links,
fan-in, port widths, and host-operation concurrency. The exact two-node
`flow/pulse -> presentation/show` profile, its three-sink local fan-out, and the
typed multi-value conformance form install those tables into hosted kernel
schedulers, drive virtual/thread timers and stdout presentation completions,
and project host-issued active play, presentation, and evidence identities.
Their resources and allocation shapes are sealed before activation; allocator
probes measure zero successful-path allocations. Unsupported production std
forms fail closed, and `StdHost` has no independent operation/connection pump.
General graph installation remains open. Hosted host-operation
requirements now bind exact contract and target identity plus concurrency and
byte bounds through capability, plan, installed implementation, and
effect/completion admission; their S1 numeric lowering exists only for the
admitted concurrency-one checkpoint. Boot-scoped resource pools now bind
semantic class, finite units, and an exact selected pool through planning and
hosted reservation until release; availability is explicitly not authority,
and numeric resource references now lower without changing hosted reservation
semantics. Authority grants enter
planning independently of advertisements,
bind exact host/boot/capability/action/subject scope, and fail closed during
preparation and effect admission; expiry/revocation/delegation and S1 lowering
remain open. Remote cords now require exact observed directional host/boot and
provider-endpoint scope, one initialized provider instance, explicit
credential/authority references, readiness, and finite limits; the legacy
compatibility runtime revalidates the same current observation before
preparation. S2 is accepted at the plan boundary, and the installed std
profiles now cross that boundary through S1.
The first S3 checkpoint wraps the existing small `form 0` checker in a bounded
lossless source document: exact source and CST tokens survive invalid edits,
diagnostics carry stable codes and UTF-8 byte/line/column spans, and no checked
form is manufactured after an error. This does not restore the archived broad
grammar. The second checkpoint now derives one exact composite contract
revision and its ports from explicit named input/output faces. Every face maps
directly to one checked internal sink/source endpoint and carries its own
direction, value kind, and independent-terminal contract; no boundary is
inferred from an internal cord. Two-input/two-output multi-kind, input-only,
and output-only exports check as ordinary parent kinds, while duplicate or
mutated faces fail closed. The hosted composite offer and parent catalog
consume that same boundary. Parent-to-child hosted value/closure/pressure
routing now crosses every face through the compatibility composite host. The
hosted operation façade retains input port identity and admits each atomic set
of named output values without broadcast; production std execution remains on
the port-aware kernel. The third checkpoint adds `operation: capability { ... }` inline
nesting with a hard depth limit. The child is an ordinary checked form whose
selected authored export becomes the parent operation; standalone and nested
spellings produce the same checked/expanded child and boundary identities, and
inner errors remain located in the lossless outer document. Parent checked
identity binds only the visible exported contract; parent expanded identity
recursively binds canonical operation paths, selected exports, and child
expanded identities. Omitted, duplicated, reordered, or substituted expansion
rows fail before planning.
The final S3 checkpoint keeps plan, active play, evidence, and presentation as
different core identity types. Activation sequences are host/boot scoped;
evidence IDs are issued by the recording host rather than synthesized from UI
row indexes; and adapters must return the exact active-play and presentation
IDs carried by each effect. S3 is accepted at this boundary.
The S4 browser-local kernel checkpoint is accepted at exact main
`b7852eed1e784a27dcd78e700b2f89ddc01bc097`; workflow `31022565054` passed
both the full Rust gate and the pinned Chromium job. Two independent WASM
instances parse and plan unchanged `examples/signal-demo.form`, lower their
exact local fragments through the shared contract, and execute through
`conduit-kernel`. JavaScript remains the real-timer/DOM adapter. Fixed frames,
exact completion correlation, item/byte limits, duplicate/malformed/wrong
identity denial, cancellation, evidence exhaustion, terminal failure, and
stable sealed capacity are executable proofs. WASM allocation is not claimed
to be measured; the accepted claim is precise capacity stability.

The S4 live std-to-browser Signal checkpoint is accepted at exact main
`a1f479dfa58b8537427b5747da73795628504913`; workflow `31031406945` passed
both the full Rust gate and the pinned Chromium job. The unchanged
`examples/signal-demo.form` lowers into exact std-source and browser-sink
fragments. Both execute through `conduit-kernel`; a binary-only loopback RFC
6455 provider carries the remote-cord session without owning scheduling or
value lifecycle. Sixteen values reach the DOM in order through one-item and
nine-byte cord/buffer limits, including one observed receiver-`Full` response
and exact same-sequence retry. Both kernels terminate with evidence, capacities
remain stable, and no value remains retained or in flight. Missing/stale links,
identity mutation, malformed/truncated/oversized/trailing frames,
duplicate/reordered sequence, early disconnect, sink failure, cancellation,
late acknowledgement, and evidence exhaustion fail closed.

The corrected S4 interactive std-to-browser toggle checkpoint is accepted from
PR #432 exact head `d5d95fbeba8e373e157d4759fa1912ad4a414a82`; workflow
`31062645805` passed. The native source admits at most one stdin activation per
source offer cycle and realizes its remote offer before reading another. Pinned
Chromium proves the first Enter produces exactly one sequence-0 toggle-on DOM
manifestation before the remaining fifteen inputs are sent. The full path then
completes sixteen exact presentations with unique request/presentation/evidence
identity, one real receiver-pressure retry, terminal agreement, stable bounded
capacity, and a structured four-receipt link-break failure. PR #432 merged as
`0a99f4d75a2ef38cb63dcae474288b3eca429e94`.

The Pico-local code path is now generated from the unchanged portable form.
PR #426 exact head `2f5fa237f5e246ae0d8b38438e64b9c3b83572ce` passed workflow
`31057256898` and merged as `fb5be830f3a77cb99a491813a3b6d5f3138d7b1b`.
Its firmware build parses `examples/signal-demo.form`, plans both operations
onto the Pico-local advertisement, lowers the selected fragment, and emits the
allocator-free fixed image consumed by the RP2040 firmware. Hand-authored
firmware topology/configuration ordinals are no longer the execution source.
The generated image and verifier carry source, checked, expanded, plan,
fragment, presentation, and evidence identities.

PR #430 exact head `ad624fed772f6f2166ef2c5f2e30cc7843d11aad` passed workflow
`31061627861` and merged as `ddc54b7073928169bc65b74af9f58bc3a1d7594d`.
It binds the deterministic generated-image sidecar, firmware manifest, USB boot,
presentation, terminal, and failure records to one `firmware_build_id`; the
verifier rejects manifest/transcript mismatch. PR #431 exact head
`10c9c430ae1b268a0a320f559f534baa44314864` passed workflow `31062057535`
and merged as `4fa0c8cb84f1c330c4c823c8a8d1f9da354c4913`. It adds runtime-generated
boot and active-play identities and requires them to remain nonempty, distinct
from fixed image identities, and stable throughout one transcript.

PR #433 exact head `ebfecafd277df3d2f649f13a917207cb536ebe3a` passed workflow
`31063155417` and merged as `8032c81bb00cfba0a4d03a7bcdcbb45bf22a1afc`.
It directly proves the unchanged Signal form through checked form, Pico-local
plan, selected `PlanFragment`, `LoweredPlanFragment`, and
`GeneratedEmbeddedPlan` under the same reviewed bounds used by firmware, with
fail-closed negatives for mismatched fragment/lowering identity, unsupported
remote cords, and range/bound overflow.

Direct commit `aeb6ecbe60edd09ef1ecc516a2e399a904113143` removes the synthetic
Observatory/Realm/browser/Pico fixture from the production `conduct` executable
and retains it only as a dedicated integration test with dev-scoped
dependencies. No PR-triggered exact-head workflow is attached to that direct
cleanup commit, so it is merged repository state but is not a new S4 acceptance
verdict.

PR #436 exact head `f911c1d8007608b1db5fece731a998e42a085c28` passed workflow
`31137116246` and merged as current main
`82fec9f1b65ff537148244698cd16744416ce8dc`. The exact framed session contract
is now carrier-neutral at the semantic boundary: `FixtureFrame` and
`WebSocket` are eligible providers, while `Local`, `InMemory`, and
`FixtureDatagram` remain invalid. Provider identity, provider-instance identity,
link binding, host/boot/endpoint identity, payload/frame bounds, pressure,
delivery, and terminal semantics remain exact. This does not implement a new
production carrier; WebSocket remains the only proven live carrier.

PR #476 exact head `7f83f916b179e098ed0a2af6bd816594a47ea406` merged as
`2ef736ca3013c4473a3fc4c523a0c42d4a71c3e0`; that exact main commit passed
workflow `31193349046`. Observatory now consumes neutral current-model reports
instead of retired Realm state. It separately projects host/boot, capability
kind/implementation/limits/status, link, plan/fragment, Play, placement,
connection, presentation, evidence, pressure, terminal/failure, and bounded
retention facts. Actual std execution can emit the artifact and the inspection
command performs no planning, activation, cancellation, or release. Tests bind
the projection to the exact current std/Pico USB plan while explicitly making
no physical-proof claim; the final three-host physical acceptance remains #350.

PR #474 merged as `a18feef9b2be092bde4dbf8c9995fc47b5cd9f3c`
from exact head `0971f0735704606b0f23d1dff9d7709e2b435977`; workflow
`31190965115` passed both required jobs. Its recorded attached-board runs prove
the unchanged Pico-local form and the exact planner-produced std-source/Pico-
sink USB CDC plan through both kernels. Sixteen ordered CYW43 LED receipts,
runtime boot/play and clean build identity, bounded pressure/delivery, and
reciprocal completion were verified. An induced physical sink failure produced
matching `Failed` facts, failed terminals, kernel cancellation, and CDC1 failure
evidence. This accepts the Pico-local and exact std↔Pico physical checkpoints;
it does not by itself establish three-host execution.

PR #482 merged as exact main
`203f9d3e37fca57a48273720bcfa8edf3d2da38f`; workflow `31195732919`
passed `check` and `browser-host` on that merge commit. Its exact tested head
`05e43888ee6117b6a61a6b1b382dd2905d9333a0` also passed workflow
`31195495733` and two recorded attached-board runs with a clean firmware image
bound to that head. One run produced sixteen matching ordered stdout, Chromium
DOM, and physical Pico LED receipts from unchanged `examples/triple-signal.form`
with bounded WebSocket and USB CDC sessions, one shared source kernel, stable
capacities, zero retained/in-flight values, and reciprocal completed terminals.
The negative run broke the browser link after four delivered values, retained
four exact stdout/DOM receipts, cancelled the shared kernel, and reached
reciprocal Pico `Failed` and failed-terminal evidence. Deterministic exact-plan
tests cover missing capability, stale boot/provider identity, malformed frames,
and browser sink failure. S4 #350 is accepted at this boundary.

No UDP, Zenoh, TCP, BODY, catalog expansion, browser Observatory UI, discovery,
TLS, public hosting, or reconnection policy is implied.

PR #483 exact head `240a12b865e36221ece7fcdbcc513d16dd432fb6`
passed workflow `31196139072` and merged as exact main
`21d3b5d4523c30540bb12bf0c05bccdbb80bc99b`; push workflow `31196307290`
passed both `check` and `browser-host`. Host advertisements now carry optional
planner profiles and finite request limits. The standard and actual browser
hosts advertise full and browser/WASM profiles, and the browser performs local
planning inside its WASM start path. Equivalent inputs produce the same plan
across distinct planner host/boot identities because planner identity is not a
plan input. Oversized bounded-profile requests fail before planning without a
delegation path. Pico and other non-planner advertisements remain complete plan
targets. This accepts #468 without claiming a general or allocator-free Pico
planner, a coordinator service, new transport, or new physical evidence.
