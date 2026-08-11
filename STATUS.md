# Conduit salvage status

This file is the repository-level claim boundary. A green check proves only the
rows and commands named here; it does not promote a simulation into a platform
adapter or physical proof.

| Surface | Contract | Simulation | Executable hosted implementation | Actual browser adapter | Actual firmware | Live transport | Physical/HIL proof |
|---|---:|---:|---:|---:|---:|---:|---:|
| Port-aware salvage kernel | port protocol, fixed scheduler, Operation adapter, retained state, node-scoped correlated host operations, exact per-request cancellation, one host-neutral monotonic-millisecond deadline contract, and exact local/remote-plan numeric lowering | fixed/hosted full adapter form, atomic join, real latest, host lifecycle, cancellation/replace ordering, finite virtual-clock deadline reactor, exact signal/multi-value lowering, remote pressure/delivery, and zero-allocation vectors | same scheduler with preallocated hosted storage; a fixed-slot reactor supplies the admitted hosted monotonic clock effect; installed Signal pair, local and remote three-target fan-out, typed multi-value std profiles, distributed Signal, and admitted Play start/toggle sources execute through it; unsupported forms fail closed without a production legacy pump | browser/WASM Signal and toggle sinks execute exact remote-ingress fragments through the same scheduler before bounded DOM presentation; no browser deadline adapter is claimed | Pico W firmware consumes generated fixed kernel tables for exact local, std→Pico, and final three-host fragments; its active kernel/transport storage remains finite; no firmware deadline adapter is claimed | bounded loopback WebSocket and physical USB CDC remote Cords; Bases own Line I/O only | exact Pico-local, std↔Pico, and final std/browser/Pico board runs recorded; no physical timing claim |
| Exact local timing profile | platform-neutral deadline requirement remains separate from boot-scoped clock/timer/execution offers and the exact Plan timing/resource basis | deterministic-emulator proof admits one 1,000 µs local Plan with a sealed 730 µs worst-case basis, refuses a 100 µs request before Play, and keeps met/missed/Base-loss/cancelled/stale outcomes distinct | the accepted strict path runs the existing fixed `conduit-kernel` scheduler with zero successful heap allocations and exact Plan/Play-correlated timing Signs; inspection is excluded | no browser timing claim | no firmware timing claim | local only; no remote guarantee | no physical timing claim; any future claim requires separate pinned-hardware proof |
| ConduitOS cooperative execution regions | ordinary Plan truth seals two exact disjoint admitted placement sets, cooperative bounded-step profiles, distinct selected execution-lane resources under one finite Base, region-local memory/timer/Cord/Sign bounds, and explicit false preemption, isolation, and physical parallelism | membership/lane/capacity/Base/budget mutation, unavailable or duplicate lane, and resealed one-lane lies refuse before Play; a causal witness retains timer interest while the text region progresses | one unchanged two-branch Form runs through one production kernel/scheduler; Observatory/Patchbay linearly projects both immutable regions and the logical overlap witness | no browser execution-lane claim | one x86_64 freestanding-emulator Host offers and validates two boot-scoped cooperative lanes | local only; no connectivity fact | no SMP, physical parallelism, preemption, isolation, context switching, physical scheduling, or HIL claim |
| ConduitOS bounded xHCI Base | one boot-scoped Base identity binds one exact PCI function, MMIO BAR, fixed controller/ring storage, finite admitted limits, and machine-readable progress/failure Signs without becoming a semantic input capability | deterministic absence, class, BAR, capability, bounds, timeout, and completion refusal vectors remain distinct; a controller-absent QEMU boot refuses without a usable Base | no hosted implementation; `cargo xtask conduitos xhci-proof` owns the repository proof entrance and retained JSON report | no browser claim | one pinned x86_64 QEMU `qemu-xhci` controller completes bounded halt/reset/start and one real No-Op command through fixed command/event rings | no USB-device enumeration or external transport claim | no physical controller, device, input, or HIL claim |
| ConduitOS bounded USB enumeration | one attachment-scoped device identity binds the exact boot, xHCI Base, root port, slot, USB address, structural interfaces, and endpoints; all enumeration storage and progress are finite | deterministic descriptor, topology, capacity, reset, vanish, transfer, completion-identity, and stale-attachment vectors remain distinct; a real device-absent QEMU boot refuses without a device | no hosted implementation; `cargo xtask conduitos usb-proof` owns the repository proof entrance and retained JSON report | no browser claim | one pinned root-attached QEMU USB device completes bounded reset, enable/address, five EP0 control transfers, descriptor parsing, and `SET_CONFIGURATION` through the accepted xHCI Base | no hub, hotplug, or external transport claim | the enumeration slice itself claims no HID report parsing, semantic input, key event, physical device, or HIL proof |
| ConduitOS bounded HID boot keyboard | one attachment-scoped HID-local identity binds the exact boot, xHCI Base, USB device, boot-keyboard interface, interrupt-IN endpoint, transfer completions, and ordered usage transitions; all report, queue, transfer, polling, and Sign work is finite | deterministic interface, endpoint, packet, protocol, report, rollover, duplicate-usage, completion-identity, removal, and pressure refusals remain distinct without fabricated transitions | no hosted implementation; `cargo xtask conduitos hid-proof` owns the repository proof entrance and retained JSON report | no browser claim | one pinned QEMU USB boot keyboard receives an acknowledged QMP key action, completes `SET_PROTOCOL` and two real interrupt-IN transfers, and derives exact usage `0x04` press then release through the accepted xHCI/USB path | no external transport claim | freestanding-emulator only; no semantic `input/keyboard` offer, layout, Unicode, general HID parser, physical device, or HIL claim |
| Portable keyboard semantics | `input/key-event@1` is one exact 3-byte value using the HID Keyboard/Keypad usage page as host-neutral vocabulary, Pressed/Released, and eight distinct modifier bits after the transition; `input/keyboard` is a normal typed closing-flow source with an exact revision, finite queue, input resource, and bounded next-event host-operation requirement | reusable A, Shift+A, left/right modifier, and simultaneous-key vectors cross the fixed kernel unchanged; capacity-one pressure, closure, cancellation, host-input failure, malformed encoding, reserved usage, and inconsistent modifier state remain distinct | contract/catalog only; Patchbay discovers the semantic Kind, but the std Host truthfully advertises no implementation offer | no browser implementation claim | no firmware or ConduitOS semantic binding claim | no transport | no physical keyboard or HIL claim |
| ConduitOS portable keyboard realization | one ready boot-local xHCI/device/interface/endpoint chain yields one exact `conduitos/usb-hid-keyboard@1` offer, ordinary keyboard Plan, production-kernel Play, and portable press/release values with finite device, report, transition, operation, memory, and Cord reservations | absent/unhealthy/ambiguous device truth, stale boot or artifact identity, capacity exhaustion, malformed values, Cord pressure, cancellation, transfer failure, device loss, and closure remain distinct; a real no-device boot emits no keyboard offer | no hosted implementation; `cargo xtask conduitos keyboard-proof` owns the repository proof entrance and retained JSON report | no browser implementation claim | one pinned QEMU USB keyboard produces `[4,0,0]` then `[4,1,0]` through the planned source and native Observatory/Patchbay projection | no external transport claim | freestanding-emulator only; no keymap/text, hotplug, physical keyboard, or HIL claim |
| ConduitOS low-level local rescue | one interactive x86_64 profile taps opaque validated physical HID transitions before semantic routing, admits exact Ctrl+Alt+Delete under local-only policy, records one boot-scoped request, and crosses one bounded machine-reset Base without making an ordinary Form or second runtime authoritative | finite matcher, malformed-report, held-key, disabled-policy, unavailable-reset, stale/same-Boot, and physical Ctrl+Delete, Alt+Delete, and Ctrl+Alt+Backspace refusal vectors remain distinct | no hosted implementation; `cargo xtask conduitos rescue-proof` owns same-QEMU request/completion correlation and retained JSON/transcript evidence | no browser rescue claim | one pinned QEMU process observes B1, one guest-issued reset request, and fresh B2 with `B2 != B1`; request acceptance and reboot completion remain distinct | local physical/emulator input only; remote semantic key values cannot construct local authority | freestanding-emulator only; active-K6-Play independence remains unproved until #812, and there is no frozen-machine/NMI, physical keyboard, or HIL claim |
| Exact plan, play, Sign, and presentation identities | S2 planning plus S3/S4 runtime identity acceptance: separate source/checked/expanded/plan types; boot-scoped active-play issuance; host-issued Sign identities; exact active-play/presentation correlation at platform and remote-cord boundaries | semantic/spelling, cycle, mutation/resealed-lie, host-operation admission/bounds, resource reservation/release, authority/link denial, observation-overflow, boot/Play start mutation, unique Sign, wrong-presentation identity, wrong session identity, generated-image mutation, firmware-build mismatch, and runtime-identity mutation vectors | yes, std preparation enforces S2 truth and distributed sources bind exact plan/fragment/play/link/connection identities | browser sinks independently reconstruct and lower exact fragments and reject stale/wrong session facts | generated image and manifest bind source/checked/expanded/plan/fragment identities and clean firmware build identity; runtime-generated boot/play plus presentation/sign identities are carried in USB records and checked across physical sessions | live sessions verify exact base instance, endpoints, limits, host/boot, fragments, plays, connection, and value kind | matching physical receipts retain exact plan/fragment/play/presentation/sign/link/base/boot/build identity |
| Lossless form and composite boundary | S3 plus #398/#399 corrections: exact source, bounded lossless CST, located diagnostics, inline checked forms, recursively bound expansion identity, and checked named input/output faces with exact endpoint, value-kind, direction, and independent-terminal contracts | round-trip/recovery/limits, expansion and face mutation denial, standalone/nested equality, two-input/two-output typed control/data execution, input-only/output-only planning, exact pressure/retry, independent closure, cancellation/failure, parent terminal Sign, and topology hiding | parser/checker and planner are general for the checked face contract; the hosted composite compatibility façade routes exact named ports atomically while production std execution remains on `conduit-kernel` | no | no | fixture in-memory links only; face mappings are not transport | no |
| Canonical Form execution corpus | canonical face/back source, declarative startup binding, recursive expansion, exact checked-face compatibility including temporal shape, and distinct source/checked/expanded/Plan identities | Programs 1–4 deterministic positive/negative corpus; Program 6 exact two-host planning and link-failure vectors | real std `text/*`, `time/every`, `state/count`, and presentation leaves execute through the existing kernel with bounded Sign and stable capacity | Program 6 browser/WASM sink executes the unchanged canonical Signal source's exact remote fragment | no new firmware claim | actual loopback WebSocket carries the Program 6 Conduit session; it is not an authored external WebSocket operation | no new physical/HIL claim |
| Portable planner capability | optional boot-scoped planner profile/limit offers are part of the `no_std` host advertisement contract; planner identity and scratch state are excluded from plan construction and identity | full and browser-profile equivalence across different planner host/boot identities, bounded pre-planning refusal, missing/ambiguous-offer denial, and a non-planner Pico target with a verified lowerable fragment | the standard reference host advertises `conduit.planner/full@1` and invokes the shared deterministic planner; non-planner hosts remain ordinary complete targets | the actual browser/WASM host advertises `conduit.planner/browser-wasm@1` and plans locally in its WASM start path before lowering and kernel execution | Pico advertises no planner profile; existing generated firmware consumes its externally planned exact fragment, with no general Pico planner claim | no new transport or delegation service | no new physical/HIL claim |
| Competing capability realizations (R2) | equal checked faces admit distinct host implementations/artifacts; general hard requirements, explicit policy, stable realization characteristics, changing resource observations, finite compute ranges/service/topology, exact authority, and immutable selected Plan facts remain separate | three explicitly synthetic text-generation fixtures prove hard context/privacy rejection, deterministic policy choice, bounded candidate-decision Sign, minimum-first compute allocation, observation-driven replacement planning without old-Plan mutation, and non-AI video/storage representability | one selected deterministic fixture executes through ordinary planning, lowering, the fixed kernel, admitted host operation, exact base, and presentation boundaries; normal std preparation validates scalable compute reservations | no new browser adapter or browser-owned realization truth; the required browser job only guards the existing adapter boundary | no firmware execution or bare-metal lane-base claim; `BareMetal` is an architecture-neutral contract demonstrated below physical proof | remote-base and network-storage cases are contract/fixture representations only; no live model API, endpoint, credential, paid service, or new transport claim | no physical CPU enumeration, scheduling-quality, firmware-lane, model, transport, or HIL proof |
| Connection envelope and session wire formats | allocating fixture envelope plus allocation-stable borrowed exact session protocol; framed sessions preserve Base-specific exact identity and bounds | deterministic envelope corpus and session lifecycle/mutation/Base-eligibility vectors | native binary-only RFC 6455 and USB CDC Bases with fixed frame/buffer bounds | real browser WebSocket API with one-message inbox and explicit send bounds | dual-CDC Pico Line keeps session frames on CDC0 and Sign on CDC1 | actual loopback WebSocket plus physical USB CDC; fixture bases remain synthetic conformance only | reciprocal physical Hello/Ready/value/pressure/failure/terminal lifecycles recorded |
| Cord meaning and finite Line realization | `CordId`, `LineId`, Base, binding, endpoint, and session identities are distinct; Plans seal exact bounded Line contracts while current availability remains an external Sign | deterministic one-Line/two-Line planning, immutable availability, unsealed-selection denial, session continuation, and replacement-Plan vectors | std planning/runtime consume only exact ready admitted Line offers and expose selected Line/Base/binding diagnostics | browser/WASM sessions and Patchbay HTML consume the same exact planned Line identities without making transport part of Form or Cord meaning | generated fixed images carry exact Line identity through every selectable Pico composition | bounded WebSocket and USB CDC are distinct Lines to the same R1 Pico boot; no new live transport family is claimed | consumes the already-accepted #361 physical replan and admitted-continuation evidence without claiming a new board run |
| Portable Signal | yes | multi-value fixtures | one std kernel pulse source atomically fans each value to stdout plus two exact remote egresses | browser/WASM kernel show sink with sixteen exact DOM receipts | unchanged Signal forms generate exact local and remote Pico images that drive the CYW43 LED | sixteen ordered values over bounded WebSocket and USB CDC remote cords | sixteen matching ordered stdout, DOM, and physical LED receipts from one exact three-host run |
| R1 Body and Line recovery | exact immutable Plan candidates, distinct Line readiness Signs, replacement-planning events, allocation-stable session checkpoints, and Body/Wake/Lull lifecycle identities | deterministic new-Plan and same-Plan recovery vectors cannot claim physical acceptance | one terminal peer and two browser peers drive exact planned inputs; ordinary planning replaces an unsatisfied WebSocket-only Plan with USB CDC | two pinned Chromium peers each issue exact keydown/on and keyup/off inputs in every physical branch | one continuously USB-powered Pico W boot exposes WebSocket and USB CDC Lines, retains pre-admitted Plan C continuation state, and seals post-Play-start allocation | physical WebSocket loss produces either a new USB Plan/Play or bounded same-Plan/same-Play USB continuation according to the immutable Plan | exact-main `cargo xtask prove r1-hil --interactive` completed both physical faults, eighteen correlated LED Signs, same Body and Pico boot, required Wake continuity, Lull, and later Wake |
| Optional pre-Play HOLD | one Wake may admit an exact immutable Plan plus finite planning-basis Signs, hold reason/source, persistence policy, fixed release-authority contract, and current-validity result while remaining distinct from Playing, active-Play pause, and Lull | deterministic direct, held, authorized release, stale-basis replacement, persistent re-hold, non-persistent replacement, authority denial, bounded-basis, replay-tamper, and lifecycle-separation vectors | `conduit-body` exposes bounded held-Plan admission, inspection, release, invalidation, and replacement APIs; no `ActivePlayId` exists before successful release, and release revalidates the complete current basis before starting Play | no browser UI or adapter claim | no firmware change | no new transport; visibility, reachability, and connectivity confer no release authority | no physical/HIL claim |
| Browser manifestation | local and remote-ingress Signal profiles | yes, `conduit-browser-sim` | actual Rust/WASM planner plus exact-plan-lowered `conduit-kernel` execution for local and distributed sink fragments | thin DOM adapter with exact fixed-frame completion correlation and sixteen receipts | no | actual loopback WebSocket to the std kernel source | included in the accepted three-host physical run with matching cross-host receipts |
| Interactive Play start/toggle | typed `interaction/start -> state/toggle -> presentation/show` contract with admitted input and exact remote planning | deterministic Play start/toggle lifecycle and identity negatives | native std source services one admitted stdin Play start through the kernel before realizing the corresponding remote offer | pinned Chromium proves the first Enter causes exactly one sequence-0 DOM update before later inputs, then completes sixteen exact presentations with one real pressure retry | no | actual bounded loopback WebSocket with structured link-break failure | no |
| Native Patchbay presentation and interaction | checked/expanded Forms project exact Gear/Port/Cord subjects; finite platform-neutral `interaction/select` and `interaction/invoke` requests cross exact typed Ports and one admitted host-operation boundary through the production kernel | deterministic geometry/hit, pointer/keyboard convergence, Plan/Play/Sign inspection, bounded-value, stale/unknown/oversized identity, request-restoration, and distinct success/refusal/failure vectors | the actual native Patchbay window renders the bounded canvas and routes pointer selection, graphical keyboard traversal, open/save/view actions, and Body lifecycle controls through ordinary interaction Plays before shared inspector or control state changes | HTML consumes the same semantic request types without DOM identity; no interactive HTML realization is claimed | no firmware or framebuffer interaction claim | no new transport | no physical/HIL claim |
| Explicit external WebSocket chat | opt-in `net/websocket` client/listener faces encode complete-message RFC 6455 semantics structurally; equal checked faces remain compatible across nominal names/revisions, while a generic byte-stream face does not | deterministic checked-face, canonical expansion, bounded planning/kernel execution, malformed/oversize, and disconnect vectors | bounded two-peer std listener executes exact accept/receive/send operations through the ordinary fixed scheduler and host-operation boundary | two independent planned browser/WASM kernels use native browser WebSocket plus bounded text-input/list adapters; pinned Chromium proves A/B exchange and truthful one-peer continuation | no | actual binary loopback external WebSocket messages, mechanically distinct from `ConnectionBase::WebSocket` Conduit-session carriage | no physical/HIL claim |
| Bounded shared pools and explicit dynamic flow | checked Forms carry exact scoped pool references and hard member bounds; Plans seal equal-face member contracts, host/boot/capability/resource envelopes, per-member queue/Sign limits, admission authority, and explicit consumers | allocator-free keyed membership, stale occupation epochs, deterministic membership snapshots, per-branch fan pressure/outcomes, and source-tagged bounded merge vectors | the existing kernel owns fixed pool/fan/merge state; the std proof host plans and lowers one 32-peer chat pool without adding a scheduler or ambient registry | two Chromium pages dynamically join, exchange addressed broadcasts, one leaves, and the remaining peer continues; the authored Form names only pool, room, fan, merge, and peer semantics | no | the proof host selects a bounded binary loopback WebSocket Line below authored semantics; no socket/address/Base fact enters source identity | no physical/HIL claim |
| Pico-shaped manifestation | exact Pico-local and remote-ingress advertisements with reviewed fixed-image bounds | yes, `conduit-pico-sim` | host-side unchanged-form planning/lowering/image generation, exact std source, and verifier tests | no | RP2040 images generated from exact local/remote fragments, CYW43 GPIO 0 LED driver, pinned radio assets, clean firmware-build identity, runtime boot/play receipt identity, and bounded dual CDC | exact bounded std↔Pico USB CDC and final three-host sessions | recorded local, exact std↔Pico success/failure, and final three-host success/broken-link runs |
| Retired membership prototype | historical only | deterministic table tests | no production Body model | no | no | no | no |
| Observatory | versioned neutral host/capability/Base/link/plan/Play/pressure/current-and-historical-Sign/retention reports with exact identity and bound validation; sealed boot provenance remains distinct from live offers and Bases | synthetic fleet retained only as an explicitly labeled integration test | actual std execution can write a bounded report artifact; the read-only `observatory-report` command validates and renders complete structured tables without runtime control; native Patchbay validates and linearly renders the same ordinary snapshot exported by ConduitOS | no browser UI or browser-owned runtime truth | no firmware-side inspector or report store; the accepted ConduitOS export is freestanding-emulator proof | no new transport; observed links are report facts only | no new physical/HIL claim |
| Durable system continuity | allocator-free realization record over explicit membership, complete checked-face role requirements, exact host+boot assignments, observed links, boot-scoped authority, Plan, Play, and Sign identities | accepted std/browser/Pico replacement vector consumes a validated current-model snapshot, separates request acceptance/old-boot terminal/new-boot observation, and requires explicit replanning with new Plan/Plays and no stale grant inheritance | no execution engine; the layer consumes current reports and exact plans without owning scheduling, placement, bases, or authority issuance | no new browser adapter or UI claim | no firmware change; the accepted Pico arrangement is consumed as already-proven input | no new transport; link observation remains distinct from membership and authority | no new physical/HIL run or claim |
| PREWAKE robotics semantics | seven exact portable Kinds cover bump, body-frame orientation, sensor-forward range, start-local odometry, battery, body velocity intent, and differential-drive projection; each Port retains a distinct bounded Info identity where shape, unit, frame, or validity differs | exact-main `607f602da25d23f9b74535e2272c6bd151f3604d`, workflow `31460348259`: deterministic codecs and an ordinary checked Form/Plan/lowered production-kernel Play prove clear-space projection plus independent pressed-bumper and insufficient-range suppression; invalid, missing, stale, pressure, cancellation, and unavailable-implementation outcomes remain distinct | the optional std robotics family advertises preallocated PREWAKE-only sources and a simulated differential-drive projection with no host operations, resources, authority requirements, live device Signs, or physical-effect completion; Netherwick bump/IMU describe-only offers reuse the same exact portable faces | no browser execution or manifestation claim | contracts compile for Thumb; no firmware implementation or execution claim | no transport | no physical actuator, device, HIL, or safety-certification claim |
| `conduit.std` | twenty-eight exact typed contracts: time tick/every, Boolean debounce and tick inactivity timeout, typed tick/text/count presentation, text literal/upper/join, state/count, scalar latest/tee/gate, scalar compare/select, Boolean not, scalar clamp/scale/deadband, seven PREWAKE robotics observation/intent/drive contracts, and protected file/copy; legacy `value/any` rows remain unsupported fixtures | UI-independent contract/codec/limit/mutation vectors, canonical Programs 1–4, and deterministic flow/state/decision/math/temporal/robotics pressure, closure, cancellation, boundary, overflow, and mutation vectors | std reference host advertises selected families and resolves exact installed implementations before bounded execution through `conduit-kernel`; scalar flow/state, decision, math, timing, and PREWAKE bumper/range/velocity/differential-drive Forms execute through ordinary planning/lowering, while minimal/subset compositions advertise only selected offers | no manifestation claim for these twenty-eight revisions | no new firmware manifestation for the eighteen flow/state/decision/math/temporal/robotics revisions; the separate ConduitOS row owns its narrower five-contract proof | no new transport; Program 6 uses the separately owned Signal family | no new physical/HIL claim |
| ConduitOS portable std gap | bounded inventory derives all exact supported-nucleus contracts/offers, revisions, faces, limits, and canonical SHA-256 content identity directly from catalog truth; legacy compatibility rows cannot enter | deterministic mutation and Host-build separation vectors plus exact 28-item classification | `cargo xtask conduitos std-gap` compares canonical kind/revision identity against the exact boot-scoped ConduitOS profile and reports 5 implemented, 23 missing without advertising any missing capability | no new browser claim | one x86_64 freestanding-emulator ConduitOS Host runs the unchanged bounded `text/literal -> text/upper -> presentation/text` Form through ordinary planning and the production kernel, then presents exactly `HELLO, CONDUITOS` through its admitted serial Base; all eighteen flow/state/decision/math/temporal/robotics revisions remain classified missing | no transport | no physical/HIL claim |
| Copy a file | unsafe prototype disabled | tests removed from default tree | no admitted host operation | no chooser | no | no | no |

## Required CI claims

The `check` workflow requires:

- workspace formatting, Clippy, and tests;
- one exact deterministic local timing profile with finite clock, timer,
  execution, arena, Cord, wake, Base scratch, mandatory Sign, and fault-reserve
  bounds; pre-Play unschedulable refusal; zero-allocation strict execution; and
  distinct met, missed, Base-loss, cancellation, and stale-basis Signs;
- two exact disjoint ConduitOS cooperative execution regions with admitted Gear
  sets, bounded-step profiles, distinct selected boot-scoped lane resources under
  one finite Base, sealed region-local memory/timer/Cord/Sign bounds, explicit
  no-preemption/no-isolation/no-physical-parallelism, pre-Play
  stale/wrong/unavailable/duplicate-lane refusal, one causal overlap witness, and
  immutable linear inspection;
- one pinned x86_64 QEMU xHCI controller discovered through PCI and realized as
  one boot-scoped finite Base with fixed MMIO/page-table/DMA/ring storage,
  bounded halt/reset/start and No-Op completion progress, distinct absence and
  malformed-controller refusals, no semantic keyboard offer, and a separately
  retained machine-readable proof report;
- one pinned root-attached QEMU USB device enumerated through that xHCI Base
  with fixed device/input contexts, transfer ring, descriptor buffer, finite
  polling and five EP0 control transfers, exact attachment/interface/endpoint
  identities, distinct refusal vectors, no semantic keyboard offer, a real
  device-absent boot, and a separately retained machine-readable proof report;
- one pinned QEMU USB boot keyboard matched by exact class/subclass/protocol and
  interrupt-IN endpoint truth, selected into Boot Protocol, driven through an
  acknowledged emulator input action and two real xHCI interrupt transfers,
  with exact ordered HID-local press/release transitions, finite admitted
  storage/work, distinct malformed/removal/pressure refusals, no semantic
  keyboard offer, and a separately retained machine-readable proof report;
- one exact portable `input/key-event@1` codec and `input/keyboard` semantic
  source contract with host-neutral usage identity, after-transition modifier
  state, three-byte values, eight-item/twenty-four-byte queue bounds, reusable
  cross-implementation vectors, ordinary-kernel pressure/cancellation/terminal
  proof, authoritative Patchbay metadata, and no current Host implementation
  offer;
- one exact boot-scoped ConduitOS `input/keyboard` realization whose ready
  xHCI/USB/HID chain, finite resources, ordinary Plan, production-kernel Play,
  portable press/release values, absent-device refusal, and read-only
  Observatory/Patchbay projection are checked by `cargo xtask conduitos
  keyboard-proof` and retained as a machine-readable report;
- one bounded low-level ConduitOS local rescue path that consumes only opaque
  validated physical HID transitions before ordinary keyboard planning,
  recognizes exact Ctrl+Alt+Delete once, records B1 and exact local authority,
  issues one guest reset, and is correlated by `cargo xtask conduitos
  rescue-proof` to a distinct B2 in the same QEMU process; retained proof and
  transcript evidence also establish physical near-miss refusal and preserve
  request acceptance separately from reboot completion;
- one bounded deterministic ConduitOS portable-std inventory/gap report derived
  from current supported-nucleus contract and offer truth, with a semantic
  content digest, exact Host build/profile basis, and complete implemented or
  missing classification without capability promotion;
- no-std checks for the salvage kernel, semantic, wire, and std-catalog contracts;
- hosted/fixed salvage-kernel protocol, storage, scheduler, pressure, atomic
  join, retained-state/latest, host lifecycle, closure, exact dispatched-request
  cancellation/replacement, completion-before-cancel, and cancellation vectors;
- one no-std monotonic-millisecond deadline operation/resource contract plus a
  finite std Host reactor with deterministic equal-deadline order, virtual and
  hosted monotonic clocks, distinct stale/full/clock failures, and one
  production-kernel arm/cancel/replace/complete flow;
- exact typed Boolean trailing-debounce and tick inactivity-timeout contracts
  with finite duration/value bounds, ordinary Form planning and production-
  kernel execution, deterministic virtual schedules and Signs, zero/maximum,
  simultaneous, late, burst/reset, closure, cancellation, and stale/missing-
  Base vectors, and zero successful post-Play allocations;
- exact portable robotics observation, body-velocity-intent, and differential-
  drive contracts with unit/frame-specific bounded Info, deterministic PREWAKE
  offers, ordinary check/plan/lower/kernel execution, stable allocation, visible
  bumper/range suppression without physical authority/effect, distinct invalid,
  missing, stale, pressure, cancellation, and unavailable-implementation
  outcomes, exact Netherwick describe-face reuse, palette metadata, and no-std/
  Thumb contract compilation;
- exact semantic-contract/profile/port, host-operation/resource/authority/link, and
  policy/budget planning with cycle, mutation-negative, action/completion and
  authority/link admission, reservation/release, and executable
  mandatory-sign storage tests;
- optional portable planner-profile advertisements with exact finite input
  limits, equivalent full/browser planning across distinct planner identities,
  bounded refusal without delegation, and browser-local WASM planning;
- competing equal-face realization selection with exact implementation/artifact
  offers, hard-gate-before-policy ordering, stable characteristics distinct from
  current observations, finite whole-form resource admission, bounded decision
  Sign, selected Plan sealing, ordinary deterministic-base kernel
  execution, and replacement planning that preserves the old Plan;
- architecture-neutral compute pools with minimum/preferred/maximum lanes,
  shared/reserved/exclusive service, optional topology/performance constraints,
  minimum-first allocation, hosted/bare-metal base identity, scalable
  preparation validation, and no physical/base lane identity in the Plan;
- exact local signal-plan lowering into numeric kernel node, directional port,
  cord, route, host-operation, resource, and Sign tables, with fail-closed
  mutation, fan-in, concurrency, remote-link, and capacity boundaries;
- real std-host execution of the exact signal pair, local three-sink fan-out,
  and typed multi-value profile through installed tables, with virtual
  timer/stdout completions, node-scoped request identity, exact
  play/presentation correlation, and measured zero-allocation Play start;
- bounded lossless form-source/CST round trips, located recoverable diagnostics,
  source/checked/expanded identity separation, named face checking, typed
  multi-face and zero-sided contracts, and face mutation rejection;
- canonical Programs 1–4 through checking, recursive expansion where required,
  exact face-compatible host offers, planning, the existing kernel, admitted
  host effects, terminal results, bounded Sign, and fail-closed negative
  vectors; Program 6 additionally crosses exact std/browser fragments over the
  observed capacity-one WebSocket Conduit session without Host/Line facts in
  source;
- boot-scoped active-play issuance, runtime-issued Sign identities, exact
  presentation completion correlation, and Observatory identity projection;
- optional bounded pre-Play HOLD admission and inspection with exact Plan,
  planning-basis Signs, reason/source, persistence policy, fixed release
  authority, no pre-release `ActivePlayId`, current-basis revalidation,
  stale-Plan invalidation, replacement planning, and persistent re-hold;
- named composite face planning/execution with two value kinds, exact
  parent-to-child input/output routing, retry pressure, independent closure,
  cancellation/failure translation, terminal Sign, and mutation denial;
- deterministic wire and simulated-host conformance vectors;
- validated durable-continuity vectors over the exact std/browser/Pico Plan,
  reports, Plays, and terminal Sign, with independent membership,
  availability, authority, stale-boot, face-compatibility, replan, grant, and
  Play-identity denial;
- Line-neutral framed-session eligibility vectors proving `FixtureFrame` and
  `WebSocket` preserve exact base/base-instance/link/endpoint identity,
  while `Local`, `InMemory`, and `FixtureDatagram` remain invalid for the exact
  remote session contract;
- exact finite Line offers with distinct Line/Base/binding/endpoint identity,
  explicit shape/duplex/ordering/reliability/framing/capacity/continuation/
  security facts, immutable Plan admission, external availability Signs,
  fail-closed unsealed selection, and Line-aware session wire and generated
  embedded identity;
- one actual Chromium browser-local kernel proof with two independent WASM
  instances, exact source/checked/expanded/plan/fragment/play/request/
  presentation/sign identities, stable sealed capacity, sixteen ordered
  receipts per host, and bounded failure negatives;
- one actual Chromium distributed kernel proof from the unchanged Signal form,
  with exact std-source/browser-sink fragments, kernel execution on both ends,
  one binary loopback WebSocket, sixteen ordered DOM receipts, complete session
  identity, one real receiver-`Full` retry of the same sequence, terminal
  Sign, stable sealed capacities, zero retained/in-flight values, and
  bounded lifecycle/identity/frame failure negatives;
- exact final-three-host planning and source-kernel vectors proving one atomic
  stdout/WebSocket/USB-CDC fan-out with fixed item/byte/frame bounds, exact
  session identities, stable capacity, reciprocal terminal state, and
  fail-closed missing capability, stale boot/base, malformed-frame, and
  browser-sink cases; the attached-board Playwright cases remain explicitly
  hardware-gated and do not run in ordinary CI;
- one actual Chromium distributed Play start/toggle proof in which the first
  admitted Enter produces exactly one sequence-0 DOM update before any later
  Play start is sent, followed by the complete sixteen-value terminal path,
  one real pressure retry, exact receipt correlation, and structured link-break
  failure;
- bounded native Patchbay geometry and hit-target vectors plus one shared
  platform-neutral selection/invocation family checked, planned, lowered, and
  run through the production kernel, with exact request/subject/action/Plan/
  Play/Sign inspection, pointer/keyboard convergence, HTML contract reuse,
  lifecycle invocation, and stale/unknown/oversized/refused/failed negatives;
- one actual Chromium explicit external-WebSocket chat proof with two separate
  browser/WASM kernels using native browser WebSocket, exact
  source/checked/expanded/Plan/fragment/Play/placement/Gear/Kind/implementation/
  host-operation identities, finite peer/message/queue/history/value/sign
  bounds, visible A/B exchange, content-free identity Sign, distinct
  disconnect, and truthful continuation by the remaining peer;
- direct unchanged `examples/signal-demo.form` to Pico-local plan, selected
  fragment, lowered fragment, and generated fixed-image conformance with exact
  reviewed bounds and fail-closed identity/lowering/remote-connection negatives;
- Pico verifier tests rejecting static identity mutation, firmware-build
  mismatch, missing/reused/changed runtime boot/play identity, reordered
  receipts, and invalid terminal Sign;
- WASM compilation of the browser-shaped simulation;
- Thumb compilation of allocator-free contracts, the Pico-shaped simulation,
  and every selectable real Pico W firmware composition. The minimal local
  composition executes the same exact kernel-backed Signal faces while omitting
  the optional Conduit wire/session and BOOTSEL lifecycle-control family.

WASM compilation is not browser execution by itself. Thumb compilation proves
that the firmware builds; it is not board execution or physical acceptance. A
generated fixed image and a valid USB verifier are also not a board transcript.
The previously accepted Chromium proof is browser-local and not a live link; the
suite also includes narrow live loopback std-to-browser Signal and toggle links.
Those links are not public networks, TLS, discovery, reconnection, or general
transport claims. A Line-neutral session contract is likewise not a new
Line implementation. Frame/datagram fixtures are not WebSocket or UDP
sockets.

## Salvage stop line

The #463 first host-architecture slice is accepted at exact main
`31995940332eaf9cd8d6b77d7a453d9ab62e2e6a`; workflow `31258983687` passed
both required jobs. The portable mandatory core is limited to boot-scoped
identity, bounded advertisements and planning facts, the admitted kernel
effect/completion protocol, and correlated Sign; platform facilities remain
optional bases. Std, browser, and Pico are peer compositions with different
exact offers. The kernel-backed `pico-local-minimal` Thumb composition retains
only its deliberately selected Signal operation subset and Sign path while
excluding the Conduit wire/session and lifecycle-control family. Planner
compatibility is canonical checked-face equality followed by explicit
resource/authority/policy admission, never platform or nominal operation
identity. This is compile/contract Sign, not a new physical Pico claim.
General BYOKernel scaffolding and further base extraction remain possible
follow-up work rather than accepted behavior.

The canonical Form execution corpus required by #515 is accepted for Programs
1–4 and 6. Program 1 merged as `8dcda744` with exact-main workflow
`31246581906`; Program 2 as `a66ef8aa` with `31247130811`; Program 3 as
`73f1955f` with `31247769460`; the temporal checked-face/Plan prerequisite as
`09333d60` with `31248540207`; Program 4 as `c91d5962` with `31249096578`;
and Program 6 as `eb0f690a` with `31249570250`. Each workflow passed the full
check and pinned browser jobs. Program 5 merged through PR #561 as exact main
`b67744f67bfc3ba2324d05d9591c1dedb04c5d38`; workflow `31265931786` passed
both required jobs. Its authored `net/websocket` operation is checked by
structural face equality under #522 and remains distinct from
`ConnectionBase::WebSocket`, which carries Conduit sessions. Two actual
browser clients exchange bounded messages through the std listener, then one
disconnects while the other continues. This proves loopback browser/std
execution only: it adds no public realization, TLS/auth, federation, firmware,
or physical/HIL claim. Exact commands, output, negative proof, and the
physical-proof stop line are recorded in `docs/try-forms.md`.

S6 durable continuity is accepted at exact main
`bfa2944e2b2944489b652f18261bb7b577e830f3`; workflow `31254152549` passed
both required jobs. `conduit-system-continuity` consumes a validated
Observatory snapshot and exact Plan, binds every checked-face role requirement
to one explicitly assigned capability on one exact host boot, and retains
separate membership, observed-link, boot-scoped authority, Play, and Sign
facts. Its three-host vector stages transition acceptance, old-boot terminal
Sign, and a distinct replacement observation separately; an equal-face
replacement remains only a candidate until a different exact Plan and new
Plays assign the new boot without inheriting stale grants. This adds no
scheduler, planner, Base, authority issuer, retired membership table, firmware change,
HIL rerun, or new physical claim.

S0 restores truth. S1 now includes the port-aware protocol, bounded storage,
deterministic fixed-capacity scheduling, transactional fanout, per-port closure,
correlated bounded host operations, late-completion rejection, and matching
hosted/fixed lifecycle vectors. The capacity-one conformance graph now includes
a stateful latest operation, atomic joins, and a stable hosted allocation shape.
The published `OperationInput`/`OperationAction` contract runs the complete
four-value tick/tee/filter/latest/show graph through the same fixed-capacity
scheduler in fixed and hosted profiles. S1 is accepted and now also admits
exact cancellation of one dispatched host request without
cancelling the Play. A completion already accepted by the kernel wins; an
accepted cancellation is returned as one correlated `Cancelled` completion
before a replacement request. The portable no-std contract names one
host/boot-scoped monotonic millisecond timer slot with an exact eight-byte
duration, no output, and one in-flight request. The std Host realizes that
contract through one fixed-slot reactor shared by virtual and hosted monotonic
clocks; it owns only arm/cancel/wake effects, not semantic timing policy. This
generic prerequisite is accepted at exact main
`7c275d8ed3958481feb790cd16977f1fee0cd4c7`; workflow `31452774926` passed all
required jobs. It adds no debounce/timeout Kind, browser or firmware timing
adapter, live transport, physical timing, or HIL claim. The first S2 slice
removes `CapabilityLimits.value_kind` and binds exact semantic contract
revisions, execution profiles, and complete per-port contracts through form
identity, planning, preparation, and Observatory projection. Source-document,
checked-form, and expanded-form identities are now distinct and all participate
in fragment and plan identity. Startup dependencies, cancellation and terminal
policies, mandatory Sign, and independent Sign item/byte budgets are
also sealed and validated during preparation. The hosted reboot runtime now
allocates fixed mandatory-sign slots from that plan and preserves them
independently of its lossy observation ring. Installed local `PlanFragment`
profiles lower before Play start into numeric kernel nodes, directional ports,
cords, direct route ranges, host-operation admission, resource references,
Sign targets, exact cord queue budgets, and mandatory-sign budgets. The
mapping is reversible to plan identities and rejects unsupported remote links,
fan-in, port widths, and host-operation concurrency. The exact two-node
`flow/pulse -> presentation/show` profile, its three-sink local fan-out, and the
typed multi-value conformance form install those tables into hosted kernel
schedulers, drive virtual/thread timers and stdout presentation completions,
and project host-issued active play, presentation, and Sign identities.
Their resources and allocation shapes are sealed before Play start; allocator
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
base-endpoint scope, one initialized base instance, explicit
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
the port-aware kernel. The third checkpoint adds a configured Gear inline
nesting with a hard depth limit. The child is an ordinary checked form whose
selected authored export becomes the parent operation; standalone and nested
spellings produce the same checked/expanded child and boundary identities, and
inner errors remain located in the lossless outer document. Parent checked
identity binds only the visible exported contract; parent expanded identity
recursively binds canonical Gear paths, selected exports, and child
expanded identities. Omitted, duplicated, reordered, or substituted expansion
rows fail before planning.
The final S3 checkpoint keeps plan, active play, Sign, and presentation as
different core identity types. Play start sequences are host/boot scoped;
Sign IDs are issued by the recording host rather than synthesized from UI
row indexes; and adapters must return the exact active-play and presentation
IDs carried by each effect. S3 is accepted at this boundary.
The S4 browser-local kernel checkpoint is accepted at exact main
`b7852eed1e784a27dcd78e700b2f89ddc01bc097`; workflow `31022565054` passed
both the full Rust gate and the pinned Chromium job. Two independent WASM
instances parse and plan unchanged `examples/signal-demo.form`, lower their
exact local fragments through the shared contract, and execute through
`conduit-kernel`. JavaScript remains the real-timer/DOM adapter. Fixed frames,
exact completion correlation, item/byte limits, duplicate/malformed/wrong
identity denial, cancellation, Sign exhaustion, terminal failure, and
stable sealed capacity are executable proofs. WASM allocation is not claimed
to be measured; the accepted claim is precise capacity stability.

The S4 live std-to-browser Signal checkpoint is accepted at exact main
`a1f479dfa58b8537427b5747da73795628504913`; workflow `31031406945` passed
both the full Rust gate and the pinned Chromium job. The unchanged
`examples/signal-demo.form` lowers into exact std-source and browser-sink
fragments. Both execute through `conduit-kernel`; a binary-only loopback RFC
6455 base carries the remote-cord session without owning scheduling or
value lifecycle. Sixteen values reach the DOM in order through one-item and
nine-byte cord/buffer limits, including one observed receiver-`Full` response
and exact same-sequence retry. Both kernels terminate with Sign, capacities
remain stable, and no value remains retained or in flight. Missing/stale links,
identity mutation, malformed/truncated/oversized/trailing frames,
duplicate/reordered sequence, early disconnect, sink failure, cancellation,
late acknowledgement, and Sign exhaustion fail closed.

The corrected S4 interactive std-to-browser toggle checkpoint is accepted from
PR #432 exact head `d5d95fbeba8e373e157d4759fa1912ad4a414a82`; workflow
`31062645805` passed. The native source admits at most one stdin Play start per
source offer cycle and realizes its remote offer before reading another. Pinned
Chromium proves the first Enter produces exactly one sequence-0 toggle-on DOM
manifestation before the remaining fifteen inputs are sent. The full path then
completes sixteen exact presentations with unique request/presentation/sign
identity, one real receiver-pressure retry, terminal agreement, stable bounded
capacity, and a structured four-receipt link-break failure. PR #432 merged as
`0a99f4d75a2ef38cb63dcae474288b3eca429e94`.

The Pico-local code path is now generated from the unchanged portable form.
PR #426 exact head `2f5fa237f5e246ae0d8b38438e64b9c3b83572ce` passed workflow
`31057256898` and merged as `fb5be830f3a77cb99a491813a3b6d5f3138d7b1b`.
Its firmware build parses `examples/signal-demo.form`, plans both Gears
onto the Pico-local advertisement, lowers the selected fragment, and emits the
allocator-free fixed image consumed by the RP2040 firmware. Hand-authored
firmware topology/configuration ordinals are no longer the execution source.
The generated image and verifier carry source, checked, expanded, plan,
fragment, presentation, and Sign identities.

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
synthetic Observatory/membership/browser/Pico fixture from the production `conduct` executable
and retains it only as a dedicated integration test with dev-scoped
dependencies. No PR-triggered exact-head workflow is attached to that direct
cleanup commit, so it is merged repository state but is not a new S4 acceptance
verdict.

PR #436 exact head `f911c1d8007608b1db5fece731a998e42a085c28` passed workflow
`31137116246` and merged as current main
`82fec9f1b65ff537148244698cd16744416ce8dc`. The exact framed session contract
is now Line-neutral at the semantic boundary: `FixtureFrame` and
`WebSocket` are eligible bases, while `Local`, `InMemory`, and
`FixtureDatagram` remain invalid. Base identity, base-instance identity,
link binding, host/boot/endpoint identity, payload/frame bounds, pressure,
delivery, and terminal semantics remain exact. This does not implement a new
production Line; WebSocket remains the only proven live Line.

PR #476 exact head `7f83f916b179e098ed0a2af6bd816594a47ea406` merged as
`2ef736ca3013c4473a3fc4c523a0c42d4a71c3e0`; that exact main commit passed
workflow `31193349046`. Observatory now consumes neutral current-model reports
instead of the retired membership prototype. It separately projects host/boot, capability
kind/implementation/limits/status, link, plan/fragment, Play, placement,
connection, presentation, Sign, pressure, terminal/failure, and bounded
retention facts. Actual std execution can emit the artifact and the inspection
command performs no planning, Play start, cancellation, or release. Tests bind
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
Sign. This accepts the Pico-local and exact std↔Pico physical checkpoints;
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
reciprocal Pico `Failed` and failed-terminal Sign. Deterministic exact-plan
tests cover missing capability, stale boot/base identity, malformed frames,
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
planner, a coordinator service, new transport, or new physical Sign.

PR #563 merged as exact main
`b5134ecb174860c8a8bc19aff2829310fdfb2868`; push workflow `31268310389`
passed both `check` and `browser-host`. Canonical Forms now admit exact scoped
`pool name: member(size = N)` declarations and explicit pool-valued startup
bindings without lexical/global capture. Planning uses exact checked-face
equality rather than nominal kind or revision, then seals the selected
host/boot/capability/resource envelope, finite member queue/sign bounds,
admission grant, and exact consumer placements into Plan identity. Runtime
preparation revalidates those current facts before Play start.

`conduit-kernel` owns the allocation-free keyed member slots, occupation epochs,
lifecycle/failure Sign, immutable key-ordered fan snapshots, bounded
per-branch pressure/outcomes, and ordered merge events retaining source-member
identity. No scalar cord implicitly broadcasts and no pooled output implicitly
merges. The pinned one-worker, zero-retry Chromium proof runs two pages against
one planned 32-peer pool: both join and exchange addressed broadcasts, one
leaves, and the remaining peer continues. The authored
`examples/pool-webchat.conduit` contains no WebSocket, socket, network, address,
or Base fact; the Host selects a bounded loopback Line below source
semantics. This accepts #517 without claiming ambient discovery, unbounded
actors, durable membership, reconnection policy, firmware, or physical/HIL
proof.

R1 #361 is physically accepted at exact main
`33c23c6200331d1f2abc23fcf97b29a4ed780fc2`; push workflow `31332882880`
passed both required jobs. One invocation of
`cargo xtask prove r1-hil --interactive` used one continuously USB-powered Pico
W boot and real operator-controlled Wi-Fi availability. Plan A carried six
terminal/browser keydown and keyup inputs over WebSocket to exact physical LED
Signs. Its physical Line loss left the immutable Plan unsatisfied, requested
ordinary planning, and produced Plan B with a new PlanId and PlayId over the
already-ready USB CDC Line while retaining BodyId, WakeId, Pico HostId, and
BootId. After restoring network availability, Plan C admitted WebSocket and USB
CDC, selected WebSocket, retained one pending offer, and survived the second
physical loss by exact checkpoint reconciliation and automatic USB selection
without planner invocation or Plan/Play identity change. Plan B and Plan C each
delivered the same six correlated physical signs. Both branches quiesced their
active Play, Lulled without deleting the Body, and issued a distinct later
WakeId. The command emitted `combined_physical_acceptance: true` and exited
successfully.

The general R1 Cord/Line boundary from #618 is accepted at exact main
`416bd3afb2f22251a0e7971773a6185e606947d5`; push workflow `31336211081`
passed both required jobs. `CordId`, `LineId`, Base, lower binding, physical
endpoint, and session identities are mechanically distinct. Hosts offer finite
Lines with explicit scope, traffic shape, duplex, ordering, reliability,
framing and capacity bounds, continuation, security, peer binding, and current
availability Sign. Planning seals one exact ordered admitted set into immutable
Plan identity; runtime selection cannot invent an unsealed Line, and changing
availability cannot mutate Form, Cord, or Plan identity.

The existing R1 WebSocket and USB CDC realizations are now distinct Lines to
the same Pico boot and may realize the same semantic Cord. The planner,
runtime, session wire v3, Observatory/Patchbay projections, generated embedded
tables, std/browser adapters, and every selectable Pico composition preserve
the exact Line and lower binding facts. Deterministic tests distinguish
same-Plan bounded continuation from new-Plan recovery, and the accepted #361
physical record supplies the corresponding live proof. This checkpoint adds
no new physical run, transport family, discovery, retry/replay policy, Wi-Fi
Base, public-network security claim, or work deferred to #356.

The canonical ontology migration from #644 is accepted at exact main
`d41f01061daded9c615ea081a40850d200dedc9f`; push workflow `31338153479`
passed both required jobs. Current public Rust, model, schema, CLI, Patchbay,
browser, native-host, firmware, documentation, example, test, and proof-tool
surfaces use Sign for finite machine-readable execution facts and Line for an
exact planned connectivity realization. Info remains the generic value
meaning; the exact Signal value/type family remains intentionally named and
port-specific, while Port and Face identities stay explicit.

This was a clean break: no live Clue/Evidence compatibility API stands in for
Sign, no generic Data/Signal compatibility surface stands in for Info, and no
Carrier alias stands in for Line. Realm, Activation, Deployment, and Provider
do not remain as current semantic aliases; repository search retains those
words only in ordinary language, external/implementation names, or explicitly
historical retirement records. This checkpoint changes vocabulary, not kernel
semantics, bounds, authority, connectivity policy, retry behavior, or proof
class, and adds no new physical/HIL claim.

That R1 physical acceptance does not imply manual fallback selection, Plan
mutation, reconnect policy, discovery, mesh, TLS/public Internet, Zenoh, route
optimization, a Pico-resident planner, or any work owned by #356 or #588. The
core-only optional pre-Play HOLD contract from #646 is accepted separately at
exact main `52850f9d6d0778c7dfe15c376ec92c31682d645c`; push workflow
`31334882531` passed both required jobs. It adds no UI, planner, authority
issuer, active-Play pause, transport, firmware, or physical/HIL claim.

The native Patchbay presentation and semantic-interaction slice from #685 and
#694 is accepted at exact main
`161d1cf6c9974ac5141204aa0c030bc44e2f5aae`; push workflow `31348577927`
passed `check`, `browser-host`, and `conduitos-boot`. The earlier #696 canvas
projects one checked/expanded Form into finite renderer-local Gear, typed Port,
Cord, panel, icon, hit-target, and inspector geometry. Its exact-head Weston
13 Wayland smoke created and rendered the real native window; in-memory pixel,
clipping, identity, and hit-target tests remain deterministic lower proof.

#699 replaces the provisional application mutation behind that canvas with a
finite platform-neutral interaction family. Each semantic selection or
invocation becomes bounded typed Info in a checked two-Gear Form, is planned
with one exact capacity-four Cord, lowered through `conduit-runtime`, and runs
through the production `conduit-kernel` before the admitted generic host
operation may update canonical Patchbay selection or invoke an existing
control seam. Receipts retain distinct source, checked, expanded, Plan, and
ActivePlay identities, the exact Plan, typed success/refusal/failure, and
bounded kernel Signs. Patchbay exposes those Kind, Gear, Port, Plan, Play, and
Sign facts in its own presentation lines.

Pointer hits remain renderer-local geometry and resolve only to an exact
`PatchbaySubjectRef`; graphical arrow keys emit the same semantic selection
request without a second direct editor-selection mutation. The resulting
canonical selected subject is revalidated against the current expanded Form
before the shared inspector reads it. Open Back, Save, view switching, and
Birth/Wake/Plan/Play/Hold/Lull/Stop controls use the same invocation path.
Deterministic negatives keep successful hit/input recognition separate from
semantic completion, distinguish stale, unknown, unavailable, refused, and
failed outcomes, preserve prior selection on refusal, restore interaction
state after malformed request construction, and retain a failed host
completion as machine-readable failure rather than success. The HTML adapter
constructs the same request types without DOM, pixel, Wayland, widget, socket,
or address identity.

The dependency-safe Gear-palette foundation from #747 is accepted at exact
main `bbcf5bf1d08fc6f648966c57e4cb0109cb78d2b2`; push workflow
`31430352301` passed `check`, `browser-host`, and `conduitos-boot`. It derives
the finite supported-nucleus Kind contracts into one authoritative palette,
supports bounded search and native drag/drop, runs placement through the
ordinary interaction kernel, edits canonical Form source, redraws exact typed
Ports, refuses stale or unknown placement, and survives save/reopen. This is a
foundation checkpoint only: at that point #747 still awaited its Cord and
direct-configuration continuations.

The categorized canonical-icon continuation from #748 is accepted at exact
main `5461442076f53afebb46c6060f67b195072a57ae`; push workflow
`31432188828` passed `check`, `browser-host`, and `conduitos-boot`. Every
supported user-facing Kind now has mandatory category, searchable tags, and a
shared icon key outside its semantic contract. Native Patchbay renders five
stably ordered, text-accessible groups from that metadata. Lucide 1.31.0 is
pinned as the one upstream source; the repository retains only nine licensed
SVGs, and `cargo xtask palette-icons` deterministically generates the bounded
native masks. Missing metadata rejects palette construction; missing rendered
data uses one visible, machine-detectable generic fallback. Category and icon
changes do not enter Form, Kind, Gear, Plan, Play, Host, or runtime identity.

This acceptance adds no widget ontology, semantic coordinates, raw-input
Kinds, drag-to-rewire behavior, retry or authority policy, second planner,
runtime, controller, truth store, or authority store. The Weston evidence
proves native surface realization, not automated physical user input; the
semantic interaction proof is deterministic below that renderer-local input
boundary. It adds no firmware, ConduitOS framebuffer, transport, or
physical/HIL claim.

The baseline visual semantic-composition story from #742 is accepted at exact
main `7aba921af0f738712b2b94963c763b3ede4ace3d`; push workflow
`31436226181` passed `check`, `browser-host`, and `conduitos-boot`. The actual
native Patchbay can instantiate palette Kinds as distinct authored Gears, move
and group them as separately persisted presentation state, duplicate their
exact authored configuration, remove them and their dependent Cords, and save
and reopen both the canonical `.conduit` source and bounded layout sidecar.
Coordinates and groups do not enter source, checked, expanded, Plan, Play,
Host, or runtime identity.

Directional Ports expose their exact human-facing Info and temporal
contracts. Dragging from an output to a compatible input creates an authored
Cord; selecting a direct authored Cord and pressing Delete removes its source
statement; drawing the replacement Cord provides the baseline reroute path.
Each semantic edit first becomes a bounded platform-neutral interaction Form,
exact Plan, production-kernel Play, typed receipt, and bounded Signs before it
may atomically replace canonical source and refresh the ordinary checked and
expanded projection. Connectivity, not canvas or statement ordering, remains
the semantic truth and the resulting Form continues through the ordinary
checker and planner paths independently of eventual realization.

Incompatible Info or temporal contracts refuse with maker-legible feedback
and byte-identical prior source. Stale revisions, stale expanded bases,
unknown subjects, duplicate Cords, and attempts to rewrite nested reusable
Face internals also fail closed. Tests cover create, delete, reconnect,
duplicate, Gear removal, movement/grouping identity stability, and
save/reopen through the native production interaction seam.

This is #742's baseline composition proof, not the richer Cord or Gear-Face
editing experience. At that checkpoint #744 still owned catalog-derived compact
configuration controls and #745 still owned selectable bend points, truthful
Cord-parameter projection, direct endpoint manipulation, and bounded reversal.
This acceptance adds no
Host placement, hardware discovery, ROS import/export, scheduler view,
implementation editing, PREWAKE automation, Line/transport selection,
firmware behavior, or physical/HIL claim.

The compact Gear-Face configuration story from #744 is accepted at exact main
`17993bfe50160fc912945ef1aac20d92c09abd24`; push workflow `31438596849`
passed `check`, `browser-host`, and `conduitos-boot`. The finite Patchbay graph
projection derives each control from the Gear's actual expanded checked
configuration and the authoritative standard Kind contract. Native Gear Faces
show the authored value together with its applicable numeric bounds, duration
unit, finite Boolean choices, or text byte bound; the renderer retains no
configuration value of its own. The currently installed canonical standard
catalog exercises placed duration, bounded/ranged numeric, and short-text
controls. A contract-level Boolean vector proves the same projection and native
toggle shape as the catalog permits, with explicit `false`/`true` choices; it
does not claim that the current installed canonical standard catalog contains a
Boolean-configured Gear.

Every native control gesture is encoded inside a finite interaction envelope
and crosses the existing checked interaction Form, exact Plan,
production-kernel Play, receipt, and bounded-Sign path before an edit may
replace source. The edit replaces an exact positional or named invocation
value, or authors a missing default as a named argument, then reparses,
rechecks, and re-expands the complete candidate atomically. Duration spelling,
quoted-text escaping, maximum admitted text, stale source and expanded bases,
unknown fields, wrong types, and contract bounds are covered. Refused edits
retain byte-identical source and report that the proposed value does not fit
the visible Gear-Face type or bounds.

The native pointer entrance is additionally accepted at exact main
`abfb7cf69322247b912c9022b1e85e33d22914ab`; push workflow `31440755159`
passed `check`, `browser-host`, and `conduitos-boot`. Hit testing follows reverse
paint order, so a compact control inside its containing Gear rectangle receives
the pointer gesture before the broader Gear-selection target. The native
vertical renders the production hit geometry, presses their overlapping
coordinates, observes the canonical duration edit, and correlates its ordinary
`ConfigureGear` interaction receipt.

Changing configuration preserves the direct Gear identity while resealing the
source, checked, and expanded identities. An already sealed Plan remains
unchanged; an explicit later planning request produces the replacement Plan.
Moving the same Gear remains presentation-only and leaves those semantic
identities and configuration unchanged. Save and reopen recover the edited
canonical value, while ordinary Cord connection and canvas arrangement remain
available without entering a separate property editor.

This acceptance adds no generic schema-form generator, arbitrary nested-object
editor, expanded inspector requirement, renderer-owned property database,
automatic replan policy, mutable Plan, implementation editor, debugger,
firmware behavior, transport behavior, or physical/HIL claim. At that checkpoint
#745 still owned the richer first-class Cord editing and tuning experience.

The first-class Cord-editing story from #745 is accepted at exact main
`349b00c539d79677a74f097d6e17585122257c44`; push workflow `31443008184`
passed `check`, `browser-host`, and `conduitos-boot`. The native Patchbay lets a
maker select a Cord and drag its body to one finite orthogonal waypoint. That
waypoint is bounded, persisted in the existing layout sidecar by exact source
and sink Port identities, and reconciled away when its semantic Cord no longer
exists. Moving either connected Gear continues to derive the Cord endpoints
from current Gear geometry. Route changes and save/reopen therefore preserve
source, checked, expanded, Cord, and Plan identity.

Dragging the selected Cord to either a compatible output or input reroutes that
exact authored endpoint without reconstructing the surrounding graph. The
gesture crosses the ordinary bounded `RerouteCord` interaction Form, exact
Plan, production-kernel Play, receipt, and Sign path before the source edit may
apply. The editor derives the unchanged opposite endpoint from the exact Cord,
checks direction, Info, temporal contract, duplicates, revision, and expanded
basis, then atomically reparses, checks, and expands the candidate. Successful
rerouting reseals source, checked, and expanded identities; an already sealed
Plan stays immutable until an explicit planning request produces a replacement.
Dragging the same Cord back to its previous compatible Port is the finite
explicit reversal path. Incompatible and stale attempts retain byte-identical
source and remain machine-readable refusals.

Cord inspection shows the current Info and temporal contract directly. The
installed canonical Cord contracts expose no authored Cord parameter fields,
so Patchbay states that no semantic parameters are available and keeps
Line/transport choices on the realization surface instead of inventing
capacity, byte, policy, label, Base, or socket meaning. Existing compatible
Cord creation, selection, deletion, and Gear-relative geometry remain accepted
from #742. Production-pointer tests cover body routing plus both endpoint
directions, ordinary receipt correlation, identity resealing, immutable prior
Plan, explicit replanning, and stale-route reconciliation.

This acceptance adds no transport panel, physical wiring CAD, packet inspector,
renderer-owned topology, generic Cord schema editor, invented Cord parameters,
Line/Base selection, mutable Plan, automatic replan policy, firmware behavior,
or physical/HIL claim.

With the #742, #744, and #745 continuations merged, the complete palette and
Gear-instantiation story from #747 is accepted at exact main
`398ae3d917243c15a692709f9bda9f7fcb8e9146`; push workflow `31443813442`
passed `check`, `browser-host`, and `conduitos-boot`. The finite palette derives
13 supported nucleus entries from authoritative catalog contracts and mandatory
presentation metadata. Every entry exposes its maker-facing name, summary,
typed input/output descriptors, configuration defaults, category, tags, and
canonical icon. Bounded search matches those human names, summaries, Port and
Info identities, configuration keys, categories, and tags without requiring an
exact internal Kind identifier.

Native drag/place requests cross the ordinary interaction kernel before adding
a syntactically valid uniquely named Gear to canonical Form source. Repeated
placement of one Kind yields distinct Gear identities with the same Kind
contract; stale and unknown placement leave source unchanged. The fresh Gear
immediately projects its typed Ports and catalog-derived Face controls, can be
moved through presentation-only coordinates, and can participate in the
accepted compatible-Cord creation and rerouting flows. Save/reopen recovers the
authored Gear instances from canonical source and the coordinates from the
separate bounded layout sidecar; it does not serialize palette metadata as
semantic graph truth. Palette selection still chooses no implementation, Host,
Line, scheduler, or physical device.

This acceptance adds no package marketplace, remote registry, dependency
resolver, code download, implementation installation, Host placement,
automatic hardware discovery, or physical/HIL claim.

The proof-native visual-evidence contract from #821 is accepted at exact main
`68b332af90690ef9fae69403bb4c77557118ef18`; push workflow `31445944339`
passed `check`, `browser-host`, and `conduitos-boot`. The repository-owned
`cargo xtask prove browser-host` entrance writes the versioned
`conduit.evidence-manifest/v1` envelope beneath a deterministic
`target/conduit-evidence/browser-host` root, with an explicit bounded-root
override available to local and CI consumers. GitHub Actions transports no
special semantic format: the same `xtask` implementation owns local and CI
manifest meaning.

Evidence declarations carry exact capture, kind, relative path, media type,
requiredness, scenario and optional step identities. Their provenance can bind
the pinned browser and rendering environment, presentation revision, Plan,
active Play, manifestation and renderer identities, plus the semantic
disposition already asserted before capture. The envelope records the exact
checked-out commit and proof/suite identities without putting wall-clock time
into evidence identity. Existing proof suites without evidence declarations
retain their previous execution behavior.

Before a result can be `complete`, validation confines regular-file outputs
beneath the configured root, rejects traversal, symlink escape, duplicate
identities and paths, more than 64 declarations, files above 16 MiB, and any
missing required declaration, and SHA-256 binds every listed object to its
exact bytes. A failed, interrupted, or incomplete proof writes the distinct
`diagnostic-incomplete` disposition instead of publishable success. The local
negative run exercised that distinction when its WebKit dependency was absent;
the merged-main browser-host job completed the same proof-native manifest path
in the pinned CI environment.

This acceptance defines the evidence and provenance contract only. It emits no
canonical Patchbay screenshot yet and adds no Actions artifact upload, accepted
main gallery, documentation publication, pixel-diff policy, second runtime, or
new source of semantic truth. #822 owns the first deterministic captures.

The deterministic canonical Patchbay captures from #822 are accepted at exact
main `f85e8ae04743d3cf6dc1029568aad8ea8c5ff45e`; push workflow
`31447628162` passed `check`, `browser-host`, and `conduitos-boot`. The existing
HTML Patchbay semantic proof now emits exactly `overview`, `selected-gear`,
`interaction`, `high-contrast`, and `disconnected` PNGs only after the Form,
exact Plan, active Play, Signs, successful selection, ordinary interaction
Plan/Play, presentation-only identity stability, or retained disconnected
state asserted for that image has passed.

One Chromium project is the documentation camera. Playwright and the CI image
are pinned at 1.62.0; viewport is 1440 by 1000 CSS pixels at scale 1 with
`en-US`, UTC, dark scheme, reduced motion, disabled screenshot animation,
hidden caret, and loaded DejaVu Sans. The portable demonstration uses a fixed
fixture Host and boot identity through the existing injectable Host
configuration seam. Two independent local Chromium processes produced the same
presentation, Plan, active Play, and manifestation identities and byte-identical
SHA-256 values for all five images. Exact identities remain visible and
provenance-bound rather than masked or redacted.

After each capture Chromium atomically refreshes the bounded `captures.json`
declarations. `xtask` rejects stale prior outputs, imports only root-confined
relative paths, requires all five identities after semantic success, and binds
the declaration document and every PNG into the ordinary #821 manifest with
exact byte length, SHA-256, browser version, camera, scenario, presentation,
Plan, active Play, manifestation, renderer, and asserted semantic disposition.
The merged-main `browser-host` job completed that same manifest path. A local
missing-WebKit-library run instead retained the successful Chromium objects in
a `diagnostic-incomplete` manifest, proving partial compatibility execution
cannot become canonical success.

Firefox and WebKit still execute the same semantic compatibility test in the
pinned CI image but never write canonical evidence. This acceptance adds no
pixel-diff policy, Actions upload, accepted-main gallery, documentation
publication, ConduitOS screenshot, second runtime, or pixel-derived semantic
claim. #823 owns transport of the already-validated proof directory.

The exact-head pull-request evidence transport from #823 is accepted at exact
main `819a2df287714a319ab59f37414ef48f5d0e07cb`; push workflow
`31449085682` passed `check`, `browser-host`, and `conduitos-boot`, including
independent complete-evidence verification at the checked-out main commit. PR
workflow `31448877430` first passed the same semantic proof and verifier, then
uploaded artifact
`conduit-visual-evidence-f7b0fbdbb856a10485975914e6c07724b5e4baa3` for its
exact checked pull-request merge commit. The artifact expires after the
explicit 14-day review retention period.

The repository-owned `cargo xtask evidence verify` entrance reopens the
versioned manifest before transport and requires the requested complete or
diagnostic-incomplete disposition, exact 40-character checked commit, proof
and suite identities, reviewed count and per-file size bounds, unique IDs and
paths, regular root-confined files, exact recorded lengths, and recomputed
SHA-256 values. It rejects symlinks and undeclared files. Complete
`browser-host` evidence additionally requires the declaration document and
exactly the five required PNG identities from #822, with non-empty semantic
and canonical-camera provenance sufficient to reconstruct what each image
depicts.

The accepted PR artifact was downloaded independently and contained exactly
`manifest.json`, `captures.json`, and the overview, selected-gear,
interaction, high-contrast, and disconnected PNGs. Reverification of those
downloaded bytes succeeded against commit
`f7b0fbdbb856a10485975914e6c07724b5e4baa3`. A real local browser-proof failure
also produced a 313-byte diagnostic-incomplete manifest that passed only the
diagnostic verifier; requesting complete verification failed closed.

The semantic browser proof remains the job's acceptance gate. PR execution
retains only `contents: read`; artifact upload receives no repository-content,
Pages, documentation, or accepted-main mutation authority. Successful and
failed runs use visibly distinct artifact names and manifest dispositions.
Main push CI verifies complete evidence but deliberately skips PR artifact
transport. This acceptance adds no stable gallery, README or docs publication,
pixel-regression gate, screenshot-derived semantic verdict, or long-term
history policy. #824 owns accepted-main gallery publication.

The typed decision-Kind slice from #776 is accepted at exact main
`b14d83742205f8dfd54e10d22a5eb90ea2333f79`; push workflow `31445629459`
passed `check`, `browser-host`, and `conduitos-boot`. The portable catalog now
defines one-shot `logic/compare`, `logic/not`, and scalar `logic/select` Ports
using only exact `value/scalar@1` and `value/bool@1` Info. Comparison admits the
finite configured set `lt`, `le`, `eq`, `ne`, `ge`, and `gt`; Boolean input is
canonical and never coerced; select requires both candidates to have the same
complete scalar Value contract.

The std Host advertises exact revisions, implementations, artifacts, and finite
limits for the three Kinds. One ordinary seven-Cord Form plans, lowers, and
executes compare, not, and select together through the production
`conduit-kernel` with capacity-one pressure and zero successful post-Play
allocations. Scalar minimum, maximum, and equality boundaries cover all six
operators. Unknown selection retains both finite candidates until a canonical
selector arrives; success transfers the selected exact value identity and
atomically releases the other. Closure before a decision, noncanonical Boolean
input, incompatible branches, cancellation, unsupported configuration, and a
mutated selected implementation remain deterministic terminal outcomes or
fail-closed refusals.

Patchbay derives the finite operator control and legibility metadata from the
authoritative contract. This acceptance adds no expression language, predicate
or callback registry, truthiness, three-valued logic, erased-value select,
browser or firmware implementation, transport, physical actuation, or HIL
claim. ConduitOS truth remains narrower: its gap inventory includes all three
revisions as missing and advertises none of them.

The bounded scalar-math slice from #777 is accepted at exact main
`38ccb4030f098ddbf1f8b3df0142bb906cc1465a`; push workflow `31450091681`
passed `check`, `browser-host`, and `conduitos-boot`. The portable catalog now
defines one-shot `math/clamp`, `math/scale`, and `math/deadband` contracts with
exact `value/scalar@1` input and output Ports. Signed raw microunit
configuration is represented directly in checked Forms, Plans, generated fixed
images, and stable identities. Clamp uses inclusive minimum and maximum bounds;
scale uses the accepted Scalar fixed-point checked multiplication, including
its truncation and overflow behavior; deadband emits zero at and inside a
nonnegative symmetric radius and preserves values outside it, including
`Scalar::MIN` without absolute-value overflow.

The std Host advertises exact revisions, implementations, artifacts, and finite
limits for all three Kinds. One ordinary capacity-one
`deadband -> scale -> clamp` Form checks, plans, lowers, and executes through the
production `conduit-kernel` with exact eight-byte values, one admitted operation
slot per transform, stable preallocated value storage, and zero successful
post-Play allocations. The portable no-std functions and installed operation
vectors agree on clamp boundaries, scale overflow, and inclusive deadband
behavior. Invalid signed configuration and mutated executable identity refuse
before Play; one-shot input closure, cancellation, malformed completion, and
attempted work outside the single admitted lifecycle remain explicit terminal
outcomes rather than invented output or retry.

Patchbay derives Transform-category metadata, compact signed numeric controls,
and the microunit label from the authoritative configuration schema. Carrying
signed configuration in a generated fixed image is representation proof only,
not a firmware math implementation. This acceptance adds no interval remapping,
calculator or expression evaluator, arbitrary units engine, matrix library,
PID controller, trajectory planner, physics engine, browser or firmware
implementation, transport, physical actuation, or HIL claim. ConduitOS truth
remains narrower: its gap inventory includes all three math revisions as
missing and advertises none of them.

The explicit planned-time debounce/timeout slice from #779 is accepted at
exact main `6dd23917ded0170d1713eb826051c653508354e6`; push workflow
`31457216826` passed `check`, `browser-host`, and `conduitos-boot`. Its main
implementation merged as `7df3462f989220da3702a62118c69ecb11431274`, whose
push workflow `31455996699` passed the same required jobs. The portable catalog
now defines exact `time/debounce` Boolean Current-to-Current trailing semantics
and exact `time/timeout` tick Flow-to-Boolean Current inactivity semantics.
Both bind the admitted monotonic-millisecond Host operation and timer resource,
accept only durations from zero through 86,400,000 milliseconds, and retain
finite one-item/eight-byte queue limits. Debounce admits at most eight values;
timeout admits at most two state values, emits false initially, true once after
inactivity, and false again on recovered activity.

The std Host advertises exact revisions, implementations, artifacts, and finite
limits for both Kinds. Representative robot Forms check, plan, lower, and run
through the one production `conduit-kernel` with stable preallocated value
storage and zero successful post-Play allocations. Repeated virtual schedules
produce identical deadlines, outputs, observations, receipts, and kernel Signs;
the conformance set covers zero and maximum duration, simultaneous input and
deadline, late wakes, debounce burst/reset and terminal pending flush, timeout
expiry/recovery/reset, exact deadline cancellation, and unavailable or
regressed monotonic Base. Patchbay derives the bounded duration, trailing-policy,
and maximum-value controls from the authoritative configuration schema.

This acceptance adds no `value/any`, wall clock, scheduler-order timing,
async/callback runtime, delay, throttle, timer wheel, retry policy, browser or
firmware implementation, live transport, physical timing, or HIL claim.
ConduitOS truth remains narrower: its current inventory classifies both exact
revisions as missing and advertises neither one.

The first ConduitOS boot slice from #588 is accepted at exact main
`35a7522703164cdc1758a3bfebfd5ac3f0649a0e`; push workflow `31340517738`
passed `check`, `browser-host`, and `conduitos-boot`. The architecture-neutral
common backbone compiles for the pinned Limine IA-32, x86_64, aarch64,
riscv64, and loongarch64 target matrix while x86_64 is the sole executable
backend. Exact Limine 12.5.2 bootstrap produces the same six-file hybrid ISO
digest across two builds, boots twice in the one-CPU 64 MiB headless q35 QEMU
profile, emits one bounded accepted boot Sign per run with fresh HostId and
BootId, and exits deterministically. This earns `freestanding-emulator` boot
proof only. At that checkpoint it started no Plan or Play, gave no machine to
`conduit-kernel`, activated no non-x86_64 backend, and claimed no firmware,
physical/HIL, interrupt, timer, serial-offer, framebuffer, SMP, preemption,
network, or ConduitOS inspection proof.

The ConduitOS machine and production-kernel slice from #588 is accepted at
exact main `feae63bbf6d392c468526e9cae352fddb2b03b74`; push workflow
`31342521260` passed `check`, `browser-host`, and `conduitos-boot`. The x86_64
backend now admits one 256 KiB runtime arena and finite boot-scoped Memory,
Clock, Timer, Serial, Interrupt, Idle, and ExecutionLane Bases, resources,
capabilities, exact typed ports, host-operation slots, IRQ facts, values,
routes, and Signs before kernel execution. The sole cooperative lane belongs
to the production `conduit-kernel` fixed scheduler. On each of two
reproducible one-CPU 64 MiB q35 boots, one real PIT interrupt captured one
bounded fact, later woke one exact kernel interest outside interrupt context,
advanced a two-operation profile across one exact Cord, produced one bounded
COM1 presentation, and terminated with zero pending host operations. Fresh
HostId, BootId, and seven distinct Base identities correlate the bounded boot
and kernel Signs.

Deterministic negatives keep cancellation, Base failure, full timer slots,
full IRQ and Sign storage, masked interrupts, stale or duplicate wakes,
malformed offer bounds or ports, stale CPU observations, missing ISA features,
and artifact/offer feature disagreement distinct and fail-closed. The profile
is deliberately hand-lowered proof input, not a Form, Plan, or Play. This
checkpoint remains `freestanding-emulator` proof: it adds no allocator,
ordinary checking/planning/lowering, preemption, SMP, APIC/IOAPIC,
framebuffer, network, transport, non-x86_64 execution, physical/HIL claim,
ConduitOS inspection surface, or second runtime. Those remain owned by later
#588 slices.

The ConduitOS ordinary-Form slice from #588 is accepted at exact main
`4d368a8f55197fcc7416ea82f2ffca5b61ce830e`; push workflow `31345447440`
passed `check`, `browser-host`, and `conduitos-boot`. On each of two fresh
x86_64 QEMU boots, ConduitOS checks the same authored
`time/tick -> presentation/tick` Form, constructs a finite boot-scoped host
advertisement, plans exact placements and a capacity-one eight-byte Cord,
lowers the resulting fragment into production `conduit-kernel` tables, issues
an ActivePlay, services one real PIT wake through an admitted Timer operation,
and presents semantic result `tick-sequence-0` through the admitted bounded
Serial operation. Source-document, checked-form, and expanded-form identities
remain stable across boots; Host, Boot, Plan, fragment, ActivePlay, and Base
identities remain exact and boot-scoped.

The shared Form, planner, lowering, and standard-catalog path runs here under
`no_std + alloc`; a 256 KiB boot arena admits all semantic preparation and is
sealed before Play. Both exact-main boots reported 80,422 bytes allocated
before and after Play, zero pending host operations, and finite Cord, Sign,
timer, serial, interrupt-fact, route, value, and operation storage. A std-host
compatibility vector proves that materially different implementations produce
different exact Plans while preserving the same Form result. Deterministic
negatives keep stale boot and Plan identities, unavailable implementations,
insufficient memory/Timer/Sign/Cord budgets, cancellation and late wake, and
Timer Base failure distinct and fail-closed.

This remains `freestanding-emulator` proof. It adds no Observatory/Patchbay
inspection surface, full Rust standard library, preemption, SMP, APIC/IOAPIC,
filesystem, framebuffer, network, transport, non-x86_64 execution, firmware,
or physical/HIL claim. The earlier hand-lowered profile remains only a named
P2/P3 regression fixture; production boot follows the ordinary Form pipeline
and the single `conduit-kernel`.

The ConduitOS ordinary-inspection slice from #588 is accepted at exact main
`b810259296c64052ca19b9fdf2e1f3837c36c877`; push workflow `31347392349`
passed `check`, `browser-host`, and `conduitos-boot`. After the admitted
ordinary Play reaches its successful terminal state, each of two fresh x86_64
QEMU boots exports one bounded Observatory v2 snapshot through the guest
serial proof seam. The actual native Patchbay binary validates the retained
snapshot and produces a deterministic 42-line linear projection without a
ConduitOS-specific privilege or runtime path.

Each snapshot carries the exact Host and Boot advertisement, two capability
offers, four resource pools, seven initialized Bases, one Plan and fragment,
two placements, one capacity-one eight-byte Cord, one completed ActivePlay,
four current terminal Signs, six retained historical lifecycle Signs, and
zero Sign gaps. Source-document, checked-form, and expanded-form identities
remain stable across boots while Host, Boot, Plan, fragment, ActivePlay, Base,
and Sign identities remain exact and fresh where their lifetimes require it.
The guest admits at-most-64-KiB report storage inside the 256-KiB boot arena
before Play; both exact-main boots reported 196,752 bytes allocated before and
after Play. Sign retention is explicit at 10 of 64 items with zero drops.

Limine 12.5.2, firmware environment, adapter revision, image/build identity,
normalized memory summary, optional boot artifacts, Plan artifacts, and
framebuffers are represented only as sealed boot provenance. They are visibly
separate from current Host offers and Bases and confer no membership, trust,
authority, or availability. Validation fails closed on stale or duplicate
Bases or provenance, duplicate Signs across current and historical retention,
invalid retention bounds, bad framebuffer Base references, and drifted
Plan/Play/Line identities.

This remains `freestanding-emulator` proof. It adds no second report store,
Patchbay backdoor, runtime control, QEMU-memory inspection, framebuffer,
network, transport, complete `conduit.std` profile, non-x86_64 executable
backend, firmware, or physical/HIL claim. The active P0-P5 #588 spine is now
implemented; frozen architecture, profile, lane/preemption, and platform
breadth remain dormant until deliberately promoted. The final #588 closure
audit remains separate from this acceptance record.

The first exact local timing profile from #706 is accepted at exact main
`bd1fdebf3c22da0553743bedfdaeb886078c56b2`; push workflow `31394899842`
passed `check`, `browser-host`, and `conduitos-boot`. The unchanged
platform-neutral `time/tick -> presentation/tick` Form is paired with one
boot-scoped deterministic clock/timer/execution offer. Planning seals a finite
730 µs worst-case timing and resource basis for a 1,000 µs deadline and
refuses an otherwise-compatible 100 µs request before an ActivePlay exists.

The exact basis includes arena, capacity-one eight-byte Cord, wake and timer
slots, Base scratch, mandatory Sign storage, and fault reserve. The admitted
Play executes through the existing fixed `conduit-kernel` scheduler with zero
successful heap allocations after Play entry. Exact Plan/Play-correlated Signs
keep deadline met, deadline miss, Timer Base loss, cancellation, and stale
timing basis distinct. Optional inspection is excluded from the strict path.

This remains `deterministic-emulator` proof. It adds no universal hard-real-time
claim, physical timing evidence, remote guarantee, mixed-criticality
framework, work stealing, CPU migration/hotplug, generic schedulability
optimizer, RTOS, or second scheduler. A physical timing claim requires a
separate pinned-hardware proof.

The ConduitOS portable-std gap inventory from #709 is accepted at exact main
`5c9d15295de20268b8eacc8b24852b91cea1610a`; push workflow `31393875458`
passed `check`, `browser-host`, and `conduitos-boot`. The bounded
`conduit.conduitos/std-gap@1` report derives all ten entries directly from
`supported_nucleus_contracts()` and their matching canonical offers, seals
revision, face, limits, and semantic content into SHA-256 digest
`c30c6b445f19dbbf79abc4c7ffd3d1dd76f6ced66844d2b895dd35af80b46688`,
and carries the exact ConduitOS build and
`conduitos/single-lane-cooperative@1` profile basis.

The exact accepted result classifies `time/tick@2` and
`presentation/tick@1` as implemented and the remaining eight supported
contracts as missing. Comparison uses canonical kind and contract revision;
legacy `value/any` compatibility rows never enter the supported-nucleus
inventory. Mutation vectors change the catalog digest, while Host build
identity remains a separate report basis.

This is an inventory and gap proof only. It advertises no missing capability
and changes no semantic implementation, scheduler, Base, planner, runtime,
device/network behavior, or physical claim. Follow-on work must promote one
small implementation family rather than reopening an aggregate mega-slice.

The first ConduitOS portable text slice from #728 is accepted at exact main
`59e76286cd6930e6bfbb9d53e008e33fb871d1d2`; push workflow `31419594904`
passed `check`, `browser-host`, and `conduitos-boot`. The unchanged authored
Form contains only the installed `conduit.std/text-literal@1` and
`conduit.std/presentation-text@1` meanings. Ordinary source checking,
canonical expansion, exact planning, numeric lowering, and the production
`conduit-kernel` carry its exact 20-byte UTF-8 value through a capacity-one,
20-byte Cord to the admitted serial presentation operation.

The Plan reuses the accepted single cooperative execution-lane realization
from #711 and seals the exact text implementations, build artifact, memory and
serial Bases, value/Cord bounds, placement identities, Play, and terminal
Signs. One real x86_64 QEMU boot presents exactly `Hello from ConduitOS` and
reaches its bounded terminal state. Observatory and native Patchbay retain the
same realization facts without becoming their source of truth.

Stale boot, offer, and Plan identities; unavailable implementations or Bases;
undersized value, Cord, memory, and Sign storage; malformed or oversized text;
presentation failure; cancellation; and malformed host completion remain
distinct refusals, failures, or Signs. The derived std-gap is now four
implemented and six missing contracts. This remains `freestanding-emulator`
proof for one x86_64 ConduitOS text program; it adds no browser, firmware,
physical/HIL, non-x86, real-time, universal-console, terminal, framebuffer,
filesystem, network, `text/upper`, or additional-lane claim.

The bounded ConduitOS `text/upper` slice from #730 is accepted at exact main
`755426dbd2ca238b60446f2bc71a29274de9bb09`; push workflow `31422614087`
passed `check`, `browser-host`, and `conduitos-boot`. The unchanged authored
Form contains only the installed `conduit.std/text-literal@1`,
`conduit.std/text-upper@1`, and `conduit.std/presentation-text@1` meanings.
Ordinary source checking, canonical expansion, exact planning, numeric
lowering, and the production `conduit-kernel` carry the value through two
distinct capacity-one, 256-byte Cords to the admitted bounded uppercase and
serial presentation operations.

The exact Plan reuses the accepted `conduitos/single-lane-cooperative@1`
realization, seals three placements and both Cord identities, and selects
`conduitos/kernel-text-upper@1` from the truthful boot-scoped offer. Its fixed
512 KiB boot arena reports 303,408 bytes used before Play and the same value
after terminal completion; the realized Plan reserves 12,288 runtime-memory
bytes. One real x86_64 QEMU boot presents exactly `HELLO, CONDUITOS` and
reaches its bounded terminal Signs. Observatory and native Patchbay project
the same three placements, two Cords, bounds, identities, and outcome without
owning runtime state.

Conformance includes `ǰ` expanding from two UTF-8 bytes to the three-byte
uppercase sequence `J` plus combining caron. A full 256-byte input of that
specimen refuses output expansion instead of truncating, wrapping, partially
succeeding, or replacing data. Malformed UTF-8, unavailable implementation,
insufficient output storage, cancellation, stale identity, and presentation
Base loss remain distinct outcomes. Shared std-host conformance checks the
same checked face and normalized semantic result while retaining different
implementation and artifact identities. The derived std-gap is now five
implemented and five missing contracts.

This remains `freestanding-emulator` proof for exactly one portable transform
on x86_64 ConduitOS. It adds no `text/join`, state/count, file/copy, locale
collation, case folding, normalization, browser manifestation, physical/HIL,
non-x86 realization, second scheduler, or second execution lane.

The two-region ConduitOS cooperative slice from #731 is accepted at exact main
`792f4bfbc843f716bf0c67272eacd7fc620eaa21`; push workflow `31427213079`
passed `check`, `browser-host`, and `conduitos-boot`. One unchanged authored
Form contains an independent `text/literal -> text/upper -> presentation/text`
branch and `time/tick -> presentation/tick` branch without lane, Base, CPU,
thread, scheduler, preemption, isolation, or platform facts.

Ordinary checking, expansion, planning, numeric lowering, and one production
`conduit-kernel` scheduler realize five placements and three bounded Cords. The
Plan seals exact disjoint `region/text` and `region/timer` membership, distinct
capacity-one execution-lane resources backed by one finite two-unit Base,
region-local resource budgets, cooperative bounded-step scheduling, and
explicit false preemption and isolation. Changing membership, lane selection,
lane capacity, pool or Base identity, or any sealed finite budget changes Plan
identity and fails current-offer validation; unavailable or duplicate lanes,
an undersized Sign reserve, and a validly resealed one-lane lie refuse before
Play.

The QEMU execution retains one admitted timer interest while the text region
makes meaningful progress through exact `HELLO, CONDUITOS` presentation, then
consumes the timer wake and reaches bounded terminal Signs. The kernel Sign
reports two regions, two lanes, five logical operations, 36 kernel Signs, one
timer wake, two serial presentations, `overlap_witness=true`,
`timer_pending_during_text_progress=true`, and `physical_parallelism=false`.
Its 1 MiB boot arena reports 475,144 bytes allocated both before and after
Play. Observatory and native Patchbay project both regions and the explicit
overlap observation without becoming scheduler or runtime truth.

This remains `freestanding-emulator` proof of logical cooperative overlap on
one x86_64 execution mechanism. It adds no SMP, physical parallelism, second
scheduler, context switching, preemption, isolation, affinity, migration,
threads, processes, transport, physical scheduling, or HIL claim. Existing
one-region Forms and the canonical strict timing projection remain valid.

The ConduitOS cooperative execution-region slice from #711 is accepted at
exact main `354c6c27a24aa87c71e4ad7ca45b486fccdae9b2`; push workflow
`31398132917` passed `check`, `browser-host`, and `conduitos-boot`. The
unchanged ordinary platform-neutral Form now produces one exact Plan region
containing its two admitted placements, the
`conduitos/cooperative-bounded-step@1` profile, one selected boot-scoped
execution-lane resource and initialized Base identity, and explicit false
preemption and isolation requirements.

The region seals the already-planned 8,192 runtime-memory bytes, one timer
slot, capacity-one eight-byte Cord, and mandatory Sign item/byte budgets.
Core Plan verification binds those facts into fragment and Plan identity;
ConduitOS additionally revalidates the single lane against the current Host
offer before lowering or Play. A validly resealed two-lane mutation and an
unavailable lane both refuse before Play. Observatory and native Patchbay
render the same immutable region through the ordinary linear projection.

This remains `freestanding-emulator` proof of the already-accepted single
cooperative lane. It adds no per-Gear task, thread, process, or context
identity; SMP, second lane, context switching, preemption, isolation, new
scheduler, transport, physical scheduling, and HIL remain outside the claim.

The canonical portable Boolean toggle slice from #900 is accepted at exact
main `bb316a547af746ef0057dcd9ac3f13c2a6fb0abf`; push workflow
`31464280116` passed `check`, `browser-host`, and `conduitos-boot`. One
`conduit.std/state-toggle@1` contract now defines exact closing Tick input,
Boolean Current output, initial state, finite transitions, pressure, closure,
cancellation, malformed-input failure, and terminal behavior. The ordinary
std profile installs `std/kernel-state-toggle@1` through planning, lowering,
and the production kernel with every possible emitted Boolean Value admitted
before Play.

The distributed browser demo remains intact as a consumer of that same
contract: its authored Form uses `state/toggle` and `presentation/bool`, the
browser receives canonical Boolean values after remote-ingress kernel
execution, and pinned Chromium proves sixteen ordered presentations, one real
pressure retry, and a distinct broken-link failure. The former Signal-only
revision, trigger value, implementation identity, and artifact identity are
absent; no compatibility facade or second toggle meaning remains active.

This exact-main result also supplies the finite missing-family promotion
required by #883. The generated Host x Kind report from #893 automatically
contains the toggle row: the ordinary std and bounded Signal profiles expose
exact direct cells, browser exposes the exact Boolean presenter cell, and
other profiles remain explicit missing or unsupported obligations. Semantic
support, installed realization, and intentionally supplied current-offer
truth remain separate; the matrix is explanatory input and never planner or
Boot truth. This adds no latch, selector, merge, UI state store, generic
reducer, `value/any`, universal Host coverage, or physical proof.

The bounded ConduitOS xHCI Base slice from #805 is accepted at exact main
`ccc26d3a0597d3254e58fcc5c106795f48d01298`; push workflow `31456936589`
passed `check`, `browser-host`, and `conduitos-boot`. The ConduitOS job ran the
dedicated `cargo xtask conduitos xhci-proof --locked` entrance and retained its
SHA-named `xhci-proof.json` separately from the ordinary console evidence.

One pinned x86_64 QEMU `qemu-xhci` controller is discovered at its exact PCI
function and BAR, mapped through dedicated static page-table storage, and
realized as one boot-scoped Base. Fixed aligned storage owns the DCBAA, one
16-TRB command ring, one 16-TRB event ring, and one ERST entry. Bounded
halt/reset/start polling and one real No-Op command completion establish
controller progress. Exact controller identity, hardware and admitted slot
limits, ring capacities, DMA bytes/alignment, pending-operation capacity, and
poll budgets are carried in the proof report and Sign.

A real controller-absent QEMU boot and deterministic malformed-controller,
capacity, timeout, and completion vectors refuse without manufacturing a usable
Base. The accepted Base advertises no semantic keyboard capability, and the
pre-existing ordinary Form still executes through the same production kernel.
That Base-only acceptance was `freestanding-emulator` proof and did not itself
claim USB device enumeration. HID report parsing, key events, keymaps, chords,
hotplug, external transport, physical devices, and HIL remain unclaimed and
owned by later #804 milestones.

The bounded ConduitOS USB-enumeration slice from #806 is accepted at exact main
`b846832a5692d63b010059081a46084dab16d35c`; push workflow `31459165964`
passed `check`, `browser-host`, and `conduitos-boot`. The ConduitOS job ran the
dedicated `cargo xtask conduitos usb-proof --locked` entrance and retained its
SHA-named `usb-proof.json` separately from ordinary console and xHCI evidence.

One real root-attached QEMU USB device is reset, assigned one xHCI slot and USB
address, queried through five finite EP0 control transfers, structurally parsed,
and configured through the already-accepted xHCI Base. The resulting Sign and
report bind the exact controller Base, boot, attachment epoch, root port, slot,
address, device instance, first interface, and first endpoint identities. They
also retain the fixed configuration/interface/endpoint/descriptor maxima,
single outstanding transfer, zero retries, 16-TRB transfer ring, aligned DMA
storage, polling budget, and mandatory Sign capacity.

A second real QEMU boot with the controller present and device absent refuses
with `usb-device-absent`. Deterministic conformance keeps malformed and
oversized descriptors, exhausted interface/endpoint/record capacities,
unsupported topology, reset failure and timeout, device vanish, stall, transfer
error and timeout, wrong controller/slot/endpoint completion identity, and stale
same-boot reattachment identity distinct and machine-readable.

This is `freestanding-emulator` structural enumeration proof only. It adds no
hub traversal, hotplug policy, HID descriptor or report parsing, semantic
`input/keyboard` offer, key events, keymaps, chords, external transport,
physical device, or HIL claim; those remain owned by later #804 milestones.

The bounded ConduitOS HID boot-keyboard slice from #807 is accepted at exact
main `26e701367ade28c5ceb2dcd1e07aab3dcbedece1`; push workflow `31461078260`
passed `check`, `browser-host`, and `conduitos-boot`. The ConduitOS job ran the
dedicated `cargo xtask conduitos hid-proof --locked` entrance and retained its
SHA-named `hid-proof.json` separately from ordinary console, xHCI, and USB
enumeration evidence.

One real enumerated QEMU USB keyboard is matched by exact HID class, Boot
Interface subclass, keyboard protocol, and one bounded interrupt-IN endpoint.
The accepted xHCI path issues `SET_PROTOCOL`, configures that endpoint, and
keeps two fixed report buffers and two outstanding transfers admitted. The
proof harness sends acknowledged key-down and key-up actions through QEMU QMP;
ConduitOS receives two real eight-byte boot reports and deterministically emits
usage `0x04` Pressed then Released, with the exact controller, device,
interface, endpoint, transfer, attachment, and modifier facts retained.

Report parsing rejects reserved bytes, rollover/error usages, duplicate usages,
short reports, wrong completion identities, device removal, and transition
pressure without fabricating keys. Storage and work remain bounded by one
4 KiB HID DMA area, a 16-TRB transfer ring, two report buffers, 1,024 finite
poll windows, twenty transitions per report, and eight retained Sign slots.

This is `freestanding-emulator` HID-local transition proof only. It adds no
semantic `input/keyboard` Host offer or portable key-event contract, keyboard
layout, character or Unicode conversion, general report-descriptor parser,
NKRO, consumer/media keys, LEDs/output reports, browser input, hotplug
replanning, external transport, physical device, or HIL claim. Those remain
owned by later #804 milestones.

The portable key-event and keyboard-source contract slice from #808 is accepted
at exact main `8566d652dd891a406fb9f3886d74bce04253af44`; push workflow
`31462611125` passed `check`, `browser-host`, and `conduitos-boot`. The portable
core and catalog compile for `thumbv6m-none-eabi` without default hosted
features, and the full matrix retained the existing xHCI, USB-enumeration, and
HID-local proofs unchanged.

`input/key-event@1` has one canonical three-byte encoding: a reviewed HID
Keyboard/Keypad usage number as platform-neutral key vocabulary, a canonical
Pressed/Released tag, and an eight-bit Left/Right Control, Shift, Alt, and GUI
snapshot after the transition. Using that vocabulary does not require a USB
Base. Wrong widths, noncanonical transitions, reserved usages, and modifier
press/release values inconsistent with the after-transition snapshot refuse.
The value carries no character, Unicode, locale, layout, compose, IME, USB,
device, endpoint, DOM, or toolkit identity.

`input/keyboard` is an exact no-input source with one `key` output of temporal
shape `Flow { closes: true }`, an eight-item/twenty-four-byte queue, one input
resource, and one maximum-in-flight bounded next-key-event host operation. The
public A, Shift+A, left/right modifier, and numeric simultaneous-key vectors
cross the fixed production scheduler unchanged. A capacity-one three-byte Cord
waits under pressure without overwrite or drop; successful closure,
cancellation, and host-input failure remain machine-distinct.

Patchbay derives the Input category, summary, tags, and pinned generated
keyboard icon from authoritative catalog metadata without changing semantic
identity. No current Host offer was added: the contract is authorable and
discoverable, while planning still requires a later truthful implementation.
This acceptance adds no concrete ConduitOS/USB, browser, native, PS/2, or
Bluetooth binding; keymap, text/Unicode, IME/compose/dead keys, repeat policy,
LED/output reports, shortcuts/hotkeys, discovery, physical keyboard, and HIL
remain outside the claim and are owned by later #804 milestones.

The truthful ConduitOS keyboard-binding slice from #809 is accepted at exact
main `fa642fb55a7a4c9021a6d9e7b2fb4c8409dfec37`; push workflow
`31465190414` passed `check`, `browser-host`, and `conduitos-boot`, including
the separately retained `keyboard-proof.json` emulator record.

ConduitOS publishes `input/keyboard` only after the boot-local xHCI Base, USB
device, HID Boot Keyboard interface, interrupt-IN endpoint, and fixed report,
transition, and operation storage are ready. The offer uses exact stable
implementation `conduitos/usb-hid-keyboard@1`, execution profile
`conduitos/usb-input-cooperative@1`, and current build artifact identities.
Controller, device, interface, and endpoint identities remain generic planned
resource bindings rather than portable Kind or Info facts. A real QEMU boot
without the USB device refuses before any keyboard Sign or offer exists.

The authored Form names only `input/keyboard`. Ordinary checking, placement,
planning, lowering, active-Play binding, and one fixed production scheduler
carry the exact Host, Boot, offer generation, implementation, artifact, eight
resource reservations, and capacity-one/three-byte Cord. The acknowledged QMP
key action completes real HID transfers, then the responsibility-named bridge
produces exact portable values `[4, 0, 0]` and `[4, 1, 0]`; those values contain
no USB, controller, endpoint, QEMU, architecture, layout, or Unicode fact.

Absent or incompatible device truth, ambiguous HID candidates, stale Boot and
artifact identities, insufficient resources, malformed semantic values, Cord
pressure, cancellation, USB transfer failure, device loss, and normal closure
remain distinct. The ordinary Observatory snapshot advertises the same exact
finite realization and native Patchbay consumes it read-only. This slice adds
no keymap/text conversion, selection UX, multi-keyboard aggregation, hub,
hotplug replanning, browser implementation, physical-device claim, or HIL
proof; those remain owned by later #804 milestones.

The no-Form local rescue slice from #816 is established at exact main
`5660cabb8d07044c8e367c4b2dc27346492e33d4`; push workflow `31470574806`
passed `check`, `browser-host`, and `conduitos-boot`, including the separately
retained `rescue-proof.json` and correlated serial transcript. PR-head workflow
`31470268374` passed the same complete matrix.

The ordinary interactive x86_64 composition observes only opaque transitions
produced after HID Boot reports pass reserved-byte, rollover, duplicate-usage,
completion-identity, device-presence, and finite-capacity checks. The
local-authority constructor is crate-private and is crossed only by that HID
transition type; portable text or remote `input/key-event` values cannot invoke
it. The tap runs before any ordinary keyboard offer, Plan, or Play exists.

Either left or right Control plus either left or right Alt and the actual HID
Delete usage admits one request under policy
`conduitos/local-physical-rescue@1` for operation
`conduitos.machine/reboot@1`. A held Delete is latched to one request until
release. The receipt binds the old BootId, local-physical authority, policy,
operation, request identity, and absence of an ordinary keyboard Plan. The
finite HID session may observe at most four successive reports while reusing
the same two admitted report buffers and fixed transfer ring.

The x86_64 reset Base performs a finite controller-readiness check and emits
exactly one guest reset command. Controller busy and a reset command that
returns are explicit failures; the old boot cannot report completion. The
runner observes B1, the request, disappearance/reset, and a fresh B2 with
`B2 != B1` while the original QEMU process remains alive, then terminates that
process only as post-proof cleanup. Ctrl+Delete, Alt+Delete, and
Ctrl+Alt+Backspace physical injections remain in B1 without a rescue receipt;
malformed HID, held-key repeat, disabled policy, unavailable reset Base, and
stale/same-Boot correlation are deterministic negative cases.

This is low-level local rescue while the CPU, xHCI/USB/HID service, and reset
Base remain responsive, not a hardware NMI or completely-frozen-machine claim.
The additional required demonstration while the ordinary K6 Play is active is
not yet established because #812 depends on the still-open keymap work. #816
therefore remains open, and no active-Play independence or final K9 acceptance
is claimed here.

The documentation-only CI fast path from #863 is accepted at exact main
`965e5ef34b87d180a13fd7cf88e70331a416e40e`; push workflow `31453748480`
classified the workflow and helper changes as executable, then passed the
unchanged required `check`, `browser-host`, and `conduitos-boot` jobs. The
separate Pages workflow `31453748440` also completed for that exact commit.

Only a non-empty exact base-to-head change set whose every tracked path ends
in `.md` may take the fast path. Source, workflow, manifest, lockfile,
image/baseline, mixed, empty, invalid, and unavailable comparisons select the
full matrix. The required job remains named `check`; documentation-only
changes still run exact-diff validation, while browser and ConduitOS jobs may
be skipped. Classification failure remains fail-closed, PR permissions are
unchanged, and `merge_group` uses the same required workflow without disabling
strict up-to-date protection. This is CI routing only: it changes no product,
runtime, proof semantics, or accepted proof class.

The finite presentation-pacing slice from #886 is accepted at exact main
`09da76a69d2e1233291cb216673646ef84aa00a5`; push workflow `31465601893`
passed `check`, `browser-host`, and `conduitos-boot`. Canonical
`conduit.std/time-delay-bool@1` delays every admitted Boolean Current value by
one exact configured duration, retains at most eight pending values in input
order, and drains those values after input closure. Canonical
`conduit.std/time-throttle-bool-leading@1` emits the first eligible value,
drops values arriving during its exact cooldown, retains no hidden latest
value, and cancels its one exact pending timer when input closes.

Both contracts use the existing planned monotonic-millisecond deadline
requirement and ordinary std planning, lowering, and production-kernel path.
The Host supplies only admitted, correlated timer operations; renderer frame
cadence, browser timers, and ambient async timing do not enter portable
meaning. Values, timer slots, queues, and mandatory work are finite before
Play. Deterministic conformance covers zero and maximum durations,
simultaneous values, late wakes, ordered closure drain, leading-edge drops,
timer completion, cancellation, and regressed monotonic time.

An ordinary non-UI Form and a Patchbay-oriented refresh Form exercise the same
installed operations. The generated Host x Kind inventory now includes both
rows and keeps other Host cells explicitly missing or unsupported. This adds
no animation framework, retry/backoff policy, arbitrary timer wheel,
scheduler redesign, hard-real-time claim, browser `setTimeout`, or physical
timing proof.

The typed Patchbay interaction slice from #887 is accepted at exact main
`f909ca84bf3842555516086cf24c4731d2731798`; push workflow `31466734393`
passed `check`, `browser-host`, and `conduitos-boot`. Existing exact
`interaction/select` and `interaction/invoke` meanings remain current for
selection, navigation, and lifecycle. One new `interaction/edit` request
family carries source-document identity, source revision, expanded-Form
basis, canonical subject identities, Kind identity, configuration key, and
typed `ConfigurationValue` as distinct bounded fields.

Native palette placement, duplication/removal, Port-to-Port connection, Cord
rerouting, and Boolean/scalar/choice/text Face controls now normalize to exact
`PatchbayEdit` variants before ordinary Form checking, planning, lowering,
and production-kernel execution. Structured browser input constructs the same
typed request without DOM or widget identity; the read-only browser adapter
truthfully refuses authoring because it lacks edit authority. Renderer-local
Gear movement and Cord waypoints remain presentation state and do not pretend
to be portable semantic edits.

The former delimiter-packed target identity and hexadecimal configuration
codec are absent. There is one current finite interaction encoding and no
legacy decoder or alternate edit revision. The fixed two-node interaction
Plan, four queue slots, one pending host operation, bounded value and Sign
stores, and 32-receipt history remain authoritative. Stale source or
presentation basis, unknown subjects, incompatible Ports, duplicate Cords,
invalid values, pressure, cancellation, refusal, failure, and terminal
completion remain distinct. This adds no raw-pointer stream, universal
gesture recognizer, widget ontology, arbitrary command bus, renderer-owned
edit authority, or semantic layout geometry.

The bounded portable layout-algebra slice from #888 is accepted at exact main
`979ce39e6a61a8ceafd457b5042e7e940135f18e`; push workflow `31469291940`
passed `check`, `browser-host`, and `conduitos-boot`. One allocator-free,
fixed-capacity `presentation/layout-frame@1` Info encoding carries bounded
viewport and child geometry. Six exact Kinds cover viewport, inset, row,
column, stack, and alignment without placing CSS, toolkit, framebuffer, font,
DOM, or device facts in authored meaning.

The reference layout operations and an independently implemented eager
Patchbay presenter normalize materially different internal representations to
the same canonical geometry bytes. A representative Patchbay shell and Gear
Face composition exercises viewport, inset, distribution, stacking, and
alignment. The existing renderer-local Patchbay demo layout remains intact;
it is a presentation consumer, not a second portable layout contract or source
of runtime truth.

An ordinary authored Form runs the six operations through checking, planning,
lowering, and the production kernel before a typed test sink observes the
result. All successful Play-time storage is preallocated. Zero and maximum
extents, undersized frames, child-capacity and arithmetic overflow, clipping,
division remainder, malformed encoding, pressure, cancellation, and terminal
behavior have deterministic coverage. Layout values do not alter source,
checked, expanded, Plan, Play, Port, Gear, or Sign identity. The generated Host
x Kind inventory includes all six exact rows and leaves other Hosts explicitly
unsupported. This adds no constraint solver, graph layout, animation system,
scene graph, font measurement, pixel renderer, or physical/HIL claim.

The bounded presentation-composition and graphics-leaf slices from #889 and
#890 are accepted together at exact main
`fddb2344ecc4e9545d6b3e17cf90dac45a651233`; push workflow `31473617082`
passed `check`, `browser-host`, and `conduitos-boot`, and Pages workflow
`31473617076` completed for the same commit. The ConduitOS job's first attempt
was refused when `curl` returned certificate error 60 while fetching the pinned
Limine 12.5.2 archive; rerunning only that failed job completed the identical
exact-main proof.

Existing `presentation/text` remains the text semantic. Three additional
renderer-neutral semantic Backs cover one canonical icon identity, a bounded
frame, and a bounded status badge. Their allocator-free
`presentation/composition@1` Info carries at most eight ordered obligations,
finite tokens and accessible names, exact roles, and the single canonical
`PresentationIconKey` vocabulary. Missing icon metadata uses one explicit
generic-Gear fallback; partial or unknown metadata refuses instead of guessing.

Below that seam, only `graphics/rect`, `graphics/text`, and `graphics/icon`
passed cross-presenter admission. Every command uses #888 `LayoutRect`
geometry, an exact clip rectangle, paint role, stable integer coordinates,
finite resolved content, and canonical ordering in one fixed-capacity
`presentation/graphics-scene@1` encoding. Clip is a property of each command,
not a stateful Kind. Line and path remain renderer helpers because no current
presentation Back gives them separate portable meaning.

The std implementation transforms the exact values through admitted host
operations and the production kernel. An ordinary seven-Gear Form lowers
icon, frame, and badge through rectangle, resolved text, and resolved icon,
then completes at a typed sink with all successful Play-time storage
preallocated. The constrained Patchbay presenter lowers the same composition
through the canonical graphics leaves, while an independently implemented
native normalizer preserves geometry, clipping class, resolved content,
ordering, paint, and style without requiring pixel parity. A ConduitOS
framebuffer presenter can consume those same obligations before its private
raster writes; framebuffer addresses remain below the contract. Direct
higher-level presenters may still join semantic presentation without exposing
graphics primitives.

Malformed and noncanonical encodings, empty or oversized payloads, zero or
overflow geometry, unknown icons, capacity pressure, cancellation, and forged
execution profiles remain distinct refusals or terminal outcomes. The
generated palette, Host x Kind inventory, Observatory fixture, and ConduitOS
std-gap report include the exact accepted rows. The existing Patchbay demo and
its semantic identities remain intact. This adds no line/path/clip Kind, pixel
API, toolkit contract, scene-graph runtime, font shaping, icon registry clone,
SVG/PostScript language, shader/GPU pipeline, image codec, or physical/HIL
claim.

The first Patchbay-specific presentation-Back slice from #891 is accepted at
exact main `ea259e838c7bad57de32b0cf55601265c283cbbe`; push workflow
`31475550408` passed `check`, `browser-host`, and `conduitos-boot`, and Pages
workflow `31475550397` completed for the same commit.

Exactly three thin meanings cover `patchbay/gear-face`, `patchbay/port`, and
`patchbay/cord`. They consume the existing canonical `PatchbayGear`,
`PatchbayPort`, `PatchbayCord`, `PortDescriptor`, and `FaceControl` values;
they do not create replacement Gear, Port, Cord, or control identities. Port
direction, Info Kind, temporal contract, concise accessible name, and exact
subject identity remain recognizable. Optional Line and Plan facts are
active-lens annotations on a Cord presentation and never replace or equal the
Cord identity.

A direct realization joins each high-level subject without exposing its Back.
Explicit inspection may reveal a distinct finite recursive expansion. The Gear
Face expansion composes accepted layout, icon, text, frame, badge, rectangle,
resolved-text, and resolved-icon leaves into the canonical bounded graphics
scene. Port and Cord expansions reuse the same admitted layout, presentation,
and graphics vocabulary. Normalized direct and recursive realizations preserve
the same subject identity and accessibility obligations while their expansion
descriptions remain visibly distinct. Face controls retain authoritative typed
value intents; no slider, checkbox, knob, textbox, or other widget type enters
portable meaning.

The default Patchbay canvas, existing renderer, interaction path, and
documentary demo remain intact and do not expose recursive machinery unless
asked. The finite `PATCHBAY_BACK_KINDS` inventory records the three accepted
Backs without advertising them as ambient std-host capabilities. This adds no
second Patchbay model, renderer-owned semantic graph, path Kind, widget
ontology, palette or inspector expansion, graph-layout engine, toolkit
contract, self-hosting recursion, or physical/HIL claim.
