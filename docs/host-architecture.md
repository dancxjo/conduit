# Host core, families, bases, and compositions

**Scope:** Host composition contracts, originating in issues #463 and #514.
The inventory below describes the current source; release proof is tracked separately.

**Current proof boundary:** [STATUS.md](../STATUS.md)

**Durable vocabulary and invariants:** [The Conduit canon](conduit-canon.md)

Conduit defines a portable host contract, not one privileged host implementation.
The distinctions below are load-bearing:

```text
host core contract
    + selected operation/capability families
    + bases and platform adapters for those families
    + host-specific policy and configuration
    = one concrete host composition
    -> one boot-scoped advertisement of exact current offers
```

## What each layer means

- **Host core contract:** boot-scoped identity, exact finite advertisements,
  planning inputs, Play start/effect/completion protocol, and Sign correlation.
  It has no mandatory filesystem, process, socket, display, audio, GPIO, USB,
  Wi-Fi, Tokio, Embassy, DOM, or operating-system method.
- **Thin Host supervisor:** the small realization of that core which owns
  Host/Boot and offer-generation truth, aggregates a bounded registry of live
  Bases, participates in the shared planner, accounts for Plays, and supervises
  Base lifecycle. It does not receive the underlying authority of a Base.
- **Catalog category:** an organizational namespace such as `text/`, `time/`, or
  `state/`. A category is neither a base nor a planner promise.
- **Kind:** one reusable semantic callable face, such as `text/upper`. Compatibility
  is equality of its canonical checked face. Its authored name and revision are
  provenance and diagnostics, not nominal compatibility gates (issue #522).
- **Family:** a composition boundary that can contribute a related set of
  Kind implementations. Selecting a family includes machinery; it does not
  itself advertise a category prefix or promise every Kind in that category.
- **Base/platform adapter:** the implementation of an admitted effect or Line,
  such as a Rust timer, browser DOM presentation, CYW43 GPIO, or USB CDC.
  It does not plan, schedule, invent connectivity, or define semantic meaning.
- **Host composition:** the deliberately selected families, bases, and
  policy in one binary or firmware image.
- **Runtime offer:** one exact boot-scoped `CapabilityOffer`, including canonical
  checked face, implementation/artifact provenance, limits, resources, and
  authority requirements. This is planner truth.

Compile-time inclusion is therefore only an upper bound. Startup conditions,
resource admission, policy, and initialization can make the runtime offer set a
strict subset. The planner consumes only the resulting exact offers.

## Current inventory

| Surface | Classification | Current role and boundary |
|---|---|---|
| `conduit-core::HostAdvertisement` and capability/resource/authority/link facts | host core and planner advertisement | Portable, bounded facts; no platform methods |
| `conduit-core::ThinHostSupervisor` and `BaseRegistry` | thin Host/Base registry contract | Bounded current provider identity, generation, lifecycle, enforcement class, capabilities, and resources; registration grants no authority and ready offers feed the ordinary `HostAdvertisement` |
| `conduit-kernel` operation protocol | host core execution contract | Numeric admitted effects/completions; owns no platform implementation |
| [`conduit-plan-lowering`](architecture/plan-kernel-lowering.md) | plan-to-kernel boundary | Lowers exact selected placements under an explicit fixed storage profile; not a host composition |
| `conduit-planner` | planner | Matches canonical checked faces against current offers, then admits exact facts |
| `conduit-semantic-catalog` | host-neutral semantic catalog | Portable Kind/value contracts and deterministic semantic calculations; it owns no Host realization offers |
| `conduit-signal` host-profile modules | capability contracts and profile fixtures | Shared Signal faces plus exact std/browser/Pico offers used by accepted vertical proofs |
| `targets/std::StdHostComposition` | host composition | Selects explicitly enabled implementation families; `reference()` is broad and `minimal()` promises none of them |
| `targets/std` timers, stdout, WebSocket, and USB code | bases/platform implementations | Real std effects and lines beneath selected plans; WebSocket/USB are not host-core methods |
| `targets/browser/host` | browser Host product entrance and assets | Authoritative browser Host launcher, HTTP delivery, fabrication package, and generic browser adapters |
| `targets/browser/runtime` | browser composition and bases | Exact browser/WASM offers with timer/DOM/WebSocket machinery; not a compatibility runtime |
| `proof/browser` | browser conformance evidence | Playwright specifications, proof pages, proof-only adapters, configurations, and local proof server; not another browser Host |
| `targets/rp2040/firmware/pico-w-signal` and generated image | Pico W composition and bases | Selectable fixed Signal images; local-minimal omits Conduit session/lifecycle control, while physical-proof and remote modes include it explicitly; no general Pico capability claim |
| fixture hosts and legacy drivers | legacy/compatibility coupling | Test-only fenced paths; not production host definitions or a second accepted runtime |

The std `reference()` composition is the batteries-included example, not the
definition of `Host`. `minimal()` demonstrates that the same host shell can be
constructed without mandatory operation families, while a text-only composition
demonstrates that compiled code for other families does not become an ambient
runtime promise.

## Reference compositions today

The checked examples intentionally expose different sets:

- std reference: Signal, time, text, input, state, logic, math, layout,
  presentation, robotics, files, HTTP, JSON, and ALife families. Optional
  external WebSocket, raw MIDI, and audio resources have separate selections;
- browser distributed sink: the exact presentation face required by that image;
- Pico-local Signal image: the exact pulse and GPIO-backed presentation faces;
- Pico-local-minimal image: the same exact Signal faces and Sign base,
  without the optional Conduit wire/session or BOOTSEL lifecycle-control family;
- Pico USB/triple remote images: the exact GPIO-backed presentation sink face
  plus the explicitly selected bounded USB session-control base;
- minimal std composition: no production operation offers;
- text-only std composition: the implemented `text/literal`, `text/upper`,
  `text/join`, and `presentation/text` subset, not the entire `text/` namespace.

Resources and reachability remain separate from offers. Compiling or initializing
a base does not authorize it, and selecting a family does not bypass resource,
authority, policy, or link admission.

## Base registry and lifecycle

Each registry entry names one exact boot-scoped Base identity, initialized
provider instance, monotonically advancing provider generation, implementation,
mechanism family, descriptive enforcement class, lifecycle state, and finite
offer/resource inventory. Only `Ready` entries contribute planner-visible
offers. `Lost`, stopped, starting, or merely configured providers do not.

Lifecycle mutation and replacement require the exact current Base, provider
instance, and generation. Restart or replacement advances generation, so stale
providers cannot mutate current truth. Loss of one Base removes only its offers;
Host/Boot identity and unrelated Bases remain current. External discovery under
a Base does not recursively create Hosts.

The registry is neither a plugin manager nor an authority store. It performs no
effects, chooses no placements, and makes no policy decisions. Pure capabilities
can be advertised by the same thin Host with no effect Base installed, while all
planning continues through the shared `HostAdvertisement` contract and planner.

## Public vocabulary and Rust names

The source and user-facing model uses these terms consistently:

```text
text/upper           Kind
upper: text/upper    Gear named upper
text                 catalog category
```

Public Rust identifiers use the same vocabulary deliberately:

| Current internal name | Public meaning |
|---|---|
| `KindId` | reusable semantic behavior discovery/provenance identity |
| `KindContractRevision` | revision provenance for a Kind contract; not a compatibility gate |
| `GearId` | exact identity of one authored/expanded Gear occurrence |
| `CheckedGear` | one checked Gear and its required canonical face |
| `CapabilityOffer` | one boot-scoped exact Host offer for a Kind |
| `PlannedGear` | one Gear bound to an exact Host offer and realization |
| `ImplementationId` / `ArtifactId` | selected realization provenance sealed by the Plan |

These names are the public vocabulary: Kind names reusable semantic behavior;
Gear names its configured occurrence in a Form. Ordinary source never names
`ImplementationId`, `ArtifactId`, a Base, or a platform.

## Remaining architecture work

This slice does not establish a general downstream BYOKernel composition API or
separate every base boundary. Pico's reviewed Signal images are selectable,
but they are not a general Pico host generator. Those are follow-up architecture
and tooling slices, not missing claims of the accepted #463 first slice. This also
does not create a dynamic registry, package manager, plugin ABI, mega-Host trait,
or one Cargo feature per operation.
