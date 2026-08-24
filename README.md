# Conduit

**Wire meaning together. Let Conduit work out how that meaning can exist here, now.**

Conduit is an experimental programming system for finite, typed, inspectable computation across heterogeneous machines.

You author **meaning**. Hosts truthfully describe the machinery they can provide. Conduit observes what is available now, admits finite resources, seals one exact immutable **Plan**, and executes that Plan as a **Play** through one bounded kernel.

The same semantic program can therefore be realized by a Linux process, a browser, ConduitOS, firmware, or several machines at once without pretending those environments are the same thing.

![A Conduit Form becomes an exact Plan and active Play by combining authored meaning with Host offers, Signs, and policy](assets/readme/meaning-to-play.svg)

> **Meaning says what must remain true. Hosts offer finite means. Admission decides what can run now. Plans make one realization exact.**

Conduit is not a visual node editor with a runtime bolted underneath it. It is not a workflow service, robotics framework, message broker, AI agent framework, RTOS, or browser application. It can present surfaces resembling all of those because they share one semantic substrate, one planner, one bounded kernel, and one truth model.

## The short version

A useful mental model is:

![A useful mental model for Conduit: authored Form, Host construction, and Body construction flow through fabrication, observed current truth, resource admission, immutable Plan, and Play](assets/readme/useful-mental-model.svg)

The project is converging on **one Conduit source language** for those authored roles. Different files may describe different things, but they should not become different little languages. Today canonical Forms are already `.conduit`; Host construction currently has a working TOML authoring path while [#1752](https://github.com/dancxjo/conduit/issues/1752) moves Host and Body construction onto the same canonical Conduit grammar.

## Start here

For the installed product:

```sh
conduit run examples/hello.conduit
conduit patchbay --on native
```

From a source checkout:

```sh
cargo xtask doctor
just patchbay
```

Patchbay is the shared human-facing front door. It is a Presenter over the same semantic and runtime truth used by the CLI and proofs, not a second graph database or scheduler.

[![Current accepted Conduit Patchbay overview showing the Form graph and structural view](https://dancxjo.github.io/conduit/current/patchbay/overview.png)](https://dancxjo.github.io/conduit/current/patchbay/overview/)

The screenshot is evidence of one Manifestation. The Form, Body, Plan, Play, Hosts, Lines, and Signs remain authoritative.

## One language, several kinds of authored truth

Conduit has different *things to author*, but they should share one language and one diagnostic model.

### Forms describe meaning

```conduit
form hello {
    upper: text/upper
    show: presentation/text

    "Hello, world." > upper > show
}
```

The Form says nothing about stdout, DOM, Linux, a browser, a particular CPU, WebSocket, USB, or which machine should perform `text/upper`.

Those facts belong to realization.

### Host construction describes machinery

A Host construction document answers a different question:

```text
What target are we building for?
Which Bases and implementation variants are included?
What finite structural capacities does this image expose?
```

The current repository already has a checked Host configuration model and:

```text
Host configuration
      ↓
   PROFILE
      ↓
    BUILD
      ↓
    IMAGE
```

Host construction is authored in the same canonical Conduit grammar as Forms, using a distinct document role:

```text
forebrain.host.conduit
brainstem.host.conduit
```

This is a source-language migration, not a new Host model. Both paths must lower to the same checked Host configuration and PROFILE truth before TOML authoring can be retired.

### Body construction describes intended composition

A **Body** is the durable continuant whose meaning may be realized by many Parts and Hosts over time.

Body building, tracked in [#1740](https://github.com/dancxjo/conduit/issues/1740), composes intended Hosts through the existing Host fabrication path and packages each resulting IMAGE with Body-directed binding material:

```text
BODY CONSTRUCTION
      │
      ├── Host construction -> PROFILE -> BUILD -> IMAGE
      ├── Host construction -> PROFILE -> BUILD -> IMAGE
      └── Host construction -> PROFILE -> BUILD -> IMAGE
                                      │
                                      ▼
                                   SPORES
```

A **Spore** is not a renamed IMAGE.

```text
SPORE
  = IMAGE
  + Body binding
  + Part identity or bounded invitation
  + deployment metadata
  + provenance
```

A Spore can be prejoined to an intended Part or carry bounded self-joining material. BUILD and deployment still do **not** fabricate a Boot, current presence, current Host offer, Line, Plan, or Play.

The intended source role is `*.body.conduit`, not another Body-specific DSL.

## The semantic graph

Most executable meaning is built from five concepts:

| Concept | Meaning | It is not |
|---|---|---|
| **Kind** | Reusable semantic behavior and its checked contract | A Rust function, thread, process, or machine implementation |
| **Gear** | One configured occurrence of a Kind in a Form | The Kind itself or a runtime task identity |
| **Port** | Exact typed directional semantic point with a temporal contract | A queue slot, socket, Base handle, or drawn jack |
| **Cord** | Typed semantic connection between compatible Ports | A WebSocket, USB connection, or other Line |
| **Info** | Shaped typed data carried through a Cord | An untyped byte bucket or automatically a Signal |

A Kind may also separate its visible semantic contract from a graph-level implementation:

- **Face**: the stable semantic contract visible to surrounding meaning.
- **Back**: another Form that implements that Face using more Conduit meaning.

Because a Back is a Form, realization may recurse:

```text
Face
  ↓
Back
  ↓
Gears
  ↓
another Back
  ↓
...
  ↓
leaf operation offered by a Host
```

A Back answers **"how can this meaning be expressed as more Conduit meaning?"**

A Host answers **"what can actually be realized here?"**

Those are deliberately different questions.

## The Body is the computer

Conduit does not require one privileged computer to contain the whole program.

```text
BODY
 ├── Part / Host A
 │     compute
 │     memory
 │     display
 │
 ├── Part / Host B
 │     model inference
 │     storage
 │
 └── Part / Host C
       sensor
       actuator
       constrained compute
```

A Form may span them. A Cord may cross a Line. A tiny Host may receive only its assigned fragment and need never comprehend the complete Body or Form.

This is why machine boundaries are not the fundamental scheduling boundary. A heavily loaded 32-core Host and a lightly loaded 8-core Host are just two places that may or may not be able to admit the same realization right now. Two competing Plays on one 16-core CPU are the same problem at a smaller scope.

> **A Body is the computer you need, assembled from the computers you actually have.**

## NEED -> OFFER -> OBSERVE -> ADMIT

This is the scheduling law Conduit is re-centering around in [#1751](https://github.com/dancxjo/conduit/issues/1751).

Keep these facts distinct:

| Concept | Meaning |
|---|---|
| **NEED** | Finite demand of one candidate realization before reservation |
| **OFFER** | Stable Host/Base capability and capacity |
| **OBSERVE** | Mutable current truth: free units, utilization, health, queue pressure, measured cost |
| **ADMIT** | Atomic resource-owner decision to reserve the Need or refuse it |
| **BIND** | Exact admitted resource entitlement sealed into Plan truth |
| **ASSIGN** | Transient runtime/Base mapping of that entitlement to concrete execution lanes |

A candidate implementation may say, for example:

```text
NEED
  memory             12 GiB
  inference slots     1
  compute
    minimum lanes     2
    preferred lanes   8
    maximum lanes    16
    service           shared
```

A Host may stably offer 32 lanes while current Signs say only 5 are unreserved.

That gives an admission result such as:

```text
minimum = 2
preferred = 8
maximum = 16
available = 5

ADMIT 5
```

or, if only one lane remains:

```text
REFUSE insufficient current capacity
```

### Admission is not selection

Conduit separates two questions:

```text
ADMISSIBILITY
Can this candidate satisfy every hard semantic/resource requirement now?

SELECTION
Among admissible candidates, which realization should policy prefer?
```

A candidate does not become valid because it is fast, cheap, local, or fashionable. Hard requirements filter first. Policy compares only candidates that can actually be admitted.

Current observations may influence selection:

- unreserved compute;
- current queue pressure;
- measured inference throughput or latency;
- memory pressure;
- Line cost and transport work;
- current provider readiness;
- explicit policy.

A measurement is a Sign with provenance and bounded validity, not timeless truth.

### Planning is not admission

The planner may conclude that a candidate *should* fit from observed truth. It is not the final owner of the capacity.

The resource owner must reserve required capacity atomically. Two planners cannot both spend the same four free lanes merely because both saw the same prior observation.

```text
planner: candidate should fit
             ↓
resource owner: ADMIT
         ├── success -> exact binding
         └── refusal -> fresh truth -> replan
```

No Plan surgery. No hidden retry that quietly changes realization.

### Plans own entitlements, not CPU numbers

A Plan may truthfully seal:

```text
this placement is admitted 6 suitable compute lanes
```

It should not normally seal:

```text
CPU 3, CPU 7, CPU 8, CPU 11, CPU 12, CPU 15
```

Concrete processor-lane assignment is runtime/Base truth. Equivalent lane reassignment during Play need not change Plan identity.

The same law scales from one multicore processor to a room full of machines.

## Hosts, Bases, Lines, and Signs

Real machines are finite, changing, and inconvenient. Conduit treats that as valuable truth.

| Concept | Responsibility |
|---|---|
| **Host** | Truthfully offers finite implementations, resources, and limits for one exact running environment |
| **Boot** | Identifies one current incarnation of a Host |
| **Base** | Platform/machine mechanism beneath Host offers |
| **Line** | One exact finite connectivity realization used by a Plan/Play |
| **Sign** | Bounded machine-readable truth about what is true or what happened |

Examples of Bases include timers, execution lanes, framebuffers, DOM mechanisms, USB controllers, sockets, audio devices, GPIO controllers, or model runtimes.

A Base is not a Kind.

A Line is not a Cord.

Hardware existence does not automatically become a Host offer.

Reachability is not membership. Membership is not authority. Availability is not permission.

## From source to a living Body

![A Seed births a durable Body that can retain its identity across multiple Wake, Plan, Play, and Lull transitions](assets/readme/identity-lifecycle.svg)

The lifecycle deliberately keeps durable intent separate from one attempt to realize it.

| Identity | Meaning |
|---|---|
| **Seed** | Dormant authored material from which a Body may be born |
| **Body** | Durable intended world and obligations |
| **Part** | Durable constituent relationship within a Body |
| **Wake** | One interval during which Conduit actively maintains Body obligations |
| **Plan** | One exact immutable realization admitted against one basis of truth |
| **Play** | Active execution of one Plan |
| **Lull** | End of a Wake without deleting the Body |

One Wake may contain several Plans and Plays:

```text
Body B
  Wake W
    Plan P1
      Play X
        becomes unsatisfied
    fresh Signs
    Plan P2
      Play Y
  Lull
```

A changed world never edits P1.

If P1 already admitted an alternative, Play may be able to continue inside the same Plan. Otherwise fresh truth produces a new Plan.

![With a WebSocket-only Plan, Line loss requires a new USB Plan and Play; with a dual-Line Plan, USB continuation preserves Plan and Play identity](assets/readme/line-recovery.svg)

The important invariant is not "always fail over." It is:

> **The Plan determines which changes may be absorbed and which require replanning.**

## Host fabrication

Host fabrication is orthogonal to Form semantics.

```text
Host construction
  target
  selected Bases
  implementation variants
  finite structural limits
        ↓
     PROFILE
        ↓
      BUILD
        ↓
      IMAGE
        ↓
 launch / boot / flash / load
        ↓
 current Host + Boot + offers
```

An IMAGE is machinery. BUILD does not create current runtime truth.

Current repository tooling can inspect and build Host configurations through `cargo xtask host ...`; architecture-package work keeps target-specific build mechanics behind narrow target/fabrication seams rather than teaching portable planning how to make UF2, EFI, ESP, browser, or disk images.

Body building composes this existing machinery rather than inventing a second Host construction system.

## LLMs are ordinary Gears

Conduit has no privileged AI runtime.

An LLM Gear has typed Ports, bounded work, ordinary authority, ordinary resource admission, and ordinary implementation selection. A model can interpret, generate, classify, extract, embed, propose, or compose, but model output is not automatically truth and model text cannot mint authority.

Different Hosts may offer materially different realizations of the same LLM Face:

```text
same semantic Gear

Host M
  local model implementation
  one Need profile
  current load / throughput Signs

Host F
  different local implementation
  different Need profile
  different current load / throughput Signs

planner + admission
  choose one exact realization now
```

No Form needs to name a particular machine, model server, accelerator API, model path, or CPU count merely to ask for the semantic operation.

This is the same general resource-selection machinery used for audio, storage, compute, presentation, networking, or future artificial-life field work.

## What is proven today

Conduit is experimental, but it is far beyond a paper architecture.

The authoritative itemized claim boundary is **[STATUS.md](STATUS.md)**. The README intentionally summarizes rather than reproduces every acceptance record.

Current repository proof includes, among other things:

- canonical `.conduit` Forms with checked and expanded semantic identity;
- named Faces and recursively expandable Backs;
- exact planning over Host implementations, resources, authority, placement, policy, and Lines;
- one finite port-aware `conduit-kernel` used across production paths;
- explicit pressure, cancellation, closure, stale identity, and terminal outcomes;
- native std execution;
- real Rust/WASM browser Hosts and browser execution;
- bounded live WebSocket and USB CDC Conduit Lines;
- physical Pico W execution and correlated Sign receipts;
- multi-Part Body membership and offline/current-presence distinctions;
- exact replan versus same-Plan Line recovery semantics;
- ConduitOS image/boot/execution work across multiple architecture profiles;
- native and browser Patchbay Manifestations driven by authoritative semantic/runtime state;
- Host PROFILE -> BUILD -> IMAGE fabrication with checked target/Base/variant/bounds truth;
- typed local-model/LLM implementations with finite model, queue, context, and memory limits;
- locality/resource machinery that already separates stable resource contracts from current utilization observations.

These proofs have different evidence classes. A build is not a boot. A browser compile is not a browser run. A generated firmware image is not a board transcript.

![Seven separate Conduit proof classes, from contracts through physical hardware-in-the-loop evidence](assets/readme/proof-classes.svg)

See [STATUS.md](STATUS.md) for the exact highest proven class of each surface.

## What is being built now

Several current architectural frontiers are especially important:

- **One source language**: [#1752](https://github.com/dancxjo/conduit/issues/1752) moves Host and Body construction onto the canonical Conduit grammar while preserving the already-working checked Host configuration model.
- **Body building and Spores**: [#1740](https://github.com/dancxjo/conduit/issues/1740) composes intended Host images into Body-bound deployable artifacts without fabricating runtime presence.
- **NEED -> ADMIT scheduling**: [#1751](https://github.com/dancxjo/conduit/issues/1751) makes resource admission the common law across heterogeneous Hosts and local multicore contention.
- **Body-aware product execution**: installed `conduit run` is being generalized from the smallest local case toward ordinary multi-Host Body execution.
- **Tiny assigned fragments**: constrained Hosts should receive only the Plan fragment they need, not the whole world.
- **Distributed artificial life**: a bounded reaction-diffusion/Lenia direction is being used as a forcing function for computation whose interesting object genuinely spans several tiny Hosts.

These are active work, not claims that every end state is already accepted on `main`.

## Try Conduit

You need a recent Rust toolchain. Platform-specific work may require additional tools reported by:

```sh
cargo xtask doctor
```

### Run a canonical Form

```sh
conduit run examples/hello.conduit
```

To retain a neutral runtime report:

```sh
conduit run examples/hello.conduit \
  --report /tmp/conduit-run.json

conduit inspect runtime-report /tmp/conduit-run.json
```

The report keeps semantic identity separate from realization identity: Host/Boot, capability offers, implementation placement, Plan, fragment, resource bindings, active Play, terminal state, and bounded Signs.

### Open Patchbay

Installed product:

```sh
conduit patchbay --on native
conduit patchbay --on browser
```

Repository convenience:

```sh
just patchbay
```

Machine acceptance:

```sh
cargo xtask prove patchbay-front-door
```

### Start a browser Host

```sh
cargo xtask doctor browser
cargo xtask host browser
```

This creates one independent page/WASM Host and Boot. It does not automatically join that browser to a Body.

Hosted browser proofs include:

```sh
cargo xtask prove std-browser-s4
cargo xtask prove std-browser-toggle
```

### Work with Host fabrication

Current Host construction tooling includes entrances equivalent to:

```sh
cargo xtask host config check profiles/host-configurations/linux-workstation.host.conduit
cargo xtask host config show  profiles/host-configurations/linux-workstation.host.conduit
cargo xtask host build        profiles/host-configurations/linux-workstation.host.conduit
```

Historical `*.host.toml` files remain migration fixtures only. Both source representations lower to the same checked model during migration, while ordinary authoring uses `.host.conduit`.

### See ConduitOS

For a visible x86-64 QEMU demo:

```sh
cargo xtask conduitos demo --arch x86-64
```

Machine proof entrances remain separate:

```sh
cargo xtask conduitos run --arch x86-64
cargo xtask conduitos prove --arch x86-64
```

### Physical Pico W work

Physical workflows are deliberately hardware-gated:

```sh
cargo xtask doctor pico
cargo xtask prove std-pico-usb --interactive
```

The larger Body membership/recovery proofs require the exact hardware, Lines, credentials, and safety/acceptance environment described by their commands and [STATUS.md](STATUS.md).

See **[Try Conduit](docs/try-conduit.md)** for the guided executable tour.

## Form syntax

Canonical Form source is graph-shaped and declarative. Statement order is not execution order.

```conduit
form greet (
    greeting: Text = "Hello"
    name: Text > text: Text
) {
    join: text/join(greeting)
    name > join > text
}
```

The core surface is intentionally small:

```text
form
:
=
>
(...)
{...}
$T
T...
T...|
```

Broadly:

- `:` says a named Gear has a Kind/Form.
- `=` expresses an immutable declarative value relationship.
- `>` expresses runtime value flow through Cords.
- `(...)` describes a Face or invocation startup arguments.
- `{...}` is a Form Back.
- `$T` is current observable state.
- open/closing flow notation keeps temporal behavior explicit.

Conduit is deliberately resistant to accumulating mini-languages. General concepts should extend the canonical grammar or remain typed data, not become another ad hoc DSL.

Learn more from:

- [canonical examples](examples/README.md)
- [runnable Form examples](docs/try-forms.md)
- [the Conduit canon](docs/conduit-canon.md)

## Boundedness is architecture, not a tuning knob

Every admitted Play must have finite truth for the resources it can consume.

That includes, where applicable:

- active operations;
- queues and bytes;
- value storage;
- compute lanes;
- memory;
- model slots;
- Line/session capacity;
- work bounds;
- evidence retention;
- authority;
- protected resources.

Pressure and exhaustion are real outcomes. Conduit does not convert them into an invisible unbounded queue or generic retry loop.

Fan-out is explicit. Cancellation is explicit. Stale identity is explicit. Provider loss is explicit. Unsupported behavior is explicit.

This is what allows the same semantics to make sense on a workstation, browser, tiny MCU, or several machines together.

## Repository map

| Path | Responsibility |
|---|---|
| `crates/` | Portable contracts, Form tooling, planner, kernel, runtime, catalogs, Body/Host/resource machinery, product CLI |
| `hosts/` | Actual hosted/browser/ConduitOS/Patchbay platform realizations |
| `firmware/` | Constrained firmware targets and generated-image consumers |
| `profiles/` | Current checked Host construction source/configuration fixtures |
| `fixtures/` | Deterministic conformance fixtures fenced from production truth |
| `examples/` | Canonical executable Forms |
| `xtask/` | Repository development, fabrication, proof, doctor, and hardware workflows |
| `docs/` | Canon, architecture, runnable guides, truth boundaries, and design history |
| `assets/` | README/Patchbay visual assets and vendored presentation material |

If you are new to the codebase:

1. Run `conduit run examples/hello.conduit`.
2. Open `just patchbay`.
3. Follow [Try Conduit](docs/try-conduit.md).
4. Read [the Conduit canon](docs/conduit-canon.md).
5. Read [STATUS.md](STATUS.md) before making a capability claim.
6. Read [AGENTS.md](AGENTS.md) before changing architecture.

## Design rules worth remembering

- **There is one Conduit language.** Different document roles should not become different DSLs.
- **Programs are graphs, not scripts.** Source order does not secretly become execution order.
- **Kinds are not Gears.** Reusable meaning and configured occurrence remain distinct.
- **Meaning is not placement.** Forms do not contain machine, provider, or transport facts merely to obtain execution.
- **Faces are not implementations.** A Back expresses more meaning; a Host offers concrete realization.
- **Hosts offer; implementations need.** Stable capacity and realization appetite belong to different owners.
- **Observed load is not capability identity.** Current performance/utilization is Sign truth.
- **Admission is not selection.** Hard feasibility comes before policy preference.
- **Planning is not admission.** Resource owners atomically reserve capacity.
- **Plans own entitlements, not incidental CPU IDs.** Physical lane assignment is runtime truth.
- **A Line is not a Cord.** Connectivity may change without changing semantic graph identity.
- **An IMAGE is not a Spore.** An IMAGE is Host machinery; a Spore binds machinery toward one Body.
- **BUILD is not BOOT.** Fabrication does not manufacture current Host/Boot truth.
- **Availability is not authority.** Reachability, membership, trust, and permission remain separate.
- **Signs are not Plans.** New truth can invalidate a Plan basis but never edit that Plan.
- **There is one execution kernel.** Fixtures, renderers, AI providers, and MCU adapters do not acquire parallel runtimes.
- **Proof classes do not collapse.** Compile, simulation, hosted execution, browser execution, transport, firmware, and physical evidence say different things.

## Contributing

Read **[AGENTS.md](AGENTS.md)** before substantial work.

The primary repository gate is:

```sh
cargo xtask check
```

Public executable workflows belong under `conduit`. Repository development, fabrication, demonstrations, hardware work, and proof belong under `cargo xtask`. `just` is a thin convenience façade and should own no independent behavior.

Changes should preserve exact distinctions among:

```text
source
checked meaning
Body / Part
PROFILE / BUILD / IMAGE / SPORE
Host / Boot / offers
NEED / OFFER / OBSERVE / ADMIT
Plan / Play
Line / Cord
authority
Signs
Manifestation
```

If one of those distinctions disappears merely because two things happen to share a Rust struct, CLI command, file, or machine, the abstraction is probably leaking.

## In one sentence

**Conduit lets you author one finite semantic world, build the machinery that may realize it, and truthfully schedule that meaning across whatever current collection of machines can actually admit it.**
