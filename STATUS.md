# Conduit salvage status

This file is the repository-level claim boundary. A green check proves only the
rows and commands named here; it does not promote a simulation into a platform
adapter or physical proof.

| Surface | Contract | Simulation | Executable hosted implementation | Actual browser adapter | Actual firmware | Live transport | Physical/HIL proof |
|---|---:|---:|---:|---:|---:|---:|---:|
| Port-aware salvage kernel | port protocol, fixed scheduler, Operation adapter, retained state, node-scoped correlated host operations, cancellation, and exact local/remote-plan numeric lowering | fixed/hosted full adapter form, atomic join, real latest, host-lifecycle, cancellation, exact signal/multi-value lowering, remote pressure/delivery, and zero-allocation activation vectors | same scheduler with preallocated hosted storage; installed Signal pair, local three-sink fan-out, typed multi-value std profiles, and the std half of the distributed Signal checkpoint execute through it; unsupported forms fail closed without a production legacy pump | browser/WASM sink executes an exact remote-ingress fragment through the same scheduler before bounded DOM presentation | allocator-free Pico W firmware consumes bounded kernel tables generated from unchanged `examples/signal-demo.form` through current checking, planning, and lowering; no board execution is recorded | one bounded loopback WebSocket remote cord; provider owns carrier I/O only | no |
| Exact plan, play, evidence, and presentation identities | S2 planning plus S3/S4 runtime identity acceptance: separate source/checked/expanded/plan types; boot-scoped active-play issuance; host-issued evidence identities; exact active-play/presentation correlation at platform and remote-cord boundaries | semantic/spelling, cycle, mutation/resealed-lie, host-operation admission/bounds, resource reservation/release, authority/link denial, observation-overflow, boot/activation mutation, unique evidence, wrong-presentation identity, and wrong session identity vectors | yes, std preparation enforces S2 truth and the distributed source binds exact plan/fragment/play/link/connection identities | browser sink independently reconstructs and lowers its exact fragment and rejects stale/wrong session facts | generated firmware manifest and USB verifier carry source/checked/expanded/plan/fragment/play/presentation/evidence identities and reject mutation; no physical transcript is recorded, and the current Pico boot/play identity is image-static rather than unique per board execution | live session verifies exact provider instance, endpoints, limits, host/boot, fragments, plays, connection, and value kind | no |
| Lossless form and composite boundary | S3 plus #398/#399 corrections: exact source, bounded lossless CST, located diagnostics, inline checked forms, recursively bound expansion identity, and checked named input/output faces with exact endpoint, value-kind, direction, and independent-terminal contracts | round-trip/recovery/limits, expansion and face mutation denial, standalone/nested equality, two-input/two-output typed control/data execution, input-only/output-only planning, exact pressure/retry, independent closure, cancellation/failure, parent terminal evidence, and topology hiding | parser/checker and planner are general for the checked face contract; the hosted composite compatibility façade routes exact named ports atomically while production std execution remains on `conduit-kernel` | no | no | fixture in-memory links only; face mappings are not transport | no |
| Connection envelope and session wire formats | allocating fixture envelope plus allocation-stable borrowed live-session protocol | deterministic envelope corpus and session lifecycle/mutation vectors | native binary-only RFC 6455 provider with fixed message bounds | real browser WebSocket API with one-message inbox and explicit send bounds | no | actual loopback socket; Hello/Ready/Offered/Pressure/Accepted/Delivered/InputClosed/Cancelled/Failed/Terminal frames | no |
| Portable Signal | yes | multi-value fixtures | std kernel pulse source and local stdout/timer paths | browser/WASM kernel show sink with sixteen exact DOM receipts | unchanged-form-derived fixed image with sixteen ordered identity-bearing USB CDC Signal receipts; compiled and host-verified, not board-accepted | sixteen ordered values over the bounded WebSocket remote cord | no |
| Browser manifestation | local and remote-ingress Signal profiles | yes, `conduit-browser-sim` | actual Rust/WASM planner plus exact-plan-lowered `conduit-kernel` execution for local and distributed sink fragments | thin DOM adapter with exact fixed-frame completion correlation and sixteen receipts | no | actual loopback WebSocket to the std kernel source | no |
| Pico-shaped manifestation | partial | yes, `conduit-pico-sim` | test-only | no | allocator-free RP2040 image generated from the unchanged Signal form, CYW43 GPIO 0 LED driver, pinned radio assets, USB CDC identity receipts, and host-side mutation verifier | USB CDC path implemented but no recorded board session; no UDP/TCP | no recorded board run |
| Realm membership | retired prototype | deterministic table tests | no production body model | no | no | no | no |
| Observatory | report-schema prototype | synthetic fleet | synthetic command only | no | no | no | no |
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
- exact local signal-plan lowering into numeric kernel node, directional port,
  cord, route, host-operation, resource, and evidence tables, with fail-closed
  mutation, fan-in, concurrency, remote-link, and capacity boundaries;
- real std-host execution of the exact signal pair, local three-sink fan-out,
  and typed multi-value profile through installed tables, with virtual
  timer/stdout completions, node-scoped request identity, exact
  play/presentation correlation, and measured zero-allocation activation;
- unchanged `examples/signal-demo.form` planning onto the Pico-local
  advertisement, current fragment lowering, reviewed fixed-image generation,
  allocator-free firmware consumption, generated identity-manifest checks, and
  host-side rejection of mutated receipt identities;
- bounded lossless form-source/CST round trips, located recoverable diagnostics,
  source/checked/expanded identity separation, named face checking, typed
  multi-face and zero-sided contracts, and face mutation rejection;
- boot-scoped active-play issuance, runtime-issued evidence identities, exact
  presentation completion correlation, and Observatory identity projection;
- named composite face planning/execution with two value kinds, exact
  parent-to-child input/output routing, retry pressure, independent closure,
  cancellation/failure translation, terminal evidence, and mutation denial;
- deterministic wire and simulated-host conformance vectors;
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
- WASM compilation of the browser-shaped simulation;
- Thumb compilation of allocator-free contracts, the Pico-shaped simulation,
  and the real Pico W firmware package.

WASM compilation is not browser execution by itself. Thumb compilation proves
that the firmware builds; it is not board execution or physical acceptance. The
previously accepted Chromium proof is browser-local and not a live link; the
suite now also includes one narrow live loopback std-to-browser link. That link
is not a public network, TLS, discovery, reconnection, or general transport
claim. Frame/datagram fixtures are not WebSocket or UDP sockets.

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

The S4 live std-to-browser checkpoint is accepted at exact main
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

PR #421 merged actual Pico W firmware and operator-tooling substrate at main
`227b457e065fd8f5c34371921ca2b8654a47ba31`. Exact PR head
`991685daed8e22aa9a9fff0422988ef83315438f` passed workflow `31050423893`
with both `check` and `browser-host` successful. The tree gained an
allocator-free RP2040 image using fixed `conduit-kernel` storage, pinned and
hash-verified CYW43 assets, CYW43 GPIO 0 LED control, bounded USB CDC Signal
receipts with host-side verifier negatives, typed build/flash/verify commands,
and a hardware-free firmware compilation gate.

PR #425 merged the host-side current-plan-to-fixed-image generator at main
`5290900ec3e0914f1856129bf8ca3062ad4be570`. Exact PR head
`88131ddf6f1e150a539bd044abbdbf36787d1461` passed workflow `31054417524`
with both jobs successful. It validates one current exact `PlanFragment` and its
accepted lowering against reviewed finite bounds and renders deterministic
primitive tables without restoring a second runtime on the target.

PR #426 merged the generated image into the actual Pico firmware at current
main `fb5be830f3a77cb99a491813a3b6d5f3138d7b1b`. Exact PR head
`2f5fa237f5e246ae0d8b38438e64b9c3b83572ce` passed workflow
`31057256898`; both `check` and `browser-host` succeeded. The firmware build now
parses unchanged `examples/signal-demo.form`, plans it entirely onto the
Pico-local advertisement, lowers the exact fragment, generates reviewed bounded
kernel tables and an identity sidecar, compiles those tables into the
allocator-free target, and verifies identity-bearing boot, presentation, and
terminal record shapes with mutation negatives. No separate PR-triggered
workflow is visible for the merge commit, so this is exact PR-head evidence.

This is generated firmware and host-side verifier proof, not #415 physical
acceptance. No physical `just pico` board run, exact board transcript, or
observed CYW43 GPIO 0 execution is recorded. The current Pico boot/play and
derived presentation/evidence identities are fixed at image generation rather
than unique to one board execution; USB records also do not yet bind themselves
to the manifest's exact firmware SHA-256. No bounded std-to-Pico transport,
final three-host proof, UDP, BODY, catalog expansion, Observatory acceptance,
discovery, TLS, public hosting, reconnection policy, or physical proof is
implied. #415 remains the next #350 acceptance checkpoint.
