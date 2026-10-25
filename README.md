# Conduit

**One continuing computer, made from the computers you have.**

A computer can be more enduring than the machine doing its work.

A laptop shuts down. A browser tab closes. A sensor board loses power or is
replaced. Yet "is this still the same computer?" need not be answered by asking
whether one process or device is still running.

**Conduit is an experimental programming and execution system for composing a
continuing computer.** That logical computer is a **body**. It may inhabit one
**host** or several, each offering only the implementations and resources it
actually has. Body identity is separate from host and boot identity.

You write a **plot** describing what work means and how its parts relate.
Conduit checks the program, plans an exact and finite realization against
present offers, resources, and authority, then executes the admitted **play**.
Portable meaning need not carry operating-system, device, or transport choices
just because today's implementation does.

Continuity is a possibility, not perpetual execution or a requirement to grow.
A body may lull; a plot may complete; a purpose may be fulfilled. Those are
different facts. None alone implies automatic restart, preservation of every
in-flight value, or destruction of the body. Durable identity also depends on
surviving continuity evidence; it cannot be recovered merely by wishing it so.

Conduit is working software and an unfinished project. Try a checked program
below, explore the browser or ConduitOS, and consult [current status](STATUS.md)
for the exact implementation and proof boundaries.

## Try it

- **[Open the body Workspace](https://dancxjo.github.io/conduit/workspace/)** to meet the browser product and its resident plots
- **[See ConduitOS running](https://dancxjo.github.io/conduit/journeys/current/conduitos/x86_64/)** in the ten-step x86_64 QEMU journey
- **[Check the published build](https://dancxjo.github.io/conduit/current-product.html)** for development, accepted-release, publication, and proof identities

To run the first hosted example, install Git and [Rust through rustup](https://rustup.rs/),
then use the checkout's pinned toolchain:

```sh
git clone --branch dev https://github.com/dancxjo/conduit.git
cd conduit
cargo xtask make host std
```

This builds the repository tooling and runs [Hello](plots/hello/main.conduit)
through the production checker, planner, and kernel. A native linker is needed;
browser and hardware tools are not prerequisites for this example. The first
build takes time and disk space. See [Try Conduit](docs/try-conduit.md) for the
browser, native workbench, and ConduitOS routes.

## Read a plot

```conduit
plot hello {
    upper: text/upper
    show: face/text("Message")

    "Hello, world." >> upper >> show
}.
```

The literal supplies text, `text/upper` makes it uppercase, and
`face/text` presents it. The full stop after `}` makes structural drain
semantic completion. The source chooses no OS, window, socket, or device.

Three small groups explain the model:

| Question | Terms |
|---|---|
| What should happen? | A **plot** composes semantic **kinds**. Each **gear** is one occurrence of a kind; its **fore** is the callable signature. Typed **ports** connect through **cords** carrying **info** |
| How can it happen here? | A **host** offers **backs** that realize kinds. A **plan** selects exact backs, resources, authority, lines, and finite bounds |
| What is happening? | A **play** executes one plan through bounded **steps**. **Signs** record evidence; a replacement plan handles changed reality explicitly |

A **fore** is the function-like signature an author calls. A **face** is the
body's human-facing meaning and agency. A **mask** realizes that face as a
**show**. Callability and human presentation are distinct concepts.

Canonical Conduitese pairs two small vocabularies. A **type** says what a value
means; a **form** says the concrete portable shape in which that value is
carried. A **plot** says what should happen; a **plan** selects how it will
happen here; a **play** is that plan executing. In short: `type -> form` and
`plot -> plan -> play`. See the
[current language reference](wiki/Current-language-surface.md) for checked
examples and their limits.

## Choose a reading path

| You want to… | Start here |
|---|---|
| Understand the idea | [Why Conduit](wiki/Why-Conduit.md), then the [architecture tour](wiki/Architecture-tour.md) |
| Learn the language | [Conduitese](wiki/Conduitese.md), [examples](wiki/Conduitese-by-example.md), then [try plots](docs/try-plots.md) |
| Find an exact contract | [Language reference](wiki/Current-language-surface.md), [glossary](wiki/Glossary.md), or [architecture reference](docs/architecture/README.md) |
| Run a target or inspect a body | [Try Conduit](docs/try-conduit.md), [targets](targets/README.md), [Patchbay workbench](plots/patchbay/workbench/README.md) |
| See what works and what remains | [Current status](STATUS.md) and [roadmap](docs/roadmap.md) |
| Make a change | [Contributing](CONTRIBUTING.md) and the [repository map](docs/repository-layout.md) |

The [documentation index](docs/README.md) is the full map. The
[website handbook](https://dancxjo.github.io/conduit/handbook/) is built from
the Markdown and diagrams in this repository's `wiki/` directory. The separate
[GitHub wiki](https://github.com/dancxjo/conduit/wiki) is maintained manually as
a short signpost to the [main website](https://dancxjo.github.io/conduit/).

## Boundaries that matter

- Authored plots describe meaning; hosts and bases own platform machinery
- A plan is immutable and admits finite storage and mandatory work before play
- There is one execution kernel across hosted, browser, and constrained targets
- Reachability, membership, trust, and authority remain separate
- A cord is a semantic connection; a line is its exact concrete carriage
- Quiescence, completion, cancellation, pressure, and failure remain distinguishable

Continuity, purpose fulfillment, semantic completion, and lifecycle retirement
are different facts; a Body is not obliged to invent more work to persist.

These are design obligations, not a claim that every target supports every
feature. Source checks, deterministic tests, browser runs, emulator runs,
physical hardware, human use, and accepted releases prove different things.
[STATUS](STATUS.md) records that distinction; the [canon](docs/conduit-canon.md)
explains the enduring design.

## Repository map

| Owner | Responsibility |
|---|---|
| `architecture/` | Identities, checking, planning, body lifecycle, and kernel execution |
| `semantics/` and `plots/` | Portable meaning and reviewed authored compositions |
| `mechanisms/`, `make/`, and `targets/` | Reusable mechanisms, manufacturing, and platform realization |
| `bodies/` and `products/` | Concrete bodies and human-facing products |
| `proof/` and `tools/` | Evidence and repository development workflows |
| `wiki/` and `docs/` | Learning, language reference, guides, architecture, and retained history |

You do not need to understand the whole system to help. Run something, find one
behavior you can explain, and improve it. [CONTRIBUTING](CONTRIBUTING.md)
covers setup, verification, and a focused pull request to `dev`.
