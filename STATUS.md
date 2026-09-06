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
| Portable text and chord semantics | `input/keymap` maps exact key events through the finite `conduit-intl` base, Shift, AltGr, Compose, and Unicode-scalar tables into canonical text; `input/chords` maps exact Control/LeftAlt/LeftMeta combinations into structural `input/chord@1` values; `input/key-tee` performs exact typed atomic fan-out | complete reviewed mappings, invalid/malformed/overflow/reset paths, RightAlt/RightMeta reservation, unknown/release suppression, cross-plane type refusal, capacity-one branching pressure, cancellation, and closure are deterministic | the std Host installs all three bounded operations and runs one ordinary production-kernel Form with independent text and explicit chord-control branches; the native Patchbay peer passes the same keymap/chord vectors; `cargo xtask check input-semantics` owns the repository proof entrance | no browser implementation claim | K6 executes the unchanged default-keymap text branch through `text/upper` and serial presentation on ConduitOS; its USB bridge and the native peer emit byte-identical shared key-event vectors | no transport | no physical keyboard or HIL claim |
| Highest-honest-seam keyboard peers | one unchanged platform-neutral keyboard Form and one exact `input/key-event@1` meaning admit materially different truthful sources at their highest honest seam | unidentified or unsupported physical codes, repeats, duplicate presses, unmatched releases, queue pressure, focus loss, cancellation, closure, and ConduitOS USB mechanism failures remain distinct | native Patchbay maps real `winit::PhysicalKey::Code` transitions directly through `patchbay-native/winit-keyboard@1`, retaining exact left/right modifiers, sixteen held keys, eight event slots, one operation slot, and one boot-scoped window-input Base; it reuses the portable keymap/chord tables and can plan the unchanged K6 Form | no browser keyboard implementation claim | ConduitOS continues to select `conduitos/usb-hid-keyboard@1` over exact xHCI/device/interface/endpoint Bases; it shares normalized bytes and semantics with the native peer without either inventing the other's lower machinery | no external transport claim | conformance plus the already-accepted freestanding-emulator USB proof; no native UI automation, physical keyboard, or HIL claim |
| ConduitOS portable keyboard realization | one ready boot-local xHCI/device/interface/endpoint chain yields one exact `conduitos/usb-hid-keyboard@1` offer, ordinary keyboard Plan, production-kernel Play, and portable press/release values with finite device, report, transition, operation, memory, and Cord reservations | absent/unhealthy/ambiguous device truth, stale boot or artifact identity, capacity exhaustion, malformed values, Cord pressure, cancellation, transfer failure, device loss, and closure remain distinct; a real no-device boot emits no keyboard offer | no hosted implementation; `cargo xtask conduitos keyboard-proof` owns the repository proof entrance and retained JSON report | no browser implementation claim | one pinned QEMU USB keyboard produces `[4,0,0]` then `[4,1,0]` through the planned source and native Observatory/Patchbay projection | no external transport claim | freestanding-emulator only; no keymap/text, hotplug, physical keyboard, or HIL claim |
| ConduitOS keyboard-text ordinary Form | one unchanged platform-neutral authored `input/keyboard -> input/keymap -> text/upper -> presentation/text` Form defaults omitted configuration to exact `conduit-intl`, plans four exact implementations and three typed bounded Cords, and executes through the production kernel | absent offer, stale realization, unsupported layout, malformed key, invalid Unicode, Cord pressure and byte underprovision, presentation or source failure, cancellation, device loss, closure, and synthetic-proof substitution remain distinct | no hosted implementation; `cargo xtask conduitos keyboard-proof` owns the typed repository entrance and retains the complete snapshot | no browser adapter claim | one pinned QEMU USB keyboard types ordinary, AltGr, Compose, and Unicode-hex sequences; visible serial fragments prove `HELLO` and `ÆÉΛ`; a standard Observatory snapshot and generic Patchbay rendering preserve the exact Form/Plan/Play, placements, Cords, Signs, boot provenance, and xHCI/device/interface/endpoint Bases | no external transport claim | freestanding-emulator only; no hotplug, peer input, physical keyboard, or HIL claim |
| ConduitOS keyboard detach/reattach | removal changes current boot-scoped realization truth without mutating the authored Form or immutable P1; the same emulated model reattaches at a new attachment epoch as D2 and requires fresh offer generation, P2, and Y | an outstanding D1 interrupt transfer terminates X with machine-readable device loss; P1 revalidation against D2 refuses, old controller completions are drained before slot reuse, and no removal-generated transition enters semantic execution | no hosted implementation; `cargo xtask conduitos hotplug-proof` owns the bounded QMP sequence and retained JSON report | Patchbay consumes the ordinary unchanged semantic topology and exact realization identities; no browser fallback claim | one pinned QEMU process performs actual `device_del`/`device_add`, xHCI port-loss observation, slot retirement, D2 enumeration/HID readiness, and successful fresh production-kernel input under unchanged HostId/BootId/Form identities | no external transport claim | freestanding-emulator only; no general hotplug manager, multi-keyboard policy, physical keyboard, or HIL claim |
| ConduitOS low-level local rescue | one interactive x86_64 profile taps opaque validated physical HID transitions before semantic routing, admits exact Ctrl+Alt+Delete under local-only policy, records one boot-scoped request, and crosses one bounded machine-reset Base without making an ordinary Form or second runtime authoritative | finite matcher, malformed-report, held-key, disabled-policy, unavailable-reset, stale/same-Boot, and physical Ctrl+Delete, Alt+Delete, and Ctrl+Alt+Backspace refusal vectors remain distinct | no hosted implementation; `cargo xtask conduitos rescue-proof` owns same-QEMU request/completion correlation and retained JSON/transcript evidence | no browser rescue claim | separate no-Form and active ordinary K6 Play cases each observe B1, one guest-issued reset request, and fresh B2 with `B2 != B1` in one pinned QEMU process; request acceptance and reboot completion remain distinct | local physical/emulator input only; remote semantic key values cannot construct local authority | freestanding-emulator only; no frozen-machine/NMI, physical keyboard, or HIL claim |
| Exact plan, play, Sign, and presentation identities | S2 planning plus S3/S4 runtime identity acceptance: separate source/checked/expanded/plan types; boot-scoped active-play issuance; host-issued Sign identities; exact active-play/presentation correlation at platform and remote-cord boundaries | semantic/spelling, cycle, mutation/resealed-lie, host-operation admission/bounds, resource reservation/release, authority/link denial, observation-overflow, boot/Play start mutation, unique Sign, wrong-presentation identity, wrong session identity, generated-image mutation, firmware-build mismatch, and runtime-identity mutation vectors | yes, std preparation enforces S2 truth and distributed sources bind exact plan/fragment/play/link/connection identities | browser sinks independently reconstruct and lower exact fragments and reject stale/wrong session facts | generated image and manifest bind source/checked/expanded/plan/fragment identities and clean firmware build identity; runtime-generated boot/play plus presentation/sign identities are carried in USB records and checked across physical sessions | live sessions verify exact base instance, endpoints, limits, host/boot, fragments, plays, connection, and value kind | matching physical receipts retain exact plan/fragment/play/presentation/sign/link/base/boot/build identity |
| Lossless form and composite boundary | S3 plus #398/#399 corrections: exact source, bounded lossless CST, located diagnostics, inline checked forms, recursively bound expansion identity, and checked named input/output faces with exact endpoint, value-kind, direction, and independent-terminal contracts | round-trip/recovery/limits, expansion and face mutation denial, standalone/nested equality, input-only/output-only planning, and topology hiding | parser/checker and planner are general for the checked face contract; current executable leaves lower into `conduit-kernel` | no | no | no separate fixture transport | no |
| Canonical Form execution corpus | canonical face/back source, declarative startup binding, recursive expansion, exact checked-face compatibility including temporal shape, and distinct source/checked/expanded/Plan identities | Programs 1–4 deterministic positive/negative corpus; Program 6 exact two-host planning and link-failure vectors | real std `text/*`, `time/every`, `state/count`, and presentation leaves execute through the existing kernel with bounded Sign and stable capacity | Program 6 browser/WASM sink executes the unchanged canonical Signal source's exact remote fragment | no new firmware claim | actual loopback WebSocket carries the Program 6 Conduit session; it is not an authored external WebSocket operation | no new physical/HIL claim |
| Portable planner capability | optional boot-scoped planner profile/limit offers are part of the `no_std` host advertisement contract; planner identity and scratch state are excluded from plan construction and identity | full and browser-profile equivalence across different planner host/boot identities, bounded pre-planning refusal, missing/ambiguous-offer denial, and a non-planner Pico target with a verified lowerable fragment | the standard reference host advertises `conduit.planner/full@1` and invokes the shared deterministic planner; non-planner hosts remain ordinary complete targets | the actual browser/WASM host advertises `conduit.planner/browser-wasm@1` and plans locally in its WASM start path before lowering and kernel execution | Pico advertises no planner profile; existing generated firmware consumes its externally planned exact fragment, with no general Pico planner claim | no new transport or delegation service | no new physical/HIL claim |
| Competing capability realizations (R2) | equal checked faces admit distinct host implementations/artifacts; general hard requirements, explicit policy, stable realization characteristics, changing resource observations, finite compute ranges/service/topology, exact authority, and immutable selected Plan facts remain separate | three explicitly synthetic text-generation fixtures prove hard context/privacy rejection, deterministic policy choice, bounded candidate-decision Sign, minimum-first compute allocation, observation-driven replacement planning without old-Plan mutation, and non-AI video/storage representability | one selected deterministic fixture executes through ordinary planning, lowering, the fixed kernel, admitted host operation, exact base, and presentation boundaries; normal std preparation validates scalable compute reservations | no new browser adapter or browser-owned realization truth; the required browser job only guards the existing adapter boundary | no firmware execution or bare-metal lane-base claim; `BareMetal` is an architecture-neutral contract demonstrated below physical proof | remote-base and network-storage cases are contract/fixture representations only; no live model API, endpoint, credential, paid service, or new transport claim | no physical CPU enumeration, scheduling-quality, firmware-lane, model, transport, or HIL proof |
| Connection envelope and session wire formats | allocating fixture envelope plus allocation-stable borrowed exact session protocol; framed sessions preserve Base-specific exact identity and bounds | deterministic envelope corpus and session lifecycle/mutation/Base-eligibility vectors | native binary-only RFC 6455 and USB CDC Bases with fixed frame/buffer bounds | real browser WebSocket API with one-message inbox and explicit send bounds | dual-CDC Pico Line keeps session frames on CDC0 and Sign on CDC1 | actual loopback WebSocket plus physical USB CDC; fixture bases remain synthetic conformance only | reciprocal physical Hello/Ready/value/pressure/failure/terminal lifecycles recorded |
| Cord meaning and finite Line realization | `CordId`, `LineId`, Base, binding, endpoint, and session identities are distinct; Plans seal exact bounded Line contracts while current availability remains an external Sign | deterministic one-Line/two-Line planning, immutable availability, unsealed-selection denial, session continuation, and replacement-Plan vectors | std planning/runtime consume only exact ready admitted Line offers and expose selected Line/Base/binding diagnostics | browser/WASM sessions and Patchbay HTML consume the same exact planned Line identities without making transport part of Form or Cord meaning | generated fixed images carry exact Line identity through every selectable Pico composition | bounded WebSocket and USB CDC are distinct Lines to the same R1 Pico boot; no new live transport family is claimed | consumes the already-accepted #361 physical replan and admitted-continuation evidence without claiming a new board run |
| Portable Signal | yes | multi-value fixtures | one std kernel pulse source atomically fans each value to stdout plus two exact remote egresses | browser/WASM kernel show sink with sixteen exact DOM receipts | unchanged Signal forms generate exact local and remote Pico images that drive the CYW43 LED | sixteen ordered values over bounded WebSocket and USB CDC remote cords | sixteen matching ordered stdout, DOM, and physical LED receipts from one exact three-host run |
| R1 Body and Line recovery | exact immutable Plan candidates, distinct Line readiness Signs, replacement-planning events, allocation-stable session checkpoints, and Body/Wake/Lull lifecycle identities | deterministic new-Plan and same-Plan recovery vectors cannot claim physical acceptance | one terminal peer and two browser peers drive exact planned inputs; ordinary planning replaces an unsatisfied WebSocket-only Plan with USB CDC | two pinned Chromium peers each issue exact keydown/on and keyup/off inputs in every physical branch | one continuously USB-powered Pico W boot exposes WebSocket and USB CDC Lines, retains pre-admitted Plan C continuation state, and seals post-Play-start allocation | physical WebSocket loss produces either a new USB Plan/Play or bounded same-Plan/same-Play USB continuation according to the immutable Plan | exact-main `cargo xtask prove r1-hil --interactive` completed both physical faults, eighteen correlated LED Signs, same Body and Pico boot, required Wake continuity, Lull, and later Wake |
| Optional pre-Play HOLD | one Wake may admit an exact immutable Plan plus finite planning-basis Signs, hold reason/source, persistence policy, fixed release-authority contract, and current-validity result while remaining distinct from Playing, active-Play pause, and Lull | deterministic direct, held, authorized release, stale-basis replacement, persistent re-hold, non-persistent replacement, authority denial, bounded-basis, replay-tamper, and lifecycle-separation vectors | `conduit-body` exposes bounded held-Plan admission, inspection, release, invalidation, and replacement APIs; no `ActivePlayId` exists before successful release, and release revalidates the complete current basis before starting Play | no browser UI or adapter claim | no firmware change | no new transport; visibility, reachability, and connectivity confer no release authority | no physical/HIL claim |
| Browser manifestation | local and remote-ingress Signal profiles | shared deterministic contract/planning/lowering/kernel vectors | actual Rust/WASM planner plus exact-plan-lowered `conduit-kernel` execution for local and distributed sink fragments | thin DOM adapter with exact fixed-frame completion correlation and sixteen receipts | no | actual loopback WebSocket to the std kernel source | included in the accepted three-host physical run with matching cross-host receipts |
| Interactive Play start/toggle | typed `interaction/start -> state/toggle -> presentation/show` contract with admitted input and exact remote planning | deterministic Play start/toggle lifecycle and identity negatives | native std source services one admitted stdin Play start through the kernel before realizing the corresponding remote offer | pinned Chromium proves the first Enter causes exactly one sequence-0 DOM update before later inputs, then completes sixteen exact presentations with one real pressure retry | no | actual bounded loopback WebSocket with structured link-break failure | no |
| Native Patchbay presentation and interaction | checked/expanded Forms project exact Gear/Port/Cord subjects; finite platform-neutral `interaction/select` and `interaction/invoke` requests cross exact typed Ports and one admitted host-operation boundary through the production kernel | deterministic geometry/hit, pointer/keyboard convergence, Plan/Play/Sign inspection, bounded-value, stale/unknown/oversized identity, request-restoration, and distinct success/refusal/failure vectors | the actual native Patchbay window renders the bounded canvas and routes pointer selection, graphical keyboard traversal, open/save/view actions, and Body lifecycle controls through ordinary interaction Plays before shared inspector or control state changes | HTML consumes the same semantic request types without DOM identity; no interactive HTML realization is claimed | no firmware or framebuffer interaction claim | no new transport | no physical/HIL claim |
| Explicit external WebSocket chat | opt-in `net/websocket` client/listener faces encode complete-message RFC 6455 semantics structurally; the authored client describes semantic Presentation regions, collection/items, text entry, status, and action meaning without host or DOM facts | deterministic checked-face, canonical expansion, exact finite interaction offers, bounded planning/kernel execution, stale Presentation/Manifestation, unknown input/action, wrong target/kind, empty/oversize/malformed value, duplicate sequence, pressure, cancellation, platform failure, missing-offer, and disconnect vectors | bounded two-peer std listener executes exact accept/receive/send operations through the ordinary fixed scheduler and host-operation boundary | two independent planned browser/WASM kernels use native browser WebSocket plus a generic semantic Presentation renderer and exact typed interaction source; pinned Chromium proves click/Enter convergence, authored-label changes without JavaScript changes, A/B exchange, content-minimizing evidence, and truthful one-peer continuation | no | actual binary loopback external WebSocket messages, mechanically distinct from Conduit-session Lines using exact `conduit.base/websocket-rfc6455@1` realization identity | no physical/HIL claim |
| Bounded shared pools and explicit dynamic flow | checked Forms carry exact scoped pool references and hard member bounds; Plans seal equal-face member contracts, host/boot/capability/resource envelopes, per-member queue/Sign limits, admission authority, and explicit consumers | allocator-free keyed membership, stale occupation epochs, deterministic membership snapshots, per-branch fan pressure/outcomes, and source-tagged bounded merge vectors | the existing kernel owns fixed pool/fan/merge state; the std proof host plans and lowers one 32-peer chat pool without adding a scheduler or ambient registry | two Chromium pages dynamically join, exchange addressed broadcasts, one leaves, and the remaining peer continues; the authored Form names only pool, room, fan, merge, and peer semantics | no | the proof host selects a bounded binary loopback WebSocket Line below authored semantics; no socket/address/Base fact enters source identity | no physical/HIL claim |
| Pico-shaped manifestation | exact Pico-local and remote-ingress advertisements with reviewed fixed-image bounds | shared deterministic contract/planning/lowering/image-generation/kernel vectors | host-side unchanged-form planning/lowering/image generation, exact std source, and verifier tests | no | RP2040 images generated from exact local/remote fragments, CYW43 GPIO 0 LED driver, pinned radio assets, clean firmware-build identity, runtime boot/play receipt identity, and bounded dual CDC | exact bounded std↔Pico USB CDC and final three-host sessions | recorded local, exact std↔Pico success/failure, and final three-host success/broken-link runs |
| Retired membership prototype | historical only | deterministic table tests | no production Body model | no | no | no | no |
| Observatory | versioned neutral host/capability/Base/link/plan/Play/pressure/current-and-historical-Sign/retention reports with exact identity and bound validation; sealed boot provenance remains distinct from live offers and Bases | synthetic fleet retained only as an explicitly labeled integration test | actual std execution can write a bounded report artifact; the read-only `observatory-report` command validates and renders complete structured tables without runtime control; native Patchbay validates and linearly renders the same ordinary snapshot exported by ConduitOS | no browser UI or browser-owned runtime truth | no firmware-side inspector or report store; the accepted ConduitOS export is freestanding-emulator proof | no new transport; observed links are report facts only | no new physical/HIL claim |
| Durable system continuity | allocator-free realization record over explicit membership, complete checked-face role requirements, exact host+boot assignments, observed links, boot-scoped authority, Plan, Play, and Sign identities | accepted std/browser/Pico replacement vector consumes a validated current-model snapshot, separates request acceptance/old-boot terminal/new-boot observation, and requires explicit replanning with new Plan/Plays and no stale grant inheritance | no execution engine; the layer consumes current reports and exact plans without owning scheduling, placement, bases, or authority issuance | no new browser adapter or UI claim | no firmware change; the accepted Pico arrangement is consumed as already-proven input | no new transport; link observation remains distinct from membership and authority | no new physical/HIL run or claim |
| One durable multi-Host Body | one bounded canonical Body keeps explicit Part membership separate from renewable current Host/Boot presence, offers, Lines, authority, immutable Plan selection, Play, and Signs; Birth explicitly admits Here, while ambient discovery remains inert and Body-directed invitations are finite, single-use, and authenticated | deterministic hostile, malformed, replay, bounds, disconnect, stale-Boot, presence-expiry/session-loss, return-atomicity, offline, active-Plan join, and explicit-replan vectors preserve exact state distinctions; refusal paths fail closed without Form, durable membership, authority, immutable Plan, or execution mutation; exact selected-Host loss changes current availability and typed Play/Wake satisfaction while leaving Form, durable membership, authority, and immutable Plan unchanged | native Patchbay derives the human-first Parts view from canonical presence plus membership, owns finite admitted browser sessions, returns one same-incarnation browser only after exact signed continuity and atomic presence preflight, projects loss/expiry offline, and executes the accepted R1 Plan through the existing planner, lowering, and production kernel | three independent Chromium Hosts prove ambient explicit admission, Body-directed admission, distinct Host/Boot identities, bounded renewal, graceful close, half-open expiry, one same-incarnation return with a fresh session/newer sequence, offline retention, immutable active-Plan join, and explicit replacement planning; browser scheduling is explicitly best-effort with no background-realtime or reload-persistence claim | the already-provisioned R1 Pico advertises and is explicitly admitted without this slice implementing provisioning or flashing | truthful WebSocket and USB CDC Lines carry the accepted Plans; browser rendezvous presence remains distinct from Ready Line truth, and real Wi-Fi loss changes Line availability rather than membership or Form meaning | exact-main physical proof retains one Body with local std, two attached browser Parts, and one Pico Part across WebSocket Plan A, explicit USB Plan B recovery, and same-Plan/same-Play Plan C USB continuation with verified physical LED receipts; browser-presence acceptance at `58a5a86e` adds deterministic, live-loopback, and pinned-Chromium evidence but no new physical/HIL claim |
| Body-wide multi-Form scheduling and birth | Program is Form; one canonical finite workset holds zero, one, or many exact checked Forms without replacing Body identity; birth establishes revision zero with that same workset, no privileged current `SeedId`, one Wake, one immutable complete Plan, and at most one active Play | deterministic zero/one/many birth, canonical ordering, duplicate/count/identity-byte bounds, explicit v2 Body identity derivation, decode-only historical Seed schema, add/remove/empty continuity, complete-workset sealing, stale replacement, one-active-Play, and atomic global resource-overcommit refusal vectors fail closed | multiple initial Forms progress through one production `conduit-kernel` Body Play; workload change replaces Plan/Play under the same Body/Wake rules, and the same Plan model contains local and distributed Form partitions | Crèche composes a bounded multi-select initial workset from the shared reviewed inventory; browser Patchbay projects Active Forms and exact biography history; Tour, Crèche, Patchbay, and conformance consume the canonical `forms/morse-network/`, `forms/memory-lantern/`, and `forms/desk-telegraph/` sources | ConduitOS presents current Form/workload vocabulary and proves the same Host/kernel substrate with one cooperative execution lane; no SMP, preemption, or physical parallelism claim | distributed Plan partitions preserve exact Host/Line facts without creating Host-local schedulers or false global time | merged as exact main `230ca04967588e91a78018503324d2e721b54099` through #2290; definitive PR workflows `33809913908` and `33809914063` passed the complete workspace, firmware, emulator, and 92-scenario pinned-Chromium product matrices; no new physical/HIL, SMP, or preemption claim |
| Composable human-facing browser Hosts | portable camera, microphone, typed human interaction, text Presentation, graphics-scene Presentation, and Line contracts remain separate finite meanings; permission-gated acquisition seals request authority and semantic constraints before browser interaction, while exact opaque resource truth and use authority become inputs only to a later immutable Plan | deterministic offer/state, authority, permission/refusal, unsupported-constraint, capacity, pressure, cancellation, closure, loss, stale-identity, pre-resource unrealizability, exact-resource selection, and unchanged-Form replan vectors fail closed | `cargo xtask host browser` binds one independent ephemeral loopback entrance per invocation and launches a fresh page/WASM Host; a native bounded Body coordinator explicitly admits two browser Parts, seals the post-acquisition camera Plan, and invalidates its exact binding on Host loss without a second planner or scheduler | pinned Chromium proves distinct Host/Boot/WASM identity and reload replacement, real browser camera and microphone API mechanics, denial/dismissal/constraint refusals, one exact acquired camera resource selected by a later Plan, one bounded camera value over the selected WebRTC Cord, pressure refusal, and source-Host loss; fake media devices establish browser mechanics, not physical input | no firmware claim | one finite planned WebRTC DataChannel Line carries only the selected typed camera Cord; page reachability, membership, permission, acquisition, authority, resource truth, and Line readiness remain distinct | exact-main `7e351676b955e8b18e297a86604f2b609d52ec7b`, required workflow `32685791138`, and visual-evidence workflow `32685791180`; no physical camera/microphone observation, browser-process identity, persistence continuity, background-realtime, or HIL claim |
| PROFILE-built Host IMAGEs and cross-Host Manifestations | versioned bounded PROFILEs resolve through a finite prerequisite graph into deterministic BUILD manifests and exact IMAGE/artifact identities; runtime offers require both exact IMAGE inclusion and current Boot facts; FACE, Presentation, Presenter, Manifestation, and admitted placement remain distinct exact identities | deterministic BUILD, budget, dependency, artifact-binding, stale Boot/generation, missing-live-prerequisite, headless-placement, cross-wiring, Manifestation bound, and immutable-Plan vectors fail closed; `cargo xtask host capstone` retains a bounded exact-identity receipt for three materially different checked PROFILEs joined to one Body | one checked native ConduitOS PROFILE includes the optional finite compositor/scanout stack, while one checked headless ConduitOS PROFILE omits all graphical machinery; the capstone uses the shared fabrication, Body, planner, Presentation, and Manifestation contracts without adding an execution kernel | the checked browser-page PROFILE BUILDs an IMAGE whose DOM/SVG Presenter is offered only with the exact manifest and live DOM/surface facts; one semantic action through the native Manifestation becomes Body truth and both native and browser independently consume the revised Presentation, with no renderer mirroring | no new firmware execution claim; the smaller checked ConduitOS Pico-W PROFILE proves intentional headless IMAGE composition and truthful graphical omission | no new transport claim; reachability remains separate from membership, authority, and Presenter placement | exact-main deterministic composition/conformance proof at `56f6ddcbeac997aecfb089851c5a62f65eceeea6`; no new live-browser, emulator, firmware, physical/HIL, or pixel-parity claim |
| Host fabrication packages | one finite deterministic package set contributes exact target descriptors, Base implementation offers, package-owned toolchain and maximum bounds, artifact kinds, and target-appropriate post-build actions; anchor and extension packages remain distinct, and PROFILE/BUILD/IMAGE/spore identities seal exact package and implementation provenance | duplicate packages, targets, offers, missing anchors, mismatched extensions, unavailable toolchains, unsupported targets, excessive bounds, and caller attempts to substitute package-owned facts refuse before fabrication; an external RP2040 PIO-audio extension passes the ordinary resolution and build path without editing generic fabrication | native/std owns BUILD → native bundle → LAUNCH through its family package | browser owns BUILD → browser bundle → LOAD/LAUNCH through its family package; the existing pinned Chromium execution remains a separate proof class | ConduitOS owns distinct x86_64, IA-32, AArch64, RISC-V64, and LoongArch64 IMAGE/BOOT targets; Pico owns UF2/FLASH/BOOT; Raspberry Pi owns exact B+ and Zero descriptors plus SD-image/FLASH/BOOT mechanics; ESP32 owns its existing exact firmware family and IMAGE/FLASH/BOOT mechanics | no new transport claim; deployment actions do not create membership, reachability, trust, or authority | exact-main `89d72639d40c238aaaf2bbe7da23f764c29c5402`, workflow `32924122237`; real firmware and emulator jobs remain the proof classes they actually ran, with no new physical/HIL claim |
| PREWAKE robotics semantics | seven exact portable Kinds cover bump, body-frame orientation, sensor-forward range, start-local odometry, battery, body velocity intent, and differential-drive projection; each Port retains a distinct bounded Info identity where shape, unit, frame, or validity differs | exact-main `607f602da25d23f9b74535e2272c6bd151f3604d`, workflow `31460348259`: deterministic codecs and an ordinary checked Form/Plan/lowered production-kernel Play prove clear-space projection plus independent pressed-bumper and insufficient-range suppression; invalid, missing, stale, pressure, cancellation, and unavailable-implementation outcomes remain distinct | the optional std robotics family advertises preallocated PREWAKE-only sources and a simulated differential-drive projection with no host operations, resources, authority requirements, live device Signs, or physical-effect completion; Pete bump/IMU describe-only offers reuse the same exact portable faces | no browser execution or manifestation claim | contracts compile for Thumb; no firmware implementation or execution claim | no transport | no physical actuator, device, HIL, or safety-certification claim |
| Pete deterministic observation-to-motion capstone | one byte-identical canonical Form observes portable bump meaning, selects bounded body velocity, and drives the exact differential-drive Port without Host, Create, UART, GPIO, transport, or safety-profile facts | exact-main `83f5845f82d6af818b62c8db93a113beeb6a869d`, workflow `32411739885`: the same expanded identity seals distinct std and Pico Plans; clear/contact select bounded motion/zero through the production kernel, while TTL expiry, authority loss, observation/drive provider loss, stale evidence, wrong Plan identity, and pressure remain distinct | deterministic std and Pico evidence retains different Host, Boot, Base, provider, implementation, safety, auxiliary-resource, and serialized-Create-lane truth; normalized receipts explicitly use proof class `deterministic-production-kernel-shape` and set `physical_motion_claimed = false` | no browser claim | the constrained Pico composition compiles for Thumb, but this row claims no firmware execution | Create UART remains a local admitted Base/resource operation lane, not a Conduit LINE; no live transport claim | no std physical motion, Pico HIL, Conduit LINE-loss physical proof, or N8 physical completion claim |
| `conduit.std` | one mechanically derived supported nucleus currently contains 58 exact typed contracts and matching canonical offers across timing, text, state/flow, logic/math, input, sound, layout, presentation/graphics, Patchbay, robotics, JSON, and protected file/copy meanings; no erased pre-v1 catalog is compiled or discoverable | UI-independent contract/codec/limit/mutation vectors, canonical Programs 1–4, and deterministic family-specific pressure, closure, cancellation, boundary, overflow, refusal, and mutation vectors | the std reference Host advertises the same canonical offer set and resolves exact installed implementations before bounded execution through `conduit-kernel`; minimal/subset compositions still advertise only selected offers | browser-supported presentation meanings consume contracts from the same nucleus; this row makes no blanket browser-manifestation claim for the full catalog | no blanket firmware-execution claim for the full nucleus; the ConduitOS gap row records exact current profile availability separately from each implementation family's accepted proof class | no new transport; Program 6 uses the separately owned Signal family | no new physical/HIL claim |
| ConduitOS portable std gap | bounded inventory derives every supported-nucleus contract/offer, revision, face, limit, and canonical SHA-256 content identity directly from catalog truth; exact count and digest changes are mechanically visible, and retired rows cannot enter | deterministic inventory mutation, profile separation, recursive-coverage, prerequisite-classification, and completeness vectors cover the current 58-entry frontier | `cargo xtask conduitos std-gap` compares exact kind/revision identity with the authoritative ConduitOS profile and currently reports 57 implemented, directly or through reviewed recursive coverage, plus one unavailable `file/copy` classified `missing-base` with unsatisfied `base:storage`; it advertises no missing capability | no new browser claim | the maximal reviewed ConduitOS profile truthfully exposes the 57 covered meanings while individual emulator/execution claims remain owned by their accepted proof slices; no storage Base, protected file resources, filesystem authority, or `file/copy` offer is fabricated | no transport | no new physical/HIL claim |
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
- one unchanged platform-neutral ConduitOS keyboard-text Form whose exact
  default `conduit-intl` keymap, uppercasing, serial presentation, four
  placements, three typed bounded Cords, production-kernel Play, real QEMU
  ordinary/AltGr/Compose/Unicode-hex sequences, visible `HELLO` and `ÆÉΛ`,
  distinct refusal outcomes, and standard Observatory snapshot consumed by
  generic Patchbay rendering are checked and retained by that same typed
  `cargo xtask conduitos keyboard-proof` entrance;
- one real bounded ConduitOS keyboard detach/reattach sequence whose QMP
  device removal is observed through xHCI/HID as D1 loss, fails the outstanding
  production-kernel source operation without semantic fabrication, retires the
  old slot and completion generation, preserves immutable P1 and unchanged
  semantic identities, enumerates the same model as distinct D2, refuses P1
  against refreshed Host truth, and executes fresh P2/Y through `cargo xtask
  conduitos hotplug-proof` with retained machine-readable evidence;
- one documented highest-honest-seam rule and real native Patchbay keyboard
  peer that maps physical codes directly to the portable key-event vocabulary,
  advertises an exact finite window-input realization without USB/HID fiction,
  matches the ConduitOS USB bridge byte-for-byte, reuses the exact Unicode and
  chord semantics, plans the unchanged K6 Form, and keeps platform mapping,
  pressure, focus, cancellation, and closure failures distinct under
  `cargo xtask check input-semantics`;
- one bounded low-level ConduitOS local rescue path that consumes only opaque
  validated physical HID transitions before ordinary keyboard planning,
  recognizes exact Ctrl+Alt+Delete once, records B1 and exact local authority,
  issues one guest reset, and is correlated by `cargo xtask conduitos
  rescue-proof` to a distinct B2 in the same QEMU process; retained proof and
  transcript evidence also establish physical near-miss refusal and preserve
  request acceptance separately from reboot completion; a second independent
  case proves the same path while the ordinary K6 keyboard-text Play is active;
- one bounded deterministic ConduitOS portable-std inventory/gap report derived
  from current supported-nucleus contract and offer truth, with a semantic
  content digest, exact Host build/profile basis, and complete implemented or
  missing classification without capability promotion;
- no-std checks for the salvage kernel, semantic, wire, and semantic-catalog contracts;
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
  outcomes, exact Pete describe-face reuse, palette metadata, and no-std/
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
- one pinned Chromium composable-browser-Host proof with independently launched
  ephemeral entrances and fresh page/WASM Host/Boot identity, split finite
  human/media/presentation/Line offers, explicit Body admission, bounded
  permission-gated acquisition, exact opaque resource and use-authority truth,
  a subsequent immutable Plan selecting one typed camera Cord over one finite
  WebRTC Line, one observed bounded value, and distinct refusal, pressure,
  replacement, and Host-loss outcomes without a physical-media claim;
- direct unchanged `proof/fixtures/forms/signal-demo.conduit` to Pico-local plan, selected
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
structural face equality under #522 and remains distinct from a Conduit-session
Line using exact `conduit.base/websocket-rfc6455@1` realization identity. Two actual
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
The Form source boundary retains a bounded lossless canonical source document:
exact source and CST tokens survive invalid edits,
diagnostics carry stable codes and UTF-8 byte/line/column spans, and no checked
form is manufactured after an error. The composite checkpoint derives one exact contract
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
instances parse and plan unchanged `proof/fixtures/forms/signal-demo.conduit`, lower their
exact local fragments through the shared contract, and execute through
`conduit-kernel`. JavaScript remains the real-timer/DOM adapter. Fixed frames,
exact completion correlation, item/byte limits, duplicate/malformed/wrong
identity denial, cancellation, Sign exhaustion, terminal failure, and
stable sealed capacity are executable proofs. WASM allocation is not claimed
to be measured; the accepted claim is precise capacity stability.

The S4 live std-to-browser Signal checkpoint is accepted at exact main
`a1f479dfa58b8537427b5747da73795628504913`; workflow `31031406945` passed
both the full Rust gate and the pinned Chromium job. The unchanged
`proof/fixtures/forms/signal-demo.conduit` lowers into exact std-source and browser-sink
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
Its firmware build parses `proof/fixtures/forms/signal-demo.conduit`, plans both Gears
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
DOM, and physical Pico LED receipts from unchanged `proof/fixtures/forms/triple-signal.conduit`
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
`forms/pool-webchat/main.conduit` contains no WebSocket, socket, network, address,
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
with one exact capacity-four Cord, lowered through `conduit-plan-lowering`, and runs
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

The portable keymap and chord implementation from #810 and #814 is accepted at
exact main `d9c28dccf076dc63715de6daca14965d3f55ae31`; push workflow
`31481729557` passed `check`, `browser-host`, and `conduitos-boot`. The separate
Pages workflow `31481729559` also completed for that exact commit. The bounded
repository entrance is `cargo xtask --locked check input-semantics`.

`input/keymap` defaults omitted configuration to the sealed `conduit-intl`
layout. Its allocator-free state and checked-in tables cover familiar base and
Shift QWERTY, a finite RightAlt/AltGr glyph layer, RightMeta Compose with a
portable fallback, and one-to-six-digit hexadecimal Unicode scalar entry.
Invalid, empty, surrogate, over-limit, malformed, cancelled, and incomplete
sequences reset finitely without retaining an editor buffer. Control, LeftAlt,
LeftMeta, releases, modifier-only transitions, and non-text keys produce no
text, and no locale, toolkit, USB, DOM, device, or operating-system fact enters
the mapping.

`input/chord@1` is one exact four-byte structural value. The finite
`conduit-core` table recognizes only the documented Control, LeftAlt, and
LeftMeta combinations; unknown combinations and releases emit nothing, while
RightAlt and RightMeta remain reserved to text semantics. Chord identities are
semantic hints and execute no ambient desktop, C0, POSIX, process, or product
action. `input/key-tee` admits each exact key event atomically to its text and
chord branches, preserving typed port identity instead of restoring implicit
broadcast.

The std Host installs all three Kinds as ordinary bounded operations with one
in-flight operation per semantic Gear, finite queues and action storage, exact
cancellation, and terminal closure. One checked and planned capacity-one Form
runs through the production kernel with independent canonical-text and explicit
chord-control sinks; another routes keymap output directly into `text/upper`.
Patchbay derives the finite `layout = conduit-intl` and `map = conduit-core`
choices from authoritative Kind metadata. The reusable core compiles for
`thumbv6m-none-eabi`, but second-Host execution remains owned by #812, so #814's
strict multiple-Host criterion remains open.

This slice adds no ConduitOS/browser/native adapter, host layout oracle, IME,
Unicode-name database, arbitrary keymap or macro language, line editor,
auto-repeat policy, process signal, low-level rescue authority, physical
keyboard, or HIL claim.

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
operation, request identity, and whether the ordinary keyboard Plan is active.
The finite HID session may observe at most 48 successive reports and retain at
most 48 transitions while reusing the same two admitted report buffers and
fixed transfer ring.

The x86_64 reset Base performs a finite controller-readiness check and emits
exactly one guest reset command. Controller busy and a reset command that
returns are explicit failures; the old boot cannot report completion. The
runner observes B1, the request, disappearance/reset, and a fresh B2 with
`B2 != B1` while the original QEMU process remains alive, then terminates that
process only as post-proof cleanup. Ctrl+Delete, Alt+Delete, and
Ctrl+Alt+Backspace physical injections remain in B1 without a rescue receipt;
malformed HID, held-key repeat, disabled policy, unavailable reset Base, and
stale/same-Boot correlation are deterministic negative cases.

The active-Play completion from #816 is established at exact main
`e5ee69ed0d881a87c9364ed0dcace73e7e2db2ee`. PR-head workflow
`31486608745` and exact-main push workflow `31487067298` passed `classify`,
`check`, `browser-host`, and `conduitos-boot`. The retained proof starts the
ordinary K6 keyboard-text Play with a real USB transition, observes the same
validated physical seam, records `ordinary_keyboard_plan: true`, issues one
guest reset, and correlates a distinct fresh Boot in the same QEMU process.

This is low-level local rescue while the CPU, xHCI/USB/HID service, and reset
Base remain responsive, not a hardware NMI or completely-frozen-machine claim.
No graceful Lull is claimed: reset makes the old Boot and its Plan/Play stale.
Together with the accepted no-Form path and the state-independent finite
matcher below semantic execution, this completes the K9 acceptance boundary.

The ConduitOS keyboard-text K6 slice from #812 is accepted through two exact
main implementation commits. The initial portable-keyboard realization landed
as `bfb0082aadd14cf4702e78599eaef04e8604b12e`; PR-head workflow `31485310057`
passed the required matrix. The completed ordinary keyboard-text Form landed
as `47a0b35af9c9a1c8a9f9cc03dfc46833156cadb9`; PR-head workflow
`31488171186` and exact-main push workflow `31488606328` passed `classify`,
`check`, `browser-host`, and `conduitos-boot`.

The authored Form remains unchanged and platform-neutral:
`input/keyboard -> input/keymap -> text/upper -> presentation/text`. It contains
no machine, USB, layout, serial, or implementation fact. Omitted keymap
configuration resolves to exact `conduit-intl`. The Plan selects the boot-local
USB HID source, finite keymap and uppercase implementations, and admitted
serial presentation, joined by three exact typed Cords with one-item and
three-byte capacity. The production kernel owns Play; its finite proof budget
admits at most 48 HID reports and transitions and uses fixed operation, queue,
value, Sign, and presentation stores.

One pinned QEMU run injects 38 acknowledged physical-key transitions covering
ordinary `hello`, AltGr+A, Compose-apostrophe-E, and Unicode-hex `03bb`. The
ordinary Form visibly presents `HELLO` and `ÆÉΛ` fragment by fragment; there is
no hidden word accumulator or keyboard-specific presentation path. The retained
standard `conduit.observatory.snapshot/v2` preserves exact source, checked and
expanded Forms, Plan, Play, four placements, three Cords, completed lifecycle,
finite Signs, sealed Boot provenance, and xHCI/device/interface/endpoint Bases.
Generic Patchbay topology parsing and rendering must expose those semantic
Kinds, the USB HID implementation, key-event Cord, and xHCI Base; it does not
become another source of runtime truth.

Deterministic refusals keep device absence, non-boot or ambiguous devices,
stale Boot and artifact identity, resource exhaustion, malformed key values,
invalid Unicode reset, Cord pressure and byte underprovision, unavailable
presentation, USB source failure, source failure versus cancellation, late
output after cancellation, device loss versus closure, omitted default versus
explicit unsupported locale, and synthetic/emulator proof substitution
machine-readable and distinct. This is freestanding-emulator proof only.
Hotplug remains owned by #813, and no physical keyboard or HIL proof is
claimed.

The portable modifier-chord K5b slice from #814 and highest-honest-seam K8
slice from #815 are accepted at exact main
`cca1aa480546e901a689810a69ec2c143d91025b`. PR-head workflow `31490336051`
and exact-main push workflow `31490835414` passed `classify`, `check`,
`browser-host`, and `conduitos-boot`. The prerequisite portable input semantics
landed at exact main `d9c28dccf076dc63715de6daca14965d3f55ae31` after
PR-head workflow `31481385019`; exact-main workflow `31481729557` passed the
same full matrix.

`input/chord@1` remains a four-byte structural semantic value and
`input/chords` remains one normal bounded Gear. Its finite `conduit-core` table
maps reviewed Control, LeftAlt, and LeftMeta combinations to semantic hints
without executing actions, synthesizing C0 bytes or POSIX signals, or stealing
RightAlt/RightMeta from Unicode entry. The std Host ordinary production-kernel
Form uses exact atomic key-event fan-out so normal text continues independently
while Ctrl+G reaches an explicit typed control sink under capacity-one pressure.
The native peer passes the same Ctrl+G, LeftAlt+P, and LeftMeta+P vectors and
keeps RightAlt/RightMeta reserved for the exact shared keymap.

The documented highest-honest-seam rule now has two materially different real
entrances. ConduitOS converts validated HID transitions above exact
xHCI/device/interface/endpoint Bases. Native Patchbay consumes real
`winit::PhysicalKey::Code` transitions directly and advertises
`patchbay-native/winit-keyboard@1` over one boot-scoped window-input Base,
eight event slots (24 bytes), sixteen held-key slots, one input slot, and one
operation slot. It does not inspect localized logical text, XKB/OS layout,
timestamps, window identity, HID reports, or USB facts to construct portable
values.

Both entrances pass the same byte-exact raw vectors for press/release,
left/right Shift, shifted A, simultaneous A/B, Alt, and Meta identity. The
native values then exercise the exact shared `conduit-intl` state machine for
plain and shifted letters, AltGr `æ`, Compose `é`, and Unicode-hex `λ`, plus
the exact shared chord mapper. The unchanged K6 authored source checks and
plans with the native source while retaining its semantic identity; the native
Plan selects distinct Host, Boot, implementation, artifact, window-input Base,
and finite resources and contains no xHCI, USB, or HID claim.

Native unidentified/unsupported keys, platform repeats, duplicate presses,
unmatched releases, queue pressure, focus loss, cancellation, and closure stay
distinct. ConduitOS device/endpoint failures retain their already-accepted
mechanism-specific diagnostics rather than being normalized away. This adds no
browser keyboard, Bluetooth, PS/2, remote-control authority, global shortcut
daemon, alternate keymap/chord contract, physical keyboard, or HIL claim.
Hotplug and fresh device realization remain owned by #813.

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

The canonical direct-versus-recursive Patchbay presenter capstone from #892,
which also completes #891's exact-Plan requirement, is accepted at exact main
`40e4ca6be2933ce127233b7a2fce485eea60137f`; push workflow `31480668293`
passed `check`, `browser-host`, and `conduitos-boot` for that merged commit.

One unchanged bounded user Form carries `presentation/patchbay` meaning. A
high-capability browser profile plans that Kind to one exact direct presenter;
the constrained profile selects four exact reusable canonical Forms and
expands them into ordinary layout, presentation, and graphics placements.
Selected Back source and checked identities are sealed into the expanded Form,
Plan, and every Plan fragment. Tampering with either Plan-level or
fragment-level provenance invalidates verification. Both realizations use
ordinary planning, lowering, and the production kernel with finite admitted
operations and Signs; there is no fixture-local Plan or recursive executor.

The realizations preserve the same normalized Patchbay subjects,
relationships, labels, accessibility text, semantic actions, and lens
obligations while retaining distinct expanded Form and Plan identities. The
same portable selection request succeeds through both shapes. Direct presenter
absence and recursive leaf absence remain distinct failures without mutating
the user's source or checked Form. The authoritative catalog matrix derives
`DIRECT` browser coverage and `RECURSIVE` constrained coverage, including exact
implementation and Back provenance. This is software execution evidence, not
pixel parity, a universal framebuffer claim, or physical/HIL proof.

The ConduitOS keyboard detach/reattach slice from #813 is accepted at exact
main `d5042ff1107f89e286487b3d2a41ec2ea9cc7805`; push workflow
`31493754053` passed `check`, `browser-host`, and `conduitos-boot`. Its
implementation PR #940 first passed the same jobs at exact head
`c45bf900d205814836874409e1102530a567432e` in workflow `31493245250`.

The typed `cargo xtask conduitos hotplug-proof` entrance boots the already
accepted K6 Form against D1, leaves one exact production-kernel keyboard host
operation outstanding, and removes the QEMU keyboard through actual QMP
`device_del`. The guest observes xHCI port loss as `HidError::DeviceRemoved`,
fails X rather than closing or succeeding it, preserves P1 byte-for-byte, emits
no removal transition into semantic execution, disables the old xHCI slot, and
boundedly drains old transfer completions before reuse.

QMP then attaches the identical model as attachment epoch 2. The guest performs
the ordinary bounded USB enumeration and HID readiness path, derives D2 with
`D2 != D1` under the unchanged HostId and BootId, publishes offer generation 2,
and proves P1 cannot validate against that current truth. The unchanged source,
checked, and expanded Form identities produce fresh P2/Y; real D2 press/release
input then completes through the same typed keyboard/keymap/upper/presentation
production-kernel topology. The proof profile admits 64 HID report/transition
slots and a 2 MiB boot arena before Play; the normal profile remains unchanged.
This adds no general hotplug manager, same-Plan substitution, multi-keyboard
policy, Wake controller, authority source, browser fallback, physical keyboard,
or HIL claim.

The native Patchbay usability epic from #982 is accepted through its final U8
and U9 slices at exact main `e38ddb6ab7163aa8daeb2d03cad58be99b562c35`;
push workflow `31614731317` passed the required `check` and `conduitos-boot`
gates after PR #1065 passed its exact-head matrix in workflow `31614333260`.
The supported `cargo xtask demo patchbay --first-run-proof` entrance opens the
bounded `default-welcome` workspace, traverses `hello : greet` with exact Face
Ports, edits canonical text through a visible bounded Face control, finds and
adds `text/upper`, recovers from one nonfatal invalid connection, wires exact
compatible Ports, traverses breadcrumbs, and performs coherent semantic
undo/redo before visible Birth, Wake, Plan, and Play actions complete through
the production kernel.

The finite JSON result records one worker, zero retries, one wrong turn,
refusal recovery, 24 interactions within the pre-stated 24-interaction bound,
and elapsed time within 30 seconds. Acceptance observes actionable native
renderer state rather than screenshot pixels, retains the actual bounded
`HOWDY` presentation, and requires ordinary Play Signs and kernel Signs through
their authoritative documents. Semantic history remains finite and does not
rewind lifecycle, filesystem, Host, evidence, navigation, selection, or
viewport state. This is hosted native software execution evidence; it adds no
browser, firmware, physical, or HIL claim and no second runtime or semantic
authority.

The Body-membership capstone from #1003 merged through PR #1092 as exact main
`018167e257508091c5e29461ccc1552ae92e526e`. The PR passed its complete
exact-head matrix in workflow `31635365460` on attempt 2 after one retained
Limine-download failure, and the merge commit passed Pages workflow
`31635911360`. Its push `check` `31635911335` retained Limine-download
failures and was later cancelled rather than promoted to acceptance. Current
exact main `e98a8114ea668bd51c3972cc9c82736c17a9669e`, which contains #1092 plus
two unrelated ConduitOS/CI changes, passed `check` and `conduitos-boot` in
workflow `31637201154` and Pages workflow `31637200988`.

The supported `cargo xtask prove body-membership-hil --locked --interactive`
entrance first runs the finite deterministic admission, replay, malformed,
pressure, disconnect, authority, active-Plan-join, offline, fresh-Boot, and
stale-Plan refusal suite. It then admits one local std Here Part, three
independent Chromium Parts, and an already-provisioned physical `r1/pico-w`
Part into one canonical Body. One browser becomes Offline without losing its
Part; admitting another browser leaves Plan A byte-for-byte unchanged; and an
explicit replacement Plan is distinct. The retained physical membership link
requires exactly one local std Part, at least two attached browser Parts, one
Pico Part, and matching Body, Pico Boot, and active Plan identities before the
R1 HIL may count.

The exact-main physical run retained Body
`a8fcb18cd76ce0dae75b656ee60ac86a5960cc3a0a0a350f327208ee9d7cb5f1`
and Pico Boot
`conduit-pico-w-signal/runtime-boot:000000000000038f:8c0ee18c6664641c4993266c24244c66`.
Production-kernel Plan A
`f3a41d5410f9a0ab77e42ea0f724b76e0df4e1be0bc08285651b078d2d48ff81`
delivered terminal plus two-Chromium inputs over WebSocket with verified
physical LED receipts. Real loss of the isolated Wi-Fi interface produced
explicit Plan B
`c1fb594033afc46e9a79dbb4bef2979daac3ef1c0417a716c128313c935e3f6f`
over USB CDC. After Wi-Fi restoration, Plan C
`959e291dd338bb38b38ca769f8bc30ed04e78d74cf7bf73c5ad53776b7ad93b6`
survived a second real link loss and continued the same Plan and Play over its
already-admitted USB route without a planner request. Each branch reached
reciprocal terminal state, Lull, and a later fresh Wake. The terminal
`conduit.r1/complete-hil@1` receipt requires
`combined_physical_acceptance=true`, `same_membership_body=true`, completed
new-Plan recovery, completed same-Plan continuation, and the exact nested
membership receipt.

This closes the bounded one-Body destination without adding a second runtime,
planner, scheduler, registry, authority system, or renderer-owned membership
truth. It adds no multi-Body control center, public federation, package or
firmware distribution, arbitrary remote execution, general PKI product, or
SOUL-continuity claim. Firmware flashing remains a repository-development
preparation step and is not part of Body membership or the demonstrated
operator admission flow.

The AArch64 Product Spine P5 slice from #1184 merged through PR #1190 as exact
main `49f776634a380c0da01ea65e548fbcb00d5883cd`. Its implementation head
`77e6a424b7922e63dbdda5224f5a7915f24e4c9a` passed 24 checks in workflow
`31726402453`, including the distinct AArch64 product lane and the unchanged
A0-A4 architecture lanes. Push workflows `31726808829` and `31726808857`
then passed on that exact merge commit.

The checked `conduitos/aarch64/virt` PROFILE lowers through the same Host BUILD
contract into a distinct long-lived `conduitos-aarch64-product` artifact and
final `BOOTAA64.EFI` image. Two independent QEMU `virt`/Cortex-A72 boots under
`QEMU_EFI.fd` require exact ProfileId, BuildId, resolved Image binding, fresh
HostId and BootId, and a process still alive after its ready Sign. The guest
constructs its current HostOffer from embedded fabrication plus live AArch64
machine truth, manifests the ordinary zero-Body front-door Presentation through
the admitted `presenter/linear-serial@1` and PL011 Base, and executes the
canonical portable tiny Form through an ordinary production-kernel Plan/Play
to `HELLO, CONDUITOS`.

The separate `cargo xtask conduitos product-readiness-matrix` keeps AArch64
local interaction and ordinary Body lifecycle explicitly false: there is no
PL011 input Base and no proof-harness command channel promoted into product
authority. It records only the earned PROFILE artifact, bootable image,
IMAGE-bound offer, long-lived zero-Body Host, linear Presenter, and
noninteractive ordinary Plan/Play cells. Product modules remain feature-fenced
out of A0-A4 proof appliances. This adds no graphics, USB/xHCI, SMP, physical
execution, HIL, or parity claim, and does not promote terminal parsing into
Presentation truth.

The portable Presentation semantic-action implementation from #1193 merged
through PR #1199 as exact main
`ef01019c09540230acf3524c0449ae132e6fc35e`. Its implementation head
`7521af89b6b77829b72af1365e6ee5617bc1b7ed` passed every check in workflow
`31730223340`, including browser-host, x86 front-door and product-journey, and
the distinct AArch64 product double-boot lane. Exact-main push workflows
`31730703199` and `31730703179` then passed the complete required check matrix
and Pages evidence publication on that merge commit.

One portable `Presentation` can now carry finite exact semantic action
identity, ordinary Conduit intent, subject target, human label, disclosure
level, and an available, unavailable, or refused state. Unavailable and
refused states retain bounded machine-readable reason codes and explanations.
The read-only exact-revision resolution check neither invokes an action nor
grants authority. Action, availability, reason, and disclosure mutations enter
the Presentation content identity; geometry, gesture, key, DOM, and renderer
facts remain absent.

The checked zero-Body Patchbay and ConduitOS front doors present OPEN FORM and
BIRTH as distinct actions over the same Form. OPEN is available for inert
inspection. BIRTH remains visible and unavailable with
`authority/not-admitted` when the entrance has no admitted authority to create
a Body. Native and browser Manifestations retain the exact semantic records;
the deterministic linear Presenter emits those same records for the AArch64
product. Duplicate action identity, unknown target, oversized reason, unknown
disclosure subject, stale revision, unknown action, unavailable action, and
refused action all fail closed without runtime mutation.

This is deterministic and hosted/emulated software evidence. Human enactment
for #1193 remains pending until a person records that the zero-Body entrance
makes the Form, distinct OPEN and BIRTH actions, unavailable reason, and
inspection-only exact identities understandable. Automated browser evidence
and coding-agent inspection do not satisfy that human claim. This slice adds
no gesture binding, layout, action execution engine, authority source, Text
Lab, physical execution, or HIL claim.

The executable interactive-Presentation and webchat migration from #1597
merged through PR #1599 as exact main
`5d0a9f84530906e1e60aa406b456b6fc28eb1458`. Its final implementation head
`2dd15fb6d6abf02cec5ff9d89443e127787b0a80` passed every PR check. Exact-main
workflow `32409065376` then passed the complete required matrix, including the
Rust 1.98 locked workspace lint/test shards and `browser-host`. A separate
exact-main `cargo xtask prove browser-host` run passed 32 pinned-Chromium tests
with two optional Firefox cases skipped and all 11 Patchbay HTML tests.

Portable `Presentation` values can now carry finite exact text-entry input
descriptions and produce an exact typed `presentation/interaction` value bound
to Presentation, revision, Manifestation, input, action, target, kind, and
sequence identities. A bounded interaction ledger retains accepted, refused,
failed, cancelled, and platform evidence without retaining submitted plaintext.
Stale Presentation and Manifestation, unknown input or action, wrong target or
kind, empty, oversized, malformed, duplicate, pressure, evidence exhaustion,
cancellation, adapter loss, and delivery failure remain distinct. Planning
refuses when the exact finite interaction offer is absent.

The authored webchat Form now names only chat and Presentation meaning. Its
explicit six-operation graph realizes state, atomic tee fan-out, renderer,
interaction, submit, and external WebSocket operations through the existing
planner and production kernel. The browser page contains no authored chat
controls, and the generic JavaScript renderer derives controls, labels,
availability, bounds, and targets from each Presentation. Click and Enter
converge on the same typed interaction path. Two actual Chromium clients
exchange bounded messages, a surviving client continues after peer loss, and
an authored label oracle changes the visible text without a JavaScript change.
The retired `web/text-input` and `web/list` compatibility meanings are absent.

This acceptance proves deterministic contracts, hosted execution, live
loopback transport, and pinned browser execution. It adds no native text-input
control, public network/TLS/auth service, renderer-owned policy or runtime
truth, browser scheduler, firmware execution, physical device, or HIL claim.

The composable human-facing browser Host sequence from #1645 merged through
H0-H3 and the final proof repairs in PRs #1678, #1681, and #1684. Its accepted
exact-main commit is `7e351676b955e8b18e297a86604f2b609d52ec7b`.
Required workflow `32685791138` passed the complete workspace, platform, and
pinned browser-host matrix; visual-evidence workflow `32685791180` separately
passed on that same commit.

Repeated `cargo xtask host browser` entrances bind independent ephemeral
loopback listeners and initialize fresh page-owned HostId, BootId, and bounded
WASM state. Reload replaces current identity and resources rather than
rehydrating them from browser persistence. The browser advertisement keeps
camera acquisition, microphone acquisition, typed human interaction, text and
graphics Presentation, and Lines as separate finite offers; availability and
browser reachability do not create Body membership or authority.

Camera and microphone acquisition cross an admitted browser-operation boundary
only after an immutable acquisition Plan seals semantic constraints, request
authority, and finite operation/value/time bounds. Granted browser-visible
opaque resource identity and use authority become new Host truth after the
browser result. The unchanged `camera-summary` Form then requires a distinct
ordinary Plan selecting that exact camera resource, authority grant, typed
`frame` ports, and finite WebRTC Line. Two explicitly admitted browser Parts
execute one bounded camera value through that selected Cord; source Host loss
invalidates the binding and leaves the unchanged Form unrealizable until fresh
resource truth and replanning exist.

Deterministic contracts and the pinned Chromium project keep denial, prompt
dismissal, unsupported constraints, no device, malformed completion, capacity,
pressure, cancellation, closure, track/page loss, and stale identity distinct.
Chromium fake-device mechanics are actual browser-adapter proof but not a
physical camera or microphone observation. This acceptance adds no implicit
membership, Plan mutation, automatic retry, persistence continuity,
browser-process identity, background-realtime guarantee, second runtime, or
physical/HIL claim.

The Host fabrication-package epic from #1766 merged through implementation PR
#1775 and its AArch64 proof-dispatch correction PR #1777. The corrected
exact-main commit is `89d72639d40c238aaaf2bbe7da23f764c29c5402`.
Required workflow `32924122237` passed the complete workspace and platform
matrix on that exact commit, including the five distinct ConduitOS architecture
lanes, two fresh AArch64 product boots, Pico, ESP32, and pinned Chromium. The
pre-correction #1775 run `32922406643` is not acceptance evidence: its AArch64
architecture lane truthfully refused after the product artifact was routed to
an A3 proof entrance.

`HostFabricationPackage` is now the package seam. One deterministic project
composition installs finite anchor and extension contributions. Anchors own
exact target descriptors, package revision, toolchain identity, finite maximum
bounds, artifact kinds, and permitted post-build actions. Packages contribute
exact Base implementation offers. Host construction remains the authority for
choosing one exact target, implementations, and requested bounds; package-owned
facts cannot be replaced by caller input. BUILD and spore manifests seal the
resolved package, target, implementation, tool, artifact, and action provenance
into their exact identities. Duplicate or ambiguous contributions and every
unsupported or over-bound request fail before fabrication.

The workspace composition root installs family-sized packages for native/std,
browser, ConduitOS, Raspberry Pi, Pico/RP2040, and ESP32. ConduitOS retains five
distinct targets—x86_64, IA-32, AArch64, RISC-V64, and LoongArch64—inside one
coherent package. Raspberry Pi retains distinct B+ and Zero board descriptors
inside its family. Target-owned directories now contain their package contract
and repository-development fabrication mechanics; `cargo xtask` remains the
entrance and generic fabrication does not acquire target-specific deployment
policy. An independently compiled RP2040 PIO-audio extension proves that a new
package can enter the ordinary PROFILE → BUILD → IMAGE path without editing the
generic fabrication crate or an anchor package.

The common contract does not invent one firmware-shaped artifact or deployment
verb. Native uses BUILD → native bundle → LAUNCH; browser uses BUILD → browser
bundle → LOAD/LAUNCH; ConduitOS uses BUILD → disk/EFI IMAGE → BOOT; Pico uses
BUILD → UF2 → FLASH/BOOT; Raspberry Pi uses BUILD → SD IMAGE → FLASH/BOOT; and
ESP32 uses BUILD → firmware IMAGE → FLASH/BOOT. The predecessor schema and
directories are absent rather than supported by a compatibility layer.

This acceptance establishes deterministic package resolution and the existing
compile, firmware-build, emulator-boot, and browser proof classes run by the
workflow. Local Raspberry Pi work additionally built the ARMv6 ELF and
`kernel.img`, while complete SD-image assembly was unavailable because the
local environment lacked `mkfs.vfat` and `mcopy`; no physical Pi, flash, boot,
or HIL result is claimed. The seam adds no second runtime, scheduler, planner,
ambient package discovery, deployment authority, physical-board result, or
equivalence between BUILD, LOAD, LAUNCH, FLASH, and BOOT.

The Patchbay Body-workbench epic from #2021 completed its P0-P5 ladder through
the capstone implementation in PR #2040, merged as exact main
`321640f74cd81af5d71d5a32671363b8838a4663`. Its final head passed required
workflow `33485697008`. The related Book integration in PR #2041 and Pages
carrier correction in PR #2042 then merged as
`1da57bbd8199a46ea144628bb4bf11350e9e401a` and
`735dded896821e1ff1dae3e3fe6ddbc03c96c41e`. PR #2042's exact head passed
required check workflow `33492273684` and product workflow `33492273756`.
Exact-main product workflow `33493449325` passed release construction,
products-proof, product assembly, Pages upload, and deployment on `735dded8`.

One typed `cargo xtask prove patchbay-body-workbench` entrance now retains a
bounded machine-readable identity ledger tying the Body evidence schema,
identity, event sequence, and graduation choice to Patchbay attachment,
current frame, biography, semantic navigation, selected subjects, and Exact
and Linear projections. Hosted attachment retains the exact planned
implementation; external attachment refuses to invent placement. Both project
the same graduated Body and biography truth. Program, Body, and History answer
the human-facing product questions, while Follow crosses from an exact planned
Cord to its realized Line without adding Body facts to authored Form truth.
The golden lulled Body offers Wake only through retained authority and action
evidence.

Deterministic native composition and pinned Chromium prove equivalent subjects
and evidence across native, browser, Exact, and Linear manifestations rather
than pixel parity. The negative matrix keeps malformed, oversized, unsupported,
ungraduated, wrong, stale, mismatched, absent, offline, refused, failed,
cancelled, and no-authoritative-time cases distinct. Closing or removing a
manifestation leaves serialized evidence and canonical Body truth
byte-identical.

The deployed Book reuses the production ReactFlow Patchbay renderer and exposes
the reviewed implementation Back for the same-Face comparison. Live Chromium
verification on exact-main deployment `735dded8` observed two production
ReactFlow roots, six production faceplates, the working Flip Back transition,
and the retained implementation Kind and checked implementation Form. The Book
handoff resolves to `https://dancxjo.github.io/conduit/creche/`, and the
independently deployed Crèche answered successfully at that path.

This acceptance establishes bounded deterministic contracts, native semantic
composition, pinned browser execution, and the deployed Book and Crèche
carrier. It adds no new semantic feature or event family, renderer-owned truth,
realtime/background guarantee, physical device execution, or HIL claim.

The Patchbay debugger P0 observation protocol from #2084 merged through PR
#2104 as exact main `0bec7d0252e912cdaac29c628740d2627ba4392e`.
Exact-head workflow `33607221717` passed after one infrastructure-only retry:
the first ConduitOS tool-bundle preparation timed out while downloading Ubuntu
packages, while the retry restored the completed exact bundle and passed every
selected workspace, firmware, ConduitOS architecture, product, and x86 proof.
The focused `conduit-kernel` suite also passed again on the merged commit.

The production kernel now offers a versioned optional debugger projection
beside mandatory Signs. Its fixed-capacity records bind observations to exact
Body, Plan, Play, Host, Form, Gear, Port, Cord, and plan-lowered type identities;
retain collector and per-Host monotonic sequences without inventing wall-clock
time; and bound both retained records and value previews. Small values remain
inspectable, while larger values expose exact byte length, truncation, and a
bounded preview. Unsupported schemas and event kinds, stale execution identity,
malformed previews, and nonmonotonic Host input refuse distinctly.

The v1 event vocabulary is Gear started/completed, Value sent/received, and
Fault. Debugger overload overwrites only the oldest optional history and records
an exact loss gap; it cannot stall or fail ordinary execution. One observer may
attach to or detach from a live scheduler through a narrow lifecycle contract
that does not expose mutable mandatory Signs. Deterministic tests prove the
same execution result with or without observation, including faults, and prove
one Body-wide Play containing work from multiple Forms.

This acceptance adds no Patchbay animation, Watch or timeline UI, replay,
breakpoints, transport, global event store, scheduler policy, arbitrary
full-value capture, authoritative timestamp, or physical/HIL claim. Those
remain downstream slices of #2083.

The Patchbay debugger P1 live projection from #2085 merged through PR #2106 as
exact main `cce3614ed9e447fe50f95c329bc06736d5bc2962`. Exact-head workflows
`33612616806` and `33612616816` passed the affected workspace lint, Host and
product shards plus the focused pinned-Chromium Book/Patchbay proof in at most
2m23s; ESP32, ConduitOS, and browser-Host matrices were correctly unselected.
Exact-main deployment workflow `33612918224` then passed all release builds,
staged-product Chromium proofs, artifact assembly, and Pages deployment. The
focused model, HTML, native, and deterministic JavaScript projection suites
also passed again on the merged commit.

One finite renderer-neutral debugger presentation consumes the P0 kernel
records and binds activity to exact current execution and admitted Gear, Port,
Cord, Host, and optional Line subjects. It refuses stale Play identity,
unknown subjects, Host drift, unsupported events, and nonmonotonic sequences
without replacing current debugger state. A 10,000-record stream coalesces by
exact subject while retaining observed/coalesced counts, latest bounded typed
preview, and an explicit telemetry-loss gap. Active, recent, inactive, and
retained-fault states remain distinct; decay is presentation policy, and fault
codes remain inspectable until explicitly cleared or the execution context is
replaced.

Browser and native Patchbay consume the same serialized semantic state over
the canonical graph. Gear and Port faces and exact Cord paths receive live
annotations; bounded scalar/text previews, counts, fault details, and
authoritative Host/Line bindings remain textual as well as visual. Browser
motion occurs only for observed active execution, respects reduced-motion
state, and has a bounded live region. Native rendering proves the same exact
subject consumption without pixel-parity claims. Detaching the overlay leaves
canonical topology and execution byte-identical.

This acceptance adds no Watch history, timeline/replay, breakpoint or resume
control, causal trace, invented reroute, independent scheduler, Book-only fake
execution, authoritative wall-clock time, transport, or physical/HIL claim.
Those remain #2086, #2087, and #2088 under #2083.

The Patchbay debugger P2 Watch interaction from #2086 merged through PR #2108
as exact main `558c70faf84a5c18d7e70a377cedd041c4e45707`. Exact-head workflow
`33617687965` passed every selected workspace, ESP32, ConduitOS architecture,
product, and x86 proof because the PR also taught the impact planner about its
new focused workflow. Focused pinned-Chromium workflow `33617687947` passed in
2m23s. The same model, Host endpoint, JavaScript syntax, and one-worker,
zero-retry Chromium Watch proof passed again from the merged commit; the model
cases themselves completed in 0.01s and the browser interaction in 1.8s.

One renderer-neutral finite Watch set now lives beside canonical Presentation
and Form truth. Up to eight Watches bind exact Gear, Port, or Cord presentation
subjects to one exact Body/Plan/Play debugger execution. Each retains at most
32 ordered observation entries with latest value or state, event kind, planned
type identity, update count, honest sequence-domain density, explicit eviction,
and telemetry-gap evidence. Replacement execution and disappeared subject
lifecycles remain distinct, and neither friendly labels nor replacement graph
subjects can silently retarget retained state.

The Host-owned browser interaction accepts add, focus, clear-history, and
remove actions only against exact Presentation and Watch revisions. Scalar,
bounded/truncated text, bytes/opaque, and fault projections remain readable as
text; each retained event exposes its exact subject, execution, sequence, and
event detail. Keyboard creation, focus, clearing, removal, and same-server
browser reload retention are proven through the production Patchbay surface.
Clearing or removing debugger state leaves the canonical Presentation and
execution topology byte-for-byte unchanged.

The focused `patchbay-debugger-pr-proof` gate builds only Patchbay, runs five
deterministic model cases, and exercises one Chromium scenario with one worker
and zero retries. The impact planner now classifies that workflow as focused,
with regression assertions that ordinary debugger workflow changes select
lint/browser only and do not select ESP32 or ConduitOS. This acceptance adds no
timeline/replay, breakpoint or runtime resume control, causal tracing,
unbounded history, authoritative wall-clock rate, or physical/HIL claim. Those
remain #2087 and #2088 under #2083.

The Patchbay debugger P3 observation replay from #2087 merged through PR #2110
as exact main `d7476d72388b6e057596b2695045de1936b34715`. Exact-head workflow
`33620077102` passed only the affected workspace lint, Host, and product shards;
ESP32, every ConduitOS architecture and image/tool lane, browser Host, and
browser-tool lanes were correctly unselected. Focused pinned-Chromium workflow
`33620077191` passed in 2m48s. From the merged commit, 14 debugger model cases,
the complete Patchbay HTML/server suite, strict focused clippy, JavaScript
syntax, and both one-worker, zero-retry Chromium debugger scenarios passed
again; the model cases completed in 0.01s and the browser scenarios in 3.8s.

One finite renderer-neutral observation timeline now retains at most 128 exact
events and 64 KiB. Each event binds its Body, Plan, Play, Host, Form, subject,
optional related subject, event kind, typed bounded value or fault, global
sequence, and Host sequence. Live and replay use the same projection. Pausing
the visualization fixes only its cursor while new observations continue to be
admitted; previous, next, explicit event selection, and jump-live never suspend
or mutate execution. The projection reconstructs subject activity and the
latest Watch value at or before the selected cursor within one exact execution
context.

Event-to-graph selection and exact graph-subject filtering are two-way through
the existing Host-owned semantic navigation. Event rows expose their exact
execution and subject identities, and Watches remain readable while scrubbing.
Keyboard controls require no drag gesture, reduced motion preserves all textual
truth, and the UI explicitly distinguishes replay pause from execution pause.
Replacement execution contexts remain separate and the prior context stays
inspectable rather than being silently retargeted.

Overflow eviction, observer telemetry loss, and incomplete historical
reconstruction are explicit. Unknown subjects, stale revisions, invalid
cursors, unsupported events, and nonmonotonic input refuse without replacing
the accepted timeline. This acceptance adds no runtime suspension or resume,
breakpoint expression, causal ancestry or descendants, speculative topology
trace, distributed control, authoritative wall-clock time, or physical/HIL
claim. Those remain #2088 under #2083.

The Patchbay debugger P4 breakpoint and causal-trace slice from #2088 merged
through PR #2112 as exact main
`734613dd9a83d15e66da486cb8e4ac73d90eddbb`. Its prerequisite impact-planner
correction merged separately through PR #2113 as
`340b6ae199c922d6036449ce058d7dad499aa5a1`: a complete recognizable debugger
kernel change selects the focused proof, while an incomplete scheduler-only
change remains conservatively broad. At the final implementation head,
general workflow `33625057308` passed only workspace lint, Host tests, and
product tests in at most 2m28s; ESP32, all ConduitOS architecture/image/tool
lanes, browser Host, and browser tools were correctly unselected. Focused
pinned-Chromium workflow `33625057264` passed in 2m02s and Book browser workflow
`33625057408` passed in 1m55s.

The production fixed scheduler now owns one exact unconditional
before-Gear-start breakpoint and suspended execution state. Suspension occurs
before a scheduling decision, repeated steps remain suspended without
advancing execution, and an exact one-shot resume makes that same Gear the next
decision. Breakpoint and resume requests bind exact Body, Plan, Play, and Gear
identities; replacement execution is stale rather than label-retargeted, and
unsupported multi-Host distributed suspension refuses explicitly. The
Patchbay Host exercises these production kernel contracts rather than a
visualization-only pause or a second scheduler, including one Body-wide Play
whose work may belong to multiple Forms.

Observation schema v2 records exact invocation and causal-parent sequences.
The production scheduler links a value send to its emitting Gear invocation,
a receive to the exact prior send on its planned Cord, and completion or fault
to the latest event in that invocation. The finite timeline derives upstream
ancestry and exact observed descendants only from those identities; it does
not substitute graph reachability, and evicted or absent parents remain an
explicit history gap. Trace selection, ordered textual steps, graph emphasis,
Watches, and fault origin share one Host-owned atomic projection. Running,
visualization replay pause, runtime suspension, stale control, and unsupported
control remain distinct renderer-neutral states.

From exact main, seven kernel observation/control cases, 16 focused debugger
model cases, the complete Patchbay HTML/server suite, and all three pinned
Chromium interactions passed again. Chromium used one worker, zero retries,
and completed in 6.0s. The browser proof includes exact Watch reload retention,
timeline/graph synchronization, real suspension and resume, and exact causal
fault tracing. This acceptance adds no conditional breakpoint language,
arbitrary memory inspection, unbounded history, invented causal inference,
distributed stop-the-world protocol, authoritative wall-clock time, or
physical/HIL claim.

The parent Patchbay realtime-debugger outcome from #2083 is accepted through
the five ordered P0-P4 slices recorded above: #2084, #2085, #2086, #2087, and
#2088 are closed, and their production paths share one finite
renderer-neutral observation/debug model. P0 supplies exact versioned runtime
observations, finite retention, explicit loss, stale-execution refusal, and
non-mutating observer lifecycle. P1 projects those observations over the
canonical Gear/Port/Cord/Host/Line graph with typed transient values, bounded
coalescing, activity decay, retained faults, reduced-motion truth, and common
native/browser semantics. P2 adds exact type-sensitive Watches with finite
history. P3 makes live and replay one projection with bounded scrubbing and
two-way event/graph selection. P4 adds real kernel suspension/resume and
causal-parent tracing rather than renderer-authored control or graph guesses.

Together these slices answer the parent product question from a visible bad
value: its exact subject and typed bounded value remain spatially visible; its
ordered event and retained Watch history establish when it occurred; exact
invocation/send/receive parent identities establish where it came from and the
path actually observed; and missing telemetry remains an explicit gap rather
than a fabricated explanation. Cross-Host and optional planned-Line subjects
use the same exact presentation identity, while a carrier or route change is
shown only when authoritative Plan/Play observations establish it. Detaching
the debugger, replaying history, clearing Watches, and selecting traces do not
mutate canonical Body, Form, Plan, Play, or topology truth.

This parent acceptance does not add route/retry event kinds beyond the initial
versioned vocabulary, claim a USB-to-Wi-Fi physical demonstration, infer
causality from topology, make observation mandatory for correctness, or claim
unbounded/full-value capture, authoritative wall-clock timing, distributed
stop-the-world control, or physical/HIL proof.

The all-target ConduitOS product-Host campaign from #2090 is accepted at exact
main `6972998a482dab165dc6be00b1338c03c016627c`. Its target-owned slices landed
through #2099 at `433e349b5c8e8a13a8e586b9dcc4d01954aca82d` for IA-32 PC,
#2101 at `b71d6e0cb32e07dffb034b8bbb4bc3e52c655033` for RISC-V64 virt,
and #2120 at `1c0c03272794bd465cb39764b055eb72f12c9bd3` for LoongArch64
virt. Those slices retain their recorded deterministic conformance, two fresh
pinned emulator boots, exact PROFILE/BUILD/IMAGE and Host/Boot correlation,
ordinary checked Form through production Plan/Play, bounded Observatory and
Patchbay projection, and malformed, foreign, stale, unavailable, exhausted,
pressure, cancellation, and unsupported refusals. Emulator execution remains
emulator proof and makes no physical or HIL claim.

Umbrella integration PR #2156 made all five ConduitOS product targets ordinary
Crèche selections, release artifacts, exact spores, and staged product inputs
instead of proof-only aliases or refusals. Follow-up PR #2159 preserved one
durable physically admitted Body across reload and kept the generic browser
release product-pure under its unchanged 4 MiB admission ceiling. Exact-head
workflow `33685625127` passed the full selected workspace and ConduitOS proof
matrix; workflow `33685625134` compiled and sealed all five ConduitOS product
IMAGEs and every other release carrier, then passed 68 staged-product tests in
pinned Chromium with one worker and zero retries. Exact-main deployment
workflow `33686957407` resolved that successful carrier, checked out
`6972998a482dab165dc6be00b1338c03c016627c`, admitted the carrier against that
exact merged tree, and deployed it. This integration adds no networking,
peripheral expansion, SMP, preemption, isolation, physical, or HIL claim.

The Windows x86_64 and macOS arm64 native Host release slice from #2096 is
accepted at the same exact main commit. Implementation PR #2097 introduced
distinct package-owned target identities, native `cargo xtask host release`
selection, sealed manifests and executables, three separate hosted-computer
Crèche profiles, and Body-bound ZIP preparation. In exact-head product
workflow `33685625134`, native jobs `host-release-windows-x86_64` and
`host-release-macos-aarch64` compiled, sealed, and uploaded their exact
artifacts successfully; the pinned-Chromium aggregate selected each target,
acquired its sealed manifest, and produced its Body-bound ZIP while preserving
the existing Linux path. Exact-main workflow `33686957407` admitted the same
proven carrier against the merged tree. BUILD and download remain distinct
from installation, launch, Host/Boot truth, join, membership, and Play; this
acceptance adds no signing, notarization, installer, package-manager,
automatic-launch, Windows arm64, Intel macOS, or runtime-equivalence claim.


## Accepted Tour identity and repository ownership (#2294, #2275)

Stable promotion [#2771](https://github.com/dancxjo/conduit/pull/2771) admitted
frozen development commit `9eb7d8e1c3669e9146e890c454029732a84e5689` through
immutable promotion head `a097e2c0328c00e9222067b6fd504af1fcd9e545`.
The exhaustive [promotion run 34038540549](https://github.com/dancxjo/conduit/actions/runs/34038540549)
passed on that head. Merged `main` commit
`41af600ae9267fd630088c5af25f347b449e008c` has the same exact tree,
`2a2fb59618a947eaa37ae98588f838a82cbfd761`. The jobs ran on the frozen
promotion head; merged-main acceptance is established by that exact tree
identity, not by claiming a second execution on the merge wrapper.
[Development integration 34038500474](https://github.com/dancxjo/conduit/actions/runs/34038500474)
also passed on the selected development commit.

The accepted current product is Tour: `products/tour`, application identity
`conduit.application/tour`, and the public `/conduit/tour/` route. The finite
legacy `/book` redirect map preserves query and fragment. Saved drafts retain
`conduit.application/book-reading-state@1` as their deliberate compatibility
identity; bounded legacy reading-state documents migrate to the Tour schema,
and malformed or over-capacity state refuses without being rewritten.
Historical Book evidence elsewhere in this file remains historical truth.

The [repository layout guide](docs/repository-layout.md) defines the accepted
ownership map. Products are Conduit, Tour, Crèche, and Patchbay; Pete is
Body-owned; canonical Forms use the reviewed `forms/inventory.toml` rather
than filesystem discovery as authority. Crèche selects ordinary Forms into
workload revision 0. Product browser assets and package descriptors are
product-owned; generic browser admission and effects remain target-owned.
Root support files now belong to target, Body, product, proof, documentation,
or repository-tool owners. Target layout follows responsibility names while
preserving package identities and declared browser resource URLs. The tracked
repository taxonomy guard refuses retired roots, misplaced browser product
assets, and noncanonical Form paths while ignoring untracked local artifacts.

The exhaustive gate passed workspace formatting, lint and tests, target
build/proof jobs, product staging, and four pinned-Chromium browser shards:
Pages 4 tests, browser Host 47, Tour 47, and Crèche machines 22, with one worker
and zero retries. These cover direct/deep/history-aware Tour routes, package
admission and exact refusals, legacy routes and drafts, peer navigation,
independent product trees, fourth-product conformance, canonical workload
selection, and target adapter packaging after relocation. Package aggregate
identity refuses before resource fetching; individual resource bytes remain
checked before launch. The Pages carrier was built from this proven snapshot.

This acceptance does not create a future Book or house implementation, a
second Form inventory, a Seed replacement ontology, a product scheduler, or a
uniform target artifact format. It adds no physical/HIL or installation claim.
Failed promotions #2767 and #2769 remain failed evidence; their corrected
proof and admission paths are accepted only through the successful snapshot
above. Forms-as-gears #2291 and the embodied-house slice #2293 retain their
separate implementation and proof obligations.
