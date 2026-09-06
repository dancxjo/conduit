# Conduit

**The Body is the computer.**

Conduit is an experimental programming system for making **one logical computer, a Body, from one or many physical and virtual computers**.

One Body might be an eight-core PC. Another might combine a browser for the interface, a laptop for heavier computation, a VM for a service, and a Pico for a button and LED. Conduit uses the same basic model for both.

A Conduit **Form is a program**: a graph of semantic work that says what should happen without baking the current machine arrangement into its meaning. The Body can carry Forms while its current Hosts, Boots, resources, and connections change.

> **Describe the programs. Build the Body. Let Conduit work out how the Body can realize its work now.**

Conduit is still experimental. [STATUS.md](STATUS.md) is the authoritative record of what the repository has actually proved today.

## Try the smallest thing

The fastest introduction is the [Conduit Tour](https://dancxjo.github.io/conduit/tour). It lets you edit real Forms, run them in a browser Host, and inspect them through the real Patchbay renderer.

A small Form looks like this:

```conduit
form hello {
    upper: text/upper
    show: presentation/text

    "Hello, world." > upper > show
}
```

The Form asks for uppercase text and presentation. It does not ask for Linux, stdout, a browser, a process, a particular CPU, or a transport. Those are realization facts.

From a source checkout, run an example with:

```sh
conduit run forms/hello/main.conduit
```

Open Patchbay with:

```sh
conduit patchbay --on native
```

or use the repository convenience entrance:

```sh
just patchbay
```

## The idea in two pictures

A Body does not have to be distributed:

```text
Body Roseau
└─ one physical computer
   ├─ CPU cores
   ├─ memory
   ├─ storage
   ├─ display
   └─ devices
```

The same model can span unlike machinery:

```text
Body Roseau
├─ browser
├─ laptop
├─ virtual machine
├─ Raspberry Pi
└─ microcontroller
```

The machine boundary is therefore not the definition of the computer. A **Host** is one current running environment that contributes truthful finite implementations and resources. The **Body** is the logical computer whose continuity matters.

This is also why Conduit treats local operating-system scheduling and distributed scheduling as the same kind of problem at different topology and cost scales. Scheduling work across several cores on one machine should not require a separate ontology from scheduling compatible work across several Hosts.

## A compact mental model

| Concept | Meaning |
|---|---|
| **Form** | A program: connected semantic work expressed as Gears, typed Ports, and Cords |
| **Body** | The logical computer that can run Forms and persist while its machinery changes |
| **Host** | One current running environment offering finite implementations and resources |
| **Initial Forms** | The bounded zero-or-more Form workset active when a Body is born; ordinary Forms from revision zero onward |
| **Wake** | A Body-wide interval in which Conduit actively maintains the Body's work |
| **Plan** | One exact immutable realization of the Body's current workload on current machinery |
| **Play** | The current execution of that Plan |
| **Spore** | A Body-bound target-native artifact used to instantiate intended machinery |

The long-term Body model is deliberately **Body-wide**: a Body may run several Forms at once, while one logical Body scheduler plans their resource use together. One current Plan and at most one current Play describe the Body's running realization; many Forms and Gear instances may execute concurrently inside that Play.

That multi-Form Body-wide scheduling work is active architecture work in [#2062](https://github.com/dancxjo/conduit/issues/2062), not a claim that every part of it is already implemented. Again, [STATUS.md](STATUS.md) is the capability boundary.

## Forms describe meaning

A Form is a graph, not a script whose line order secretly becomes execution order.

```text
Kind   reusable semantic behavior
Gear   one configured occurrence of a Kind in a Form
Port   typed directional semantic point
Cord   typed connection between compatible Ports
Face   the stable contract visible to surrounding meaning
Back   a reviewed Form that implements a Face using more Conduit meaning
```

A surrounding Form depends on a Gear's **Face**, not on one particular implementation. A reviewed **Back** can open that Gear into smaller Gears. A Host may instead offer a direct implementation of the same Face.

That separation is what lets the same program survive changes in placement and machinery without pretending those machines are identical.

For a guided explanation, use the [Tour](https://dancxjo.github.io/conduit/tour) rather than treating this README as the language manual. The deeper architectural contract lives in [the Conduit canon](docs/conduit-canon.md).

## The Body is the computer

A Body can have one Part or many. Its current Parts may be realized by hosted Linux computers, browser Hosts, ConduitOS machines, microcontrollers, virtual machines, or other exact targets.

Hosts and Boots are current machinery. They can appear, disappear, reboot, or be replaced without automatically erasing the Body.

A Body is born with a bounded initial active Form set. It may contain zero, one, or many Forms, and no member is privileged because it was present at birth. Later Form additions and removals use the same Body-wide workload mechanism without replacing Body identity.

The [Crèche](https://dancxjo.github.io/conduit/creche) is the temporary application for beginning that lifecycle: birth a Body, prepare or attach machinery, observe exact joins/admission, and graduate so the nursery can go away while the Body remains.

## Plans make current realization exact

Forms say what the work means. Hosts say what machinery is actually available. A Plan records one finite admitted answer for the current situation.

A Plan can bind implementation choices, placements, resources, Lines, authority, limits, and exact Host/Boot identities. Plans are immutable. If the world changes enough to need a different answer, Conduit creates a replacement Plan rather than editing the old one in place.

A Play executes the current Plan.

For a Body with several Forms, admission must be global. Two programs cannot each be independently promised the same last CPU lane, device, or memory budget. This is the architectural reason the scheduler belongs to the Body rather than to each Form separately.

## Spores and machinery

Body building prepares exact machinery without pretending that a build artifact is already a running Host.

A **spore** is the target-native, Body-bound artifact you can carry to the intended target and use with that target's ordinary mechanism. Depending on the target that may be a UF2, HEX, merged flash image, IMG, ISO, ZIP, or another native package.

Writing, flashing, loading, or launching a spore does not by itself manufacture current presence, membership, trust, authority, offers, a Plan, or a Play. Those remain runtime facts.

See [Body building and spores](docs/body-building.md) for the exact construction and deployment boundaries.

## What is real today

Conduit has executable proof across materially different environments, but different proof classes mean different things. A build is not a boot, a browser compile is not a browser run, and a generated firmware image is not physical hardware evidence.

Current accepted work includes, among other things:

- canonical `.conduit` Forms with checked and expanded semantic identity;
- one finite port-aware execution kernel used across production paths;
- hosted and browser execution;
- live bounded Lines including WebSocket and USB CDC paths;
- physical Pico W execution with correlated evidence;
- Body membership and current/offline presence distinctions;
- native and browser Patchbay projections over authoritative state;
- checked Host and Body fabrication;
- target-native Body-bound spores for several target families;
- ConduitOS boot, input, presentation, and execution proofs at explicitly bounded rungs.

Read **[STATUS.md](STATUS.md)** before making a capability claim. It is intentionally more precise than this overview.

## Explore Conduit

- **[Conduit home](https://dancxjo.github.io/conduit)**: the public front door.
- **[Conduit Tour](https://dancxjo.github.io/conduit/tour)**: learn Conduit by running it.
- **[Crèche](https://dancxjo.github.io/conduit/creche)**: birth and provision a Body.
- **[Try Conduit](docs/try-conduit.md)**: repository-oriented runnable guide.
- **[Conduit canon](docs/conduit-canon.md)**: durable architecture and design distinctions.
- **[Repository layout](docs/repository-layout.md)**: architectural owners and where contributions belong.
- **[STATUS.md](STATUS.md)**: exact current proof and capability boundary.
- **[Patchbay](products/patchbay/README.md)**: visual application and projection over Form, Body, Plan, Play, Host, and evidence truth.

## From a source checkout

Check prerequisites:

```sh
cargo xtask doctor
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

Machine-oriented ConduitOS proof remains separate:

```sh
cargo xtask conduitos run --arch x86-64
cargo xtask conduitos prove --arch x86-64
```

Physical hardware work is intentionally gated. See [STATUS.md](STATUS.md) and the relevant target documentation before treating a build as permission to touch hardware.

## Design center

A few rules explain much of the repository:

- **The Body is the computer.** It may be one machine or many.
- **Program = Form.** Do not invent a second semantic Program object around Forms.
- **Meaning is not placement.** Forms do not name machines or transports just to obtain execution.
- **Faces are not implementations.** A Back expresses more meaning; a Host offers concrete realization.
- **One logical scheduler belongs to the Body.** Host-local kernels execute admitted work rather than inventing competing global policy.
- **Plans are exact and immutable.** Changed truth may require another Plan.
- **A Line is not a Cord.** Connectivity can change without changing the semantic graph.
- **Availability is not authority.** Reachability, membership, trust, and permission remain separate.
- **Boundedness is part of correctness.** Pressure and exhaustion are explicit facts.
- **Proof classes do not collapse.** Compile, simulation, hosted execution, browser execution, firmware, and physical evidence are different claims.

## Contributing

Read [AGENTS.md](AGENTS.md) before substantial architecture work. The primary repository gate is:

```sh
cargo xtask check
```

Public executable workflows belong under `conduit`. Repository development, fabrication, demonstrations, hardware work, and proof belong under `cargo xtask`. `just` remains a thin convenience layer.

The repository map and exact ownership rules live in [AGENTS.md](AGENTS.md) and the architecture documentation rather than being duplicated here.

## In one sentence

**Conduit makes one logical computer from the computers you have, lets that Body run Forms as programs, and works out how the Body's current machinery can realize that work now.**
