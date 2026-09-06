# Try Conduit

Conduit has reached the point where several of its architectural claims can be
experienced directly, not only read in tests or design documents.

This page is a tour of runnable programs and proofs in the current repository.
It deliberately distinguishes ordinary demonstrations, automated hosted proofs,
and hardware-gated physical proofs. `STATUS.md` remains the authoritative claim
boundary when those categories differ.

## Prerequisites

Start from the repository root with a recent Rust toolchain. Repository
orchestration, demonstrations, and proofs enter through `cargo xtask`.

Inspect the current host before trying platform-specific programs:

```bash
cargo xtask doctor
```

For browser work:

```bash
cargo xtask doctor browser
```

For Pico W work:

```bash
cargo xtask doctor pico
```

## First minute: enter the shared Patchbay

For an installed product, choose the Host that will manifest the same semantic
front door:

```bash
conduit patchbay --on native
conduit patchbay --on browser
```

From a checkout, use the thin friendly Patchbay recipe:

```bash
just patchbay  # cargo xtask demo patchbay --on native
```

The native command opens a real window. Both installed Patchbay renderers begin
with this Host and `BODY: NONE`, then
show bounded Body candidates and openable Forms without granting membership or
birthing anything. `OPEN` is inspection-only; explicit `JOIN` or `BIRTH`
establishes the current Body. The embodied view then exposes canonical Parts,
truthful Lines, the checked/expanded Form, and—after explicit actions—its
immutable Plan and active Play. Selection is semantic, can be cleared back to a
quiet WORLD view, and persists across current presentation revisions. DOM
nodes, window handles, layout, and pixels remain renderer-local.

Run the bounded acceptance path without package-level commands:

```bash
cargo xtask prove patchbay-front-door
```

That one proof runs the zero-Body semantic equivalence oracle, the native
manifestation, explicit OPEN and BIRTH through one pinned Chromium front
door, and the post-transition authenticated live browser Part regression. It
retains digest-bound JSON for exact Body, Wake, Form, Plan, Play, presentation,
Part, Host/Boot, Sign, and Manifestation outcomes.

## 1. Run one Form on the native std host

The smallest useful program is canonical `hello.conduit`:

```bash
CARGO_BUILD_JOBS=1 just run forms/hello/main.conduit
```

This parses and checks canonical source, expands it, plans it onto the actual
`StdHost`, lowers the admitted fragment into `conduit-kernel`, and executes the
text pipeline and presentation implementation.

The terminal output names the host and boot, immutable plan identity, selected
capabilities and implementations, exact connection, sixteen Signal values and
presentation receipts, and terminal completion.

The Form itself says only:

```text
form hello {
    upper: text/upper
    show: presentation/text
    "Hello, world." > upper > show
}
```

It does not say `stdout`, `DOM`, `GPIO`, `USB`, or `WebSocket`. Those are host and
plan facts.

Launch the ordinary std Host with the canonical Hello Form:

```bash
cargo xtask host
# equivalently: cargo xtask host std
```

The earlier `cargo xtask demo std` spelling remains a compatibility façade for
the same lifecycle.

## 2. Start an actual browser Host

Start one independent page/WASM Host with:

```bash
cargo xtask host browser
```

This is a Host lifecycle entrance, not a Patchbay or demo entrance. The command:

1. builds the real Rust browser runtime for `wasm32-unknown-unknown`;
2. binds one independent ephemeral IPv4 loopback server;
3. invokes the supported platform URL opener;
4. initializes a fresh HostId, BootId, and bounded WASM instance in the page.

Repeated `just browser` invocations create independent browser Hosts. Opening a
page establishes neither Body membership nor permission to use ambient browser
resources. The earlier `cargo xtask browser` spelling remains a compatibility
façade. The interactive distributed toggle remains available separately:

```bash
cargo xtask demo toggle
```

That demonstration creates the bounded loopback WebSocket Line and prints an
HTTP URL for a real browser page.

Open the exact URL printed by the command in a normal browser. It looks like:

```text
http://127.0.0.1:4174/proof/browser/distributed-toggle.test.html?ws=...
```

Then press **Enter in the terminal**. Each admitted Play start runs through the
std kernel, toggles state, crosses the exact `SessionMachine` over WebSocket,
enters the Rust/WASM browser kernel, and completes through the thin DOM adapter.
The page appends an `<output>` receipt for each presentation.

The canonical toggle emits its exact initial Boolean and then flips after each
of fifteen admitted terminal triggers, producing sixteen visible values:

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
cargo xtask prove std-browser-s4
cargo xtask prove std-browser-toggle
```

These run deterministic hosted proof suites rather than asking an operator to
press keys.

## 3. Inspect a real execution with Observatory

A native run can emit a neutral runtime-report artifact:

```bash
conduit run forms/hello/main.conduit \
  --report /tmp/conduit-run.json
```

Render that artifact without controlling the runtime:

```bash
conduit inspect runtime-report /tmp/conduit-run.json
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
[`architecture/semantic-catalog.md`](architecture/semantic-catalog.md). Programs 1–4 in
[`try-forms.md`](try-forms.md) exercise the text/time/state nucleus.

## 4. Drive a physical Pico W over USB CDC

To run the physical std-to-Pico USB proof, build and flash the `usb-remote` firmware image to a Pico W in BOOTSEL mode:

On a desktop-free Linux host, install the narrow BOOTSEL mount helper once:

```bash
sudo targets/rp2040/tools/install-pico-headless-flash.sh
```

The installer permits members of `plugdev` to invoke only the fixed root-owned
mount and cleanup operations. The helper discovers exactly one removable USB
FAT volume labeled `RPI-RP2` or `BOOTSEL`, mounts it beneath
`/run/conduit-pico-bootsel` with `nosuid,nodev,noexec`, then unmounts that exact
fixed path after the synchronized copy. It accepts no caller-controlled device
or mount path. The ordinary flash command uses it automatically; no desktop
automounter is required.

```bash
cargo xtask pico build --usb-remote
cargo xtask pico flash --usb-remote
```

Then start the interactive std-to-Pico session:

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
cargo xtask prove std-pico-usb
```

## 5. Run the final std + browser + Pico proof

The accepted S4 demonstration plans the unchanged `proof/fixtures/forms/triple-signal.conduit`
once and executes one kernel-owned source fan-out to:

- native stdout;
- browser DOM over bounded WebSocket; and
- the physical Pico W LED over bounded USB CDC.

Because this is an attached-hardware proof, it is intentionally not part of the
ordinary no-hardware browser suite.

Build and flash the exact Pico image:

```bash
cargo xtask pico build --triple-remote
cargo xtask pico flash --triple-remote
```

The current one-command hardware-gated browser/Pico recovery proof supersedes
the older direct test-runner recipe:

```bash
cargo xtask prove r1-hil --interactive \
  --ssid-env CONDUIT_WIFI_SSID \
  --credential-env CONDUIT_WIFI_PASSWORD
```

To additionally bind the live Body membership receipt to that exact physical
Body, Pico Boot, and Plan, use the final membership capstone entrance:

```bash
cargo xtask prove body-membership-hil --interactive \
  --link-port /dev/serial/by-id/<pico-cdc-0> \
  --sign-port /dev/serial/by-id/<pico-cdc-1> \
  --ssid-env CONDUIT_WIFI_SSID \
  --credential-env CONDUIT_WIFI_PASSWORD
```

This admits three independent browser Hosts and the already-provisioned Pico,
retains the membership receipt, and then rejects the production R1 HIL unless
the Body, physical Boot, and active Plan identities match exactly.

The accepted success vector records matching stdout, DOM, and physical LED
receipts across the R1 recovery lifecycle. See closed roadmap issue #361 and
`STATUS.md` for the exact accepted Sign boundary rather than treating this
command alone as proof of a historical run.

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

- `cargo xtask demo std` and `cargo xtask demo triple`: executable native programs;
- `cargo xtask browser`: browser Host lifecycle entrance;
- `cargo xtask demo toggle`: interactive hosted browser demonstration;
- `cargo xtask prove std-browser-*`: deterministic actual-browser proofs;
- `conduit inspect runtime-report`: read-only inspection of a recorded runtime artifact;
- `cargo xtask prove std-pico-usb`: attached-board physical transport proof;
- `cargo xtask prove r1-hil --interactive`: attached-board final R1 proof.
- `cargo xtask prove body-membership-hil --interactive`: attached-board Body
  membership and R1 Play identity-link proof.

Compilation is not execution. Simulation is not an actual browser. Firmware
build is not board execution. A live Line is not automatically a general
network stack. Conduit keeps those distinctions explicit because they are part
of the architecture, not documentation caveats.

For the precise accepted claim boundary, read [`../STATUS.md`](../STATUS.md).
For the completed R1 sequence and its evidence, read roadmap issue #361.
