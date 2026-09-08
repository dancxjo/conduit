# Try Conduit

Start with the [live Tour](https://dancxjo.github.io/conduit/tour/) or the
[ConduitOS visual journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)
to see the project before building it. The journey shows seventeen checkpoints
from a real QEMU session, including Body lifecycle, a hot-plugged Line, Tour,
and Patchbay.

The commands below run from a checkout. You do not need an installed `conduit`
binary or `just`. See [contributor setup](../CONTRIBUTING.md) for prerequisites.
The first invocation compiles repository tooling; browser and ConduitOS builds
need additional tools and disk space.

## Run a first Form

```bash
cargo xtask host std
```

`cargo xtask doctor` is an optional diagnostic across targets; missing browser
proof or Pico tools do not block this hosted example.

This command runs [Hello](../forms/hello/main.conduit) through the native Host, planner,
and production execution kernel. Its authored meaning is simply:

```conduit
form hello {
    upper: text/upper
    show: presentation/text
    "Hello, world." > upper > show
}
```

Look for `HELLO, WORLD.` and the terminal execution result. The Form chooses
text operations; the Host supplies their implementations and the Plan records
that selection. Explore more examples in [Try Forms](try-forms.md).

## Explore the workbench and Tour

```bash
cargo xtask demo patchbay --on native
```

Patchbay opens a native window. Inspect a Form, then use the explicit lifecycle
and execution actions to move from description to running work. Opening a Form
alone does not start it. For the browser manifestation, install Node.js and npm,
then add the WASM target:

```bash
rustup target add wasm32-unknown-unknown
cargo xtask demo patchbay --on browser
```

To build and open the guided executable Tour locally:

```bash
cargo xtask demo tour
```

These commands build the required browser runtime and serve it locally. Keep
the terminal process running while using the page. A separate browser Host can
be launched with `cargo xtask host browser`; each launch has its own runtime
identity. The [Body lifecycle guide](self-hosted-biography.md) explains how Tour,
Crèche, and Patchbay relate.

## Boot ConduitOS

Install the build and emulator prerequisites listed in the
[ConduitOS guide](../targets/conduitos/README.md), then run:

```bash
cargo xtask conduitos live x86_64
cargo xtask conduitos live-boot x86_64
```

The first command builds `target/conduitos/live/x86_64-pc/conduitos-x86_64.iso`.
The second verifies and boots that artifact in visible QEMU. The graphical
session stays open until QEMU closes. It includes the normal front door,
compositor, keyboard, and pointer paths.

```bash
cargo xtask conduitos live-matrix
```

The matrix describes the supported media and their limits. Additional `ia32`,
`aarch64`, `riscv64`, and `loongarch64` product ISOs expose serial-console
sessions; they do not have the x86_64 graphical experience. Board images have
their own fabrication and physical-proof boundaries. See
[Host fabrication](host-fabrication.md) for those workflows.

To capture the reproducible x86_64 journey yourself:

```bash
cargo xtask conduitos journey-proof
```

This runs QEMU and writes the manifest and checkpoint PNGs under
`target/conduitos/x86_64/journey-frames/`. The
[visual evidence guide](visual-evidence.md) explains their provenance. A local
run does not publish or replace the accepted gallery.

## Explore distributed execution

```bash
cargo xtask demo toggle
```

Open the exact local URL printed by the command, then follow its terminal
prompts. An admitted Signal sequence crosses a bounded WebSocket Line from the
native kernel to a real Rust/WASM browser kernel. The browser shows the
presentation receipts. This is a specific working transport demonstration;
it does not establish arbitrary networking or automatic membership.

For automated browser checks, follow the pinned-tool setup in
[the browser proof guide](../proof/browser/README.md). `cargo xtask doctor browser`
checks those proof prerequisites; Playwright is not required just to open Tour.

```bash
cargo xtask prove patchbay-front-door
cargo xtask prove std-browser-toggle
```

## Work with a Pico W

Start with `cargo xtask doctor pico` and the
[RP2040 target guide](../targets/rp2040/README.md). With an attached Pico W in
BOOTSEL mode, the explicit USB firmware workflow is:

```bash
cargo xtask pico build --usb-remote
cargo xtask pico flash --usb-remote
cargo xtask prove std-pico-usb --interactive
```

Flashing changes the attached board. This proof uses separate CDC interfaces
for Conduit session frames and physical Sign receipts, and checks the running
boot and generated image/Plan relationship before admitting the session.

The larger `cargo xtask prove r1-hil` and
`cargo xtask prove body-membership-hil` workflows require their named hardware,
ports, and Wi-Fi credential references. Consult `--help` and the target guides
before running them. Firmware compilation alone cannot establish their result.

## Choose the next step

- [Try Forms](try-forms.md): inspect the reviewed programs and run conformance.
- [Current status](../STATUS.md): what works and the boundaries of its evidence.
- [Roadmap](roadmap.md): active work and remaining gaps.
- [Contributing](../CONTRIBUTING.md): choose and validate a useful change.
