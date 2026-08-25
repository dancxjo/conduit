# Conduit

**The Body is the computer.**

Conduit is an experimental programming system for building one computer out of the computers you actually have.

You describe **what should happen** in a portable **Form**. Separately, body building prepares the Parts that may embody it: Linux machines, browsers, ConduitOS systems, microcontrollers, and other Hosts. Each intended Host can be built into a deployable spore. When those spores boot and join, Conduit sees the current Body that actually came alive.

Conduit then finds a finite realization of the Form that fits that Body now. It seals that realization into an immutable **Plan** and executes the Plan as a **Play**. If the world changes, the meaning does not have to change with it. A different collection of Hosts, a failed Line, a busy processor, or a smaller machine may simply require a different Plan.

> **You describe the meaning. You build the Body. Conduit works out how that Body can make the meaning real now.**

![A useful mental model for Conduit: a Form describes what should happen, body building prepares Parts as spores, the current Body records what came alive, and Conduit creates an exact Plan and Play](assets/readme/useful-mental-model.svg)

## Try the smallest thing

A Conduit program is a graph of semantic operations, not a script that names the machine that will run it.

```conduit
form hello {
    upper: text/upper
    show: presentation/text

    "Hello, world." > upper > show
}
```

Run it with:

```sh
conduit run examples/hello.conduit
```

The Form asks for uppercase text and presentation. It does **not** ask for stdout, Linux, a browser, a process, a particular CPU, or a particular transport. Those are realization facts.

To look at the same system visually:

```sh
conduit patchbay --on native
```

From a source checkout, `just patchbay` is the friendly repository entrance.

## Why Conduit exists

Conduit is trying to make several awkward facts about real computers ordinary rather than exceptional.

- **One program can span unlike machines.** A laptop can handle presentation and human input while another Host does heavy compute and a microcontroller handles physical I/O. The Form does not need to become three platform-specific programs just because the realization crosses machine boundaries.
- **The whole Body can be more useful than any one machine.** A weak computer is not globally obsolete because one workload no longer fits. It may remain the best place for a display, keyboard, network interface, sensor, or other compatible work.
- **Current load matters.** A nominally powerful Host may be heavily burdened while a smaller Host is mostly idle. Scheduling should care about what can actually be admitted now, not merely which machine looks strongest on paper.
- **Tiny Hosts can stay tiny.** A constrained MCU should receive only the finite fragment it has been assigned. It need not store, understand, or schedule the whole Body.
- **Failure need not rewrite meaning.** If a Line disappears or a Host leaves, an immutable Plan may already contain an admitted alternative, or Conduit can make a fresh Plan from new truth. The Form and Body can remain the same.
- **Local models are ordinary work.** An LLM implementation is one possible realization of a semantic Gear. It consumes finite resources, competes with other work, can disappear, and can be placed elsewhere without becoming a privileged AI subsystem.

That is the forest. The rest of the architecture exists to make those statements precise.

## The model

### A Form is meaning

A Form describes semantic composition. Most executable meaning is built from a small set of concepts:

| Concept | Meaning |
|---|---|
| **Kind** | Reusable semantic behavior and its checked contract |
| **Gear** | One configured occurrence of a Kind in a Form |
| **Port** | Exact typed directional semantic point with a temporal contract |
| **Cord** | Typed semantic connection between compatible Ports |
| **Info** | Shaped typed data carried through a Cord |

A **Face** is the stable contract visible to surrounding meaning. A **Back** is another Form that implements that Face using more Conduit meaning. Recursive composition can therefore continue until the remaining leaves are operations some current Host can directly realize.

A Form is deliberately ignorant of placement. Host names, operating systems, processor counts, transport mechanisms, provider names, and transient load do not belong in ordinary semantic source merely to obtain execution.

### The Body is the computer

A **Body** is the durable computer assembled from the Parts Conduit can use to maintain some intended world. A Body may have one Part or many. Its Parts may be backed by very different machines.

A **Host** is one exact running environment that truthfully offers finite realizations and resources. A **Boot** identifies one current incarnation of that Host. A durable Part can remain part of the Body even while its current Host or Boot is absent.

This distinction lets the Body survive changes in machinery. The workstation can reboot. A browser page can disappear. A Pico can go offline. None of those events necessarily means the Body itself ceased to exist.

### A Plan is one exact answer

A **Plan** records one exact finite answer to the question: *how can this Form be realized by this current Body?*

The Plan seals implementation choices, placements, resource bindings, authority, Cord realizations, Lines, limits, and the exact Host/Boot identities that matter to that realization. A **Play** is the active execution of one Plan.

Plans are immutable. New Signs may make an old Plan unsatisfied, but they do not edit it. If the world has changed enough to require another realization, Conduit creates another Plan.

![A Seed births a durable Body that can retain its identity across multiple Wake, Plan, Play, and Lull transitions](assets/readme/identity-lifecycle.svg)

A **Wake** is one interval in which Conduit actively maintains a Body's obligations. A **Lull** ends that interval without deleting the Body. One Wake may therefore contain several successive Plans and Plays as current truth changes.

## Body building

Body building prepares the machinery intended to participate in a Body.

A checked `*.body.conduit` document describes the intended Parts and references reusable `*.host.conduit` construction documents. Each Host construction selects a target, Bases, implementation variants, and finite structural bounds. The existing fabrication path checks that construction, resolves a profile, performs a build, and produces an image. Body binding then packages that image as a spore for one intended Part or bounded self-joining flow.

A spore is **not** a Host and it is **not** a Boot. It is prepared machinery plus enough Body-directed binding and provenance to become useful after launch, boot, load, or flash. Building or deploying a spore does not manufacture current presence, current offers, Lines, Plans, Plays, or runtime authority.

The repository includes a checked multi-Host example:

```sh
cargo xtask body check profiles/bodies/pete-r1.body.conduit
cargo xtask body show profiles/bodies/pete-r1.body.conduit
cargo xtask body build profiles/bodies/pete-r1.body.conduit
```

See [Body building and spores](docs/body-building.md) for the exact artifact and deployment boundaries.

## One Conduit language

Forms, Host construction, and Body construction are different document roles, not different little languages.

Canonical source uses one tokenizer, value syntax, declaration machinery, span model, and diagnostic system. Files such as `examples/hello.conduit`, `profiles/host-configurations/linux-workstation.host.conduit`, and `profiles/bodies/pete-r1.body.conduit` describe different things but belong to the same Conduit language.

The language stays intentionally small. New architectural concepts should extend the common grammar only when necessary; they should not sprout a YAML, TOML, JSON, or ad hoc mini-language merely because a new subsystem needs configuration.

For Form source, the central relations remain simple: `:` associates a named Gear with a Kind or Form, `=` expresses an immutable declarative value relationship, and `>` expresses runtime flow. Faces use `(...)`; Backs use `{...}`.

See [the Conduit canon](docs/conduit-canon.md) and [runnable Form examples](docs/try-forms.md) for the language rather than treating this README as a full reference manual.

## Scheduling finite resources

Real machines are finite, and their current state changes. Conduit treats that as useful truth rather than something to hide behind an unbounded queue.

An implementation has a **need**: the finite resources required by one candidate realization. A Host has stable **offers**: the capacities and mechanisms it can provide. Current **observations** say what is free, busy, ready, unavailable, or expensive now. The resource owner then **admits** the need or refuses it atomically.

Those are ordinary scheduling words, not another layer of ceremonial vocabulary.

The distinction matters. A Host might have 32 processor lanes in total but only five currently unreserved. A model realization that can use between two and sixteen lanes may still run there, just more slowly than preferred. Another realization that requires at least six lanes must be refused until current truth changes.

The same law applies across machines and inside one machine. Choosing between two Hosts for model inference and choosing whether a new Play fits beside several existing Plays on one multicore processor are the same resource-admission problem at different scopes.

Admission and selection are also different questions. Hard requirements determine whether a candidate can run at all. Policy compares only the candidates that remain admissible. Current throughput, latency, queue pressure, memory pressure, transport cost, and locality may influence that comparison, but a favorable score can never resurrect an impossible realization.

The more explicit atomic admission work is tracked in [#1751](https://github.com/dancxjo/conduit/issues/1751). The stable resource contracts, scalable compute requirements, current resource observations, and locality cost machinery already exist and are being pulled back toward this simpler center.

## Hosts, Bases, Lines, and Signs

These concepts describe current realization rather than authored meaning:

| Concept | Responsibility |
|---|---|
| **Host** | Truthfully offers finite implementations, resources, and limits for one exact running environment |
| **Boot** | Identifies one current incarnation of a Host |
| **Base** | Platform or machine mechanism beneath Host offers |
| **Line** | One exact finite connectivity realization used by a Plan and Play |
| **Sign** | Bounded machine-readable truth about what is true or what happened |

A Base is not a Kind. A Line is not a Cord. Hardware existence does not automatically become a Host offer. Reachability is not membership, membership is not trust, and availability is not authority.

This separation is what allows a semantic Cord to survive a transport change, a Part to survive a Boot change, and a Form to survive a placement change.

![With a WebSocket-only Plan, Line loss requires a new USB Plan and Play; with a dual-Line Plan, USB continuation preserves Plan and Play identity](assets/readme/line-recovery.svg)

## What runs today

Conduit is still experimental, but the repository proves real execution across materially different environments. The authoritative claim boundary is [STATUS.md](STATUS.md); the README only gives the shape.

Current accepted work includes:

- canonical `.conduit` Forms with checked and expanded semantic identity;
- one finite, port-aware `conduit-kernel` used across production paths;
- native std execution and real Rust/WASM browser Hosts;
- bounded live WebSocket and USB CDC Lines;
- physical Pico W execution with correlated Sign receipts;
- multi-Part Body membership and offline/current-presence distinctions;
- exact replan versus same-Plan Line recovery semantics;
- ConduitOS image, boot, input, and execution work across several architecture profiles;
- native and browser Patchbay manifestations driven by authoritative semantic and runtime state;
- checked Host fabrication and canonical Host construction source;
- body building that composes checked Host construction into Body-bound spores;
- typed local-model and LLM realizations with finite model, queue, context, memory, and provider lifecycle limits.

These proofs do not collapse into one vague "works" badge. A build is not a boot. A browser compile is not a browser run. A generated firmware image is not a physical board transcript.

![Seven separate Conduit proof classes, from contracts through physical hardware-in-the-loop evidence](assets/readme/proof-classes.svg)

Read [STATUS.md](STATUS.md) before making a capability claim.

## Try more

Check local prerequisites with:

```sh
cargo xtask doctor
```

Open Patchbay:

```sh
conduit patchbay --on native
conduit patchbay --on browser
```

Start an independent browser Host:

```sh
cargo xtask doctor browser
cargo xtask host browser
```

Run the x86-64 ConduitOS demo in QEMU:

```sh
cargo xtask conduitos demo --arch x86-64
```

Machine-oriented ConduitOS checks remain separate:

```sh
cargo xtask conduitos run --arch x86-64
cargo xtask conduitos prove --arch x86-64
```

Physical Pico work is intentionally hardware-gated:

```sh
cargo xtask doctor pico
cargo xtask prove std-pico-usb --interactive
```

See [Try Conduit](docs/try-conduit.md) for the guided executable tour and [STATUS.md](STATUS.md) for exact prerequisites and proof scope.

## Boundedness is architecture

Every admitted Play has finite truth for the resources it can consume: operations, queues, bytes, values, memory, compute, model slots, Line/session capacity, evidence retention, authority, protected resources, and mandatory work.

Pressure, exhaustion, cancellation, provider loss, stale identity, unsupported behavior, and failure remain explicit outcomes. Conduit does not quietly turn them into unbounded buffering, hidden retries, or mutable Plan surgery.

This is what lets the same semantics remain meaningful on a workstation, browser, microcontroller, or Body made from all three.

## Repository map

| Path | Responsibility |
|---|---|
| `crates/` | Portable contracts, Form tooling, planner, kernel, runtime, catalogs, Body/Host/resource machinery, product CLI |
| `hosts/` | Hosted, browser, ConduitOS, and Patchbay platform realizations |
| `firmware/` | Constrained firmware targets and generated-image consumers |
| `profiles/` | Checked Host and Body construction source |
| `fixtures/` | Deterministic conformance fixtures fenced from production truth |
| `examples/` | Canonical executable Forms |
| `xtask/` | Repository development, fabrication, proof, doctor, and hardware workflows |
| `docs/` | Canon, architecture, runnable guides, truth boundaries, and design history |
| `assets/` | README and presentation assets |

If you are new to the codebase, start with `conduit run examples/hello.conduit`, then open Patchbay, follow [Try Conduit](docs/try-conduit.md), and read [the Conduit canon](docs/conduit-canon.md). Read [AGENTS.md](AGENTS.md) before changing architecture.

## Design rules worth remembering

- **The Body is the computer.** A machine is one possible Part of a larger realization, not the universal boundary of computation.
- **There is one Conduit language.** Different authored roles do not get independent DSLs.
- **Programs are graphs, not scripts.** Source order does not secretly become execution order.
- **Meaning is not placement.** Forms do not name machines, providers, or transports merely to obtain execution.
- **Kinds are not Gears.** Reusable behavior and configured occurrence remain distinct.
- **Faces are not implementations.** A Back expresses more meaning; a Host offers concrete realization.
- **Body building prepares spores; it does not create runtime truth.** A spore is not a Host or Boot.
- **A Plan is exact and immutable.** New truth may require another Plan but never repairs the old one in place.
- **A Line is not a Cord.** Connectivity can change without changing semantic graph identity.
- **Availability is not authority.** Reachability, membership, trust, and permission remain separate.
- **Boundedness is part of correctness.** Finite resources and explicit pressure are architectural facts.
- **There is one execution kernel.** Renderers, model providers, firmware adapters, and fixtures do not acquire private runtimes.
- **Proof classes do not collapse.** Compile, simulation, hosted execution, browser execution, transport, firmware, and physical evidence say different things.

## Contributing

Read [AGENTS.md](AGENTS.md) before substantial work. The primary repository gate is:

```sh
cargo xtask check
```

Public executable workflows belong under `conduit`. Repository development, fabrication, demonstrations, hardware work, and proof belong under `cargo xtask`. `just` is a thin convenience layer and should own no independent behavior.

## In one sentence

**Conduit lets you describe what should happen, build a Body from the computers you have, and truthfully realize that meaning across whatever Parts can actually make it happen now.**
