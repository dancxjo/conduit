# Try Conduit

Start with the [live Tour](https://dancxjo.github.io/conduit/tour/) or the
[ConduitOS visual journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)
to see the project before building it. The journey shows a bounded sequence of
checkpoints from a real QEMU session, including Body lifecycle, a hot-plugged Line, every
Tour page and runnable exercise, foreground Form switching, and Patchbay.

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
compositor, keyboard, and pointer paths. Birth a Body from the Crèche, then use
the resident Tour and Patchbay: they are Forms in the same Body-wide Plan and
Play, not special programs outside Conduit.

In the native Tour, `F3`/`F4` move between stages, `F5`/`F6` move between
chapters, `F10` runs the current exercise, and `F11` opens Patchbay. Patchbay
shows the active Forms on the present Body and their exact current identities.
The browser, Linux, Windows, and ConduitOS presentations consume the same
portable Tour application state and action identities; each Host supplies its
own presentation implementation.

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

## Hear and answer with local providers

With already-local Whisper, Ollama, and Piper providers, retain one addressed
recording and its synthesized answer as bounded journey evidence:

```bash
cargo xtask host journey-hears-speaks \
  --whisper-executable /path/to/whisper-cli \
  --whisper-model /path/to/ggml-model.bin \
  --pcm-s16le-16000-mono /path/to/question.pcm \
  --ollama-model model:tag --admitted-memory-mib 8192 \
  --piper-executable /path/to/piper \
  --piper-model /path/to/voice.onnx \
  --piper-config /path/to/voice.onnx.json
```

The command refuses an existing output directory and removes its newly created
directory if any provider or proof step fails. On success,
`target/journeys/hears-speaks/` contains the raw and WAV input, recognition and
response JSON, the exact same-Play receipt, the synthesized output WAV, and a
digest-bound evidence manifest. This is hosted-provider evidence from one
recorded clip; it is not live-microphone, browser, emulator, or physical proof.

## Manifest one Presentation twice

After installing the pinned browser proof dependencies, retain the same
deterministic front-door Presentation through the native software renderer and
pinned Chromium DOM/SVG renderer:

```bash
npm --prefix proof/browser ci --ignore-scripts --prefer-offline
cargo xtask evidence one-form-two-faces
```

The command refuses an existing output directory. On success,
`target/journeys/one-form-two-faces/` contains native and browser PNGs, their
distinct renderer receipts, and one four-output digest-bound manifest. The
verifier requires a shared Presentation identity and revision but deliberately
does not require pixel equality. The native frame proves the repository's
software rendering path, not a physical display or human viewing.

## Watch a Little Life evolve

Retain the deterministic Orbium seed and three checkpoints from one ordinary
32-step Lenia Plan/Play:

```bash
cargo xtask evidence little-life
```

The exact six-output manifest under `target/journeys/little-life/` contains
`t000.png`, `t001.png`, `t008.png`, `t032.png`, the complete scalar-field
terminal transcript, and the neutral execution report. Generation zero is the
deterministic semantic seed lowered to a gray8 bitmap. Generations 1, 8, and 32
are derived from cells emitted by the installed std scalar-field terminal
presentation. These files are not native graphical renderer, physical-display,
or human-perception proof.

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
