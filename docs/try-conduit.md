# Try Conduit

Conduit has reached the point where several of its architectural claims can be
experienced directly, not only read in tests or design documents.

This page is a tour of runnable programs and proofs in the current repository.
It deliberately distinguishes ordinary demonstrations, automated hosted proofs,
and hardware-gated physical proofs. `STATUS.md` remains the authoritative claim
boundary when those categories differ.

## Prerequisites

Start from the repository root with a recent Rust toolchain. The convenience
commands below use `just`; repository orchestration itself lives in `xtask`.

Inspect the current host before trying platform-specific programs:

```bash
just doctor
```

For browser work:

```bash
just doctor browser
```

For Pico W work:

```bash
just doctor pico
```

## 1. Run one form on the native std host

The smallest useful program is the unchanged Signal form:

```bash
just demo-std
```

This parses and checks `examples/signal-demo.form`, plans it onto the actual
`StdHost`, lowers the admitted fragment into `conduit-kernel`, and executes the
pulse source and stdout presentation implementation.

The terminal output names the host and boot, immutable plan identity, selected
capabilities and implementations, exact connection, sixteen Signal values and
presentation receipts, and terminal completion.

The form itself says only:

```text
form 0

signal-demo {
    pulse: flow/pulse
    show: presentation/show

    pulse.count = 16
    pulse.period-ms = 250
    pulse.initial = false

    pulse > show
}
```

It does not say `stdout`, `DOM`, `GPIO`, `USB`, or `WebSocket`. Those are host and
plan facts.

A larger all-local fan-out is available with:

```bash
just demo-triple-local
```

## 2. See an actual browser host

The most immediate visual demonstration is the interactive distributed toggle:

```bash
just toggle
```

This is not the browser simulation. The command:

1. builds the real Rust browser runtime for `wasm32-unknown-unknown`;
2. starts the repository's static HTTP server;
3. starts the native std-side distributed toggle source;
4. creates the bounded loopback WebSocket Line; and
5. prints an HTTP URL for a real browser page.

Open the exact URL printed by the command in a normal browser. It looks like:

```text
http://127.0.0.1:4174/hosts/browser/distributed-toggle.test.html?ws=...
```

Then press **Enter in the terminal**. Each admitted Play start runs through the
std kernel, toggles state, crosses the exact `SessionMachine` over WebSocket,
enters the Rust/WASM browser kernel, and completes through the thin DOM adapter.
The page appends an `<output>` receipt for each presentation.

There are sixteen planned Play starts. The visible Signal levels alternate:

```text
sequence=0 level=true
sequence=1 level=false
sequence=2 level=true
...
```

At terminal completion the page reports the receipt count and whether admitted
capacity remained stable. In browser developer tools, the structured result is
also available as:

```js
globalThis.__distributedToggleProof
```

For non-interactive accepted browser proofs, use:

```bash
just prove-std-browser-s4
just prove-std-browser-toggle
```

These run deterministic hosted proof suites rather than asking an operator to
press keys.

## 3. Inspect a real execution with Observatory

A native run can emit a neutral runtime-report artifact:

```bash
cargo run -p conduit -- \
  examples/signal-demo.form \
  --placements examples/std-local.placements \
  --report /tmp/conduit-run.json
```

Render that artifact without controlling the runtime:

```bash
cargo run -p conduit -- \
  observatory-report /tmp/conduit-run.json
```

The resulting report exposes the machine Conduit actually realized. Among other
things it lists:

- host and boot identity;
- capability offers and resource pools;
- the exact plan and fragment;
- selected placements and implementations;
- connection base and queue bounds;
- the active Play and terminal state;
- presentation Sign; and
- bounded Sign retention and visible gaps.

A current std reference-host report contains the separately owned Signal pair
and nine installed `conduit.std` operation offers. The standard nucleus is:

```text
time/tick
time/every
presentation/tick
text/literal
text/upper
text/join
presentation/text
state/count
presentation/count
```

The `time/tick` row was the first rearticulated installed `conduit.std` kind:
`conduit.std/time-tick@2` over `value/tick@1`, implemented by the std host through
`conduit-kernel`. Every row above is a real current host offer; the exact
contracts, limits, implementations, and platform stop lines are recorded in
[`architecture/std-catalog.md`](architecture/std-catalog.md). Programs 1–4 in
[`try-forms.md`](try-forms.md) exercise the text/time/state nucleus.

## 4. Drive a physical Pico W over USB CDC

To run the physical std-to-Pico USB proof, build and flash the `usb-remote` firmware image to a Pico W in BOOTSEL mode:

On a desktop-free Linux host, install the narrow BOOTSEL mount helper once:

```bash
sudo scripts/install-pico-headless-flash.sh
```

The installer permits members of `plugdev` to invoke only the fixed root-owned
mount and cleanup operations. The helper discovers exactly one removable USB
FAT volume labeled `RPI-RP2` or `BOOTSEL`, mounts it beneath
`/run/conduit-pico-bootsel` with `nosuid,nodev,noexec`, then unmounts that exact
fixed path after the synchronized copy. It accepts no caller-controlled device
or mount path. The ordinary flash command uses it automatically; no desktop
automounter is required.

```bash
just pico-build-remote
just pico-flash-remote
```

Or using `xtask` directly:

```bash
cargo xtask pico build --usb-remote
cargo xtask pico flash --usb-remote
```

Then start the interactive std-to-Pico session:

```bash
just prove-std-pico-usb --interactive
```

or via `xtask`:

```bash
cargo xtask prove std-pico-usb --interactive
```

The proof uses the real dual-CDC firmware:

- CDC 0 carries bounded Conduit session frames;
- CDC 1 carries physical Sign receipts.

Before admitting the graph session, the operator tooling verifies the physical
CDC path, the running Pico boot identity, the exact generated image/plan
relationship, and the reciprocal `Hello` / `Ready` session lifecycle.

In interactive mode, key presses release the planned kernel Signal sequence and
the Pico's CYW43 LED manifests the corresponding physical state. This is a
hardware-gated physical proof, not something ordinary CI can reproduce without
the board.

The non-interactive exact Line proof is:

```bash
just prove-std-pico-usb
```

or:

```bash
cargo xtask prove std-pico-usb
```

## 5. Run the final std + browser + Pico proof

The accepted S4 demonstration plans the unchanged `examples/triple-signal.form`
once and executes one kernel-owned source fan-out to:

- native stdout;
- browser DOM over bounded WebSocket; and
- the physical Pico W LED over bounded USB CDC.

Because this is an attached-hardware proof, it is intentionally not part of the
ordinary no-hardware browser suite.

Build and flash the exact Pico image:

```bash
just pico-build --triple-remote
just pico-flash --triple-remote
```

or via `xtask`:

```bash
cargo xtask pico build --triple-remote
cargo xtask pico flash --triple-remote
```

Then run the hardware-gated browser/physical proof:

```bash
CONDUIT_THREE_HOST=1 \
  npx playwright test \
  --config hosts/browser/playwright.config.mjs \
  hosts/browser/triple-signal.spec.mjs
```

The accepted success vector records sixteen matching ordered stdout, DOM, and
physical LED receipts. The corresponding broken-browser-link physical negative
is available with:

```bash
CONDUIT_THREE_HOST=1 \
CONDUIT_THREE_HOST_FAILURE=1 \
  npx playwright test \
  --config hosts/browser/playwright.config.mjs \
  hosts/browser/triple-signal.spec.mjs
```

See issue #350 and `STATUS.md` for the exact accepted Sign boundary rather
than treating these commands alone as proof of a historical run.

## 6. What the platform can do now

The current executable substrate includes:

- native hosted execution through the bounded Conduit kernel;
- actual Rust/WASM browser execution and DOM presentation;
- actual physical RP2040/Pico W execution;
- bounded live WebSocket and USB CDC lines using the same exact remote-session
  semantics;
- one exact three-host form spanning stdout, DOM, and physical LED;
- boot-scoped portable planner capability offers on std and browser hosts;
- a read-only Observatory report path over neutral runtime facts; and
- the first installed `conduit.std` operation, `time/tick@2`.

That is more capability than the current general-purpose command-line UX exposes
comfortably. In particular, not every installed standard kind yet has a polished
example form or ordinary CLI tour. That is a usability/catalog frontier, not a
reason to blur the difference between installed code and user-facing programs.

## Proof classes matter

These commands intentionally represent different levels of Sign:

- `just demo-std` and `just demo-triple-local`: executable native programs;
- `just toggle`: interactive hosted browser demonstration;
- `just prove-std-browser-*`: deterministic actual-browser proofs;
- `observatory-report`: read-only inspection of a recorded runtime artifact;
- `cargo xtask prove std-pico-usb`: attached-board physical transport proof;
- `CONDUIT_THREE_HOST=1 ... triple-signal.spec.mjs`: attached-board final
  three-host proof.

Compilation is not execution. Simulation is not an actual browser. Firmware
build is not board execution. A live Line is not automatically a general
network stack. Conduit keeps those distinctions explicit because they are part
of the architecture, not documentation caveats.

For the precise accepted claim boundary, read [`../STATUS.md`](../STATUS.md).
For the implementation sequence and current frontier, read roadmap issue #361.
