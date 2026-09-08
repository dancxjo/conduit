# Conduit

**One logical computer, made from the computers you have.**

Conduit is an experimental programming system for connecting computation,
interfaces, and devices into one continuing computer: a **Body**. That Body
might live on one machine, or span a laptop, browser, server, and microcontroller.
Its programs describe what should happen; Conduit checks how the available
machinery can realize that work.

> **[See ConduitOS running: the illustrated visual journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)**
>
> Follow the actual x86_64 system through Tour, Patchbay, keyboard and pointer
> interaction, and USB Line attachment. The 17 screenshots come from a QEMU
> journey with correlated runtime evidence. This is emulator evidence; physical
> laptop support is an ongoing project.

**[Try the interactive Tour](https://dancxjo.github.io/conduit/tour/)** ·
**[Start contributing](CONTRIBUTING.md)** ·
**[What works](STATUS.md)** ·
**[What we're building next](docs/roadmap.md)**

## A program is a Form

```conduit
form hello {
    upper: text/upper
    show: presentation/text

    "Hello, world." > upper > show
}
```

This **Form** connects an uppercase transformation to text presentation. It
produces `HELLO, WORLD.` without naming an operating system, display, device,
or transport. A **Host** offers implementations and resources. An immutable
**Plan** records the exact choices and finite limits. A **Play** executes that
Plan through the shared kernel.

Forms are graphs of configured **Gears**, joined at typed **Ports** by **Cords**.
A Form can also appear as a Gear inside another Form through its public
**Face**. Its **Back** exposes the composition behind that contract. You can
inspect those relationships in Patchbay.

The Body gives this work continuity: it can contain several Forms and retain
its identity while machinery goes offline or is replaced. Membership,
connectivity, and permission are separate decisions. The
[architecture guide](docs/conduit-canon.md) explains the design.

## What you can explore

| Surface | What it does | Start here |
|---|---|---|
| **Tour** | Run and edit real Forms in a browser, with embedded Patchbay inspection | [Open Tour](https://dancxjo.github.io/conduit/tour/) |
| **Patchbay** | Inspect and compose Forms; explore Body state, execution, Watches, and retained observations | [Patchbay guide](products/patchbay/README.md) |
| **Crèche** | Birth a Body, select its initial Forms, and prepare target-specific machinery | [Open Crèche](https://dancxjo.github.io/conduit/creche/) |
| **ConduitOS** | Run the same architectural kernel on a freestanding Host, with an x86_64 graphical shell and other serial targets | [Visual journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/) · [Build and boot](targets/conduitos/README.md) |
| **Devices and distributed work** | Exercise bounded Host membership and Lines, including recorded physical Pico W control and recovery | [Target guides](targets/README.md) · [Proof limits](STATUS.md) |

These are working experimental slices with different levels of proof. Conduit
is not yet a general desktop OS or a turnkey distributed application platform.
The [current status](STATUS.md) distinguishes demonstrated behavior, available
implementation, and remaining integration or hardware work.

## Run from a checkout

Install [Rust through rustup](https://rustup.rs/) and Git. The checkout pins its
Rust toolchain; Cargo installs that toolchain when needed.

```sh
git clone --branch dev https://github.com/dancxjo/conduit.git
cd conduit
cargo xtask host std
```

The final command builds and runs the local Hello Form. For the interactive
browser experience, also install Node.js and npm, then add the WASM target:

```sh
rustup target add wasm32-unknown-unknown
cargo xtask demo tour
```

The first build takes longer than subsequent runs. `cargo xtask doctor` reports
prerequisites for several targets; missing browser-proof or Pico tools do not
block the local Hello example. [Try Conduit](docs/try-conduit.md) covers native Patchbay,
browser Hosts, and further examples.

`conduit` is the installed product command. `cargo xtask` builds and runs
repository development workflows. The checkout commands above work without
assuming that `conduit` is already installed.

## Help build it

Contributions to examples, explanations, usability, tests, and implementations
are welcome. Start by running something, then choose a small improvement you
can explain and verify. The [contributor guide](CONTRIBUTING.md) covers setup,
finding the right files, and opening a PR against `dev`.

Current work includes making the ConduitOS shell easier to use, correcting the
default lifecycle of quiet programs, and completing the House and physical
laptop journeys. See the [roadmap](docs/roadmap.md) for their dependencies and
open issues.

The [documentation index](docs/README.md) connects tutorials, architecture,
target guides, evidence, and historical records. You can learn the parts you
need as you go.
