# Host core, families, providers, and compositions

**Status:** accepted first host-architecture slice from issues #463 and #514

**Current proof boundary:** [STATUS.md](../STATUS.md)

**Durable vocabulary and invariants:** [The Conduit canon](conduit-canon.md)

Conduit defines a portable host contract, not one privileged host implementation.
The distinctions below are load-bearing:

```text
host core contract
    + selected operation/capability families
    + providers and platform adapters for those families
    + host-specific policy and configuration
    = one concrete host composition
    -> one boot-scoped advertisement of exact current offers
```

## What each layer means

- **Host core contract:** boot-scoped identity, exact finite advertisements,
  planning inputs, activation/effect/completion protocol, and evidence correlation.
  It has no mandatory filesystem, process, socket, display, audio, GPIO, USB,
  Wi-Fi, Tokio, Embassy, DOM, or operating-system method.
- **Catalog category:** an organizational namespace such as `text/`, `time/`, or
  `state/`. A category is neither a provider nor a planner promise.
- **Operation:** one semantic callable face, such as `text/upper`. Compatibility
  is equality of its canonical checked face. Its authored name and revision are
  provenance and diagnostics, not nominal compatibility gates (issue #522).
- **Family:** a composition boundary that can contribute a related set of
  operation implementations. Selecting a family includes machinery; it does not
  itself advertise a category prefix or promise every operation in that category.
- **Provider/platform adapter:** the implementation of an admitted effect or
  carrier, such as a Rust timer, browser DOM presentation, CYW43 GPIO, or USB CDC.
  It does not plan, schedule, invent connectivity, or define semantic meaning.
- **Host composition:** the deliberately selected families, providers, and
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
| `conduit-kernel` operation protocol | host core execution contract | Numeric admitted effects/completions; owns no platform implementation |
| `conduit-runtime::lowering` | plan-to-kernel boundary | Lowers exact selected placements; not a host composition |
| `conduit-planner` | planner | Matches canonical checked faces against current offers, then admits exact facts |
| `conduit-std-catalog` | semantic catalog plus some reference offers | Operation contracts are semantic; offer constructors are current reference-host implementation facts |
| `conduit-signal` host-profile modules | capability contracts and profile fixtures | Shared Signal faces plus exact std/browser/Pico offers used by accepted vertical proofs |
| `hosts/std::StdHostComposition` | host composition | Selects existing Signal, time, text, and state implementation families; `reference()` is broad and `minimal()` promises none of them |
| `hosts/std` timers, stdout, WebSocket, and USB code | providers/platform implementations | Real std effects and carriers beneath selected plans; WebSocket/USB are not host-core methods |
| `hosts/browser-runtime` | browser composition and providers | Exact browser/WASM offers with timer/DOM/WebSocket machinery; not a compatibility runtime |
| `firmware/conduit-pico-w-signal` and generated image | Pico W composition and providers | Selectable fixed Signal images; local-minimal omits Conduit session/lifecycle control, while physical-proof and remote modes include it explicitly; no general Pico capability claim |
| fixture hosts and legacy drivers | legacy/compatibility coupling | Test-only fenced paths; not production host definitions or a second accepted runtime |

The std `reference()` composition is the batteries-included example, not the
definition of `Host`. `minimal()` demonstrates that the same host shell can be
constructed without mandatory operation families, while a text-only composition
demonstrates that compiled code for other families does not become an ambient
runtime promise.

## Reference compositions today

The checked examples intentionally expose different sets:

- std reference: Signal, time, text, and state operation families;
- browser distributed sink: the exact presentation face required by that image;
- Pico-local Signal image: the exact pulse and GPIO-backed presentation faces;
- Pico-local-minimal image: the same exact Signal faces and evidence provider,
  without the optional Conduit wire/session or BOOTSEL lifecycle-control family;
- Pico USB/triple remote images: the exact GPIO-backed presentation sink face
  plus the explicitly selected bounded USB session-control provider;
- minimal std composition: no production operation offers;
- text-only std composition: the implemented `text/literal`, `text/upper`,
  `text/join`, and `presentation/text` subset, not the entire `text/` namespace.

Resources and reachability remain separate from offers. Compiling or initializing
a provider does not authorize it, and selecting a family does not bypass resource,
authority, policy, or link admission.

## Public vocabulary and current internal names

The source and user-facing model uses these terms consistently:

```text
text/upper           operation
upper: text/upper    cell named upper
text                 catalog category
```

Some established Rust identifiers predate that vocabulary. Their current
conceptual mapping is deliberate:

| Current internal name | Public meaning |
|---|---|
| `KindId` | semantic operation discovery/provenance identity |
| `KindContractRevision` | revision provenance for an operation contract; not a compatibility gate |
| `OperationId` | exact identity of one authored/expanded cell occurrence |
| `CheckedOperation` | one checked cell and its required canonical face |
| `CapabilityOffer` | one boot-scoped exact host operation offer |
| `PlannedOperation` | one cell bound to an exact host offer and realization |
| `ImplementationId` / `ArtifactId` | selected realization provenance sealed by the Plan |

These internal names remain compatibility-sensitive APIs. Renaming them would be
a broad mechanical migration with little semantic benefit, so #511 explicitly
defers it. New user-facing documentation and diagnostics should say operation
for the semantic callable and cell for its occurrence; ordinary source never
names `ImplementationId`, `ArtifactId`, a provider, or a platform.

## Remaining architecture work

This slice does not establish a general downstream BYOKernel composition API or
separate every provider boundary. Pico's reviewed Signal images are selectable,
but they are not a general Pico host generator. Those are follow-up architecture
and tooling slices, not missing claims of the accepted #463 first slice. This also
does not create a dynamic registry, package manager, plugin ABI, mega-Host trait,
or one Cargo feature per operation.
