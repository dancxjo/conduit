# Try Conduit

Start with the [body Workspace](https://dancxjo.github.io/conduit/workspace/) or the
[ConduitOS visual journey](https://dancxjo.github.io/conduit/journeys/current/conduitos/x86_64/)
to see the project before building it. The ten-step journey captures a real
QEMU session from zero-Body Crèche arrival through Birth, Play, Patchbay Face
inspection, its live diagram, and Stop.

## Create a Body in your browser

1. Open the [body Workspace](https://dancxjo.github.io/conduit/workspace/) in
   the browser profile you plan to use again. Under **A body of your own**,
   enter a **Friendly Body name** and keep the reviewed **Memory Lantern** Plot
   selected.
2. Click **Birth Body**. Your named Body appears as **Lulled**. Click
   **wake body** to start its Plots and see the state change to **Playing**.
3. Open **Memory Lantern** under **Your plots**, then reload the browser tab.
   The Workspace prepares again and returns to your named Body and selected
   Plot. It creates a fresh Boot and starts a new Play for the Body that was
   playing before reload.

This return uses storage in that browser profile. Joining from another Host
requires its own admission. Native disk persistence depends on the Host.

## Start Conduit on Linux

On a systemd-based Linux x86-64 machine, download `hosted-linux.tar.gz` from
the [latest release](https://github.com/dancxjo/conduit/releases/latest). Unpack
it, enter `hosted-linux/install-bundle/`, and run this as the account that will
own the Host:

```sh
sh ./install-linux-x86_64.sh
```

The installer verifies the release, starts the durable Host immediately, and
enables `conduit-host.service` for future user sessions. To have it start at
machine boot before anyone logs in, enable lingering for that account once
(administrator authorization may be required):

```sh
loginctl enable-linger "$(id -un)"
```

Lingering applies to the account's user services. After reboot, check the
service and the retained Host identity from the bundle directory:

```sh
systemctl --user is-active conduit-host.service
./conduit-linux-x86_64 host service status --state-dir "${XDG_STATE_HOME:-$HOME/.local/state}/conduit"
```

Keep the same state directory across launches. Use
`systemctl --user disable --now conduit-host.service` to stop automatic
startup while keeping that state. Starting the Host runs its background
service. Opening a full-screen Face still requires a configured display
and a Body that presents one.

### Try Ambient Home and Patchbay

A Linux download containing `linux-ambient.host.conduit` and
`linux-ambient.image.json` can run both residents from a new Crèche Body.
Follow the [installed Host setup](host-make.md#ambient-home-and-patchbay-on-installed-linux).
Keep the state directory to return to the same Body after restarting the
service.
The bundle also includes `linux-ambient-clock` and `linux-ambient-wallpaper`
profile and IMAGE pairs. Use the matching `.host.conduit` and `.image.json`
files in the same setup command, then select Clock or Wallpaper in Crèche.

### Use a dedicated Linux display at boot

From a separate terminal on a Linux console machine with systemd 256 or newer,
install a **native-display** release with the Host configuration and IMAGE made
for its display. For example, from an x86-64 release directory:

```sh
sh ./install-linux-x86_64.sh --direct-display-tty 1 \
  --display-configuration ./panel.host.conduit --display-image ./panel.image.json
```

On another supported Linux architecture, pass the same options to the release
executable's `host service install MANIFEST --state-dir STATE` command.
On a configured installation, stop the Host first and omit the two equipment
arguments to retain its existing selection. Run as the ordinary Host account.
The installer checks the selected scanout and installed release, requests
administrative authorization, then enables and starts the PAM/logind console
service. Use an unoccupied VT and release the display from any running desktop.
Check startup with `systemctl status conduit-direct-display.service`.

The installer retains the previous service states and rollback instructions in
a private review directory beneath the Host state. To preview the unit without
activation, use `conduit host service prepare-direct-display --help`.
The console runs SDL KMSDRM directly. Visible Body content requires the Host
to receive and present a current admitted Show.

## Try Conduit from source

The commands below run from a checkout. You do not need an installed `conduit`
binary or `just`. See [contributor setup](../CONTRIBUTING.md) for prerequisites.
The first invocation compiles repository tooling; browser and ConduitOS builds
need additional tools and disk space.

## Check that the pieces work together

```bash
cargo xtask integrate
```

This is the fast local developer truth loop. It exercises representative real
language, planning, kernel, std Host, Body lifecycle, multi-placement,
failure/recovery, and Patchbay paths. If the `wasm32-unknown-unknown` target is
installed, it also builds the production browser runtime; otherwise it reports
the exact prerequisite. It does not perform release promotion, generate Journey
or gallery evidence, run QEMU, or claim physical hardware proof.

## Run a first plot

```bash
cargo xtask make host std
```

`cargo xtask doctor` is an optional diagnostic across targets; missing browser
proof or Pico tools do not block this hosted example.

This command runs [Hello](../plots/hello/main.conduit) through the native host, planner,
and production execution kernel. Its authored meaning is simply:

```conduit
plot hello {
    upper: text/upper
    show: face/text("Message")
    "Hello, world." >> upper >> show
}.
```

Look for `HELLO, WORLD.` and the terminal execution result. The plot chooses
text operations; the host supplies their implementations and the plan records
that selection. Explore more examples in [Try plots](try-plots.md).

## Explore Patchbay

```bash
cargo xtask prove journey patchbay --on native
```

Patchbay opens a native window. Inspect a plot, then use the explicit lifecycle
and execution actions to move from description to running work. Opening a plot
alone does not start it. For the browser manifestation, install Node.js and npm,
then add the WASM target:

```bash
rustup target add wasm32-unknown-unknown
cargo xtask prove journey patchbay --on browser
```

This command builds the required browser runtime and serves it locally. Keep
the terminal process running while using the page.

To open your own editable source, add `--plot <LABEL> <PATH>` (repeatable):

```bash
cargo xtask prove journey patchbay --on browser --plot "My Plot" work/making.conduit
```

The file must already contain a checked Plot; `plot making { }` is an ordinary
empty starting point. Open it from Library, place Gears, select a Gear to configure
it, and drag between compatible Ports or use their connection controls in Inspect.
**Save Plot** writes the acknowledged source to that file. Relaunch with the same
command to reopen it. Layouts and notes are saved separately and do not edit source.
Birth, Wake, Plan and Play are explicit actions; editing alone starts no Body.

A separate browser host can
be launched with `cargo xtask make host browser`; each launch has its own
runtime identity. The [body lifecycle guide](self-hosted-biography.md) explains
how resident Plots, Crèche compatibility, and Patchbay relate.

## boot ConduitOS

Install the build and emulator prerequisites listed in the
[ConduitOS guide](../targets/conduitos/README.md), then run:

```bash
cargo xtask make conduitos live x86_64
cargo xtask make conduitos live-boot x86_64
```

The first command builds `target/conduitos/live/x86_64-pc/conduitos-x86_64.iso`.
The second verifies and boots that artifact in visible QEMU. The graphical
session stays open until QEMU closes. It includes the normal front door,
compositor, and keyboard path. At Crèche, name and birth a Body with a locally
available plot. Wake it with `F4`, plan with `F5`, start Play with `F6`, and
return Home with Escape. Open resident Patchbay, inspect its current semantic
Face with `F2`, show the live gear-and-cord diagram with `F3`, and request Stop
with `F8`. The [captured journey](https://dancxjo.github.io/conduit/journeys/current/conduitos/x86_64/)
shows each action and resulting screen.
The [recorded one-Body journey](https://dancxjo.github.io/conduit/journeys/current/one-body-five-masks/)
shows a live local encounter spanning a Linux owner, a browser, and a QEMU
guest across five Masks. See [STATUS](../STATUS.md) for the scope of that proof.
For an owner-issued invitation and a guest that truthfully waits to join, see
[ConduitOS spore provisioning](contributing/native-spore-provision.md).

```bash
cargo xtask make conduitos live-matrix
```

The matrix describes the supported media and their limits. Additional `ia32`,
`aarch64`, `riscv64`, and `loongarch64` product ISOs expose serial-console
sessions; they do not have the x86_64 graphical experience. Board images have
their own make and physical-proof boundaries. See
[host make](host-make.md) for those workflows.

To capture the reproducible x86_64 journey yourself:

```bash
cargo xtask make conduitos journey-proof
```

This runs QEMU and writes the manifest and checkpoint PNGs under
`target/conduitos/x86_64/journey-frames/`. The
[visual evidence guide](visual-evidence.md) explains their provenance. A local
run does not publish or replace the accepted gallery.

## Manifest one presentation twice

After installing the pinned browser proof dependencies, retain the same
deterministic front-door presentation through the native software renderer and
pinned Chromium DOM/SVG renderer:

```bash
npm --prefix proof/browser ci --ignore-scripts --prefer-offline
cargo xtask prove one-plot-two-fronts
```

The command refuses an existing output directory. On success,
`target/journeys/one-plot-two-fronts/` contains native and browser PNGs, their
distinct renderer receipts, and one four-output digest-bound manifest. The
verifier requires a shared presentation identity and revision but deliberately
does not require pixel equality. The native frame proves the repository's
software rendering path, not a physical display or human viewing.

## Watch a Little Life evolve

Retain the deterministic Orbium seed and three checkpoints from one ordinary
32-step Lenia plan/play:

```bash
cargo xtask prove little-life
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
cargo xtask prove journey toggle
```

Open the exact local URL printed by the command, then follow its terminal
prompts. An admitted Signal sequence crosses a bounded WebSocket line from the
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
cargo xtask make pico build --usb-remote
cargo xtask make pico flash --usb-remote
cargo xtask prove std-pico-usb --interactive
```

Flashing changes the attached board. This proof uses separate CDC interfaces
for Conduit session frames and physical sign receipts, and checks the running
boot and generated image/plan relationship before admitting the session.

The larger `cargo xtask prove r1-hil` and
`cargo xtask prove body-membership-hil` workflows require their named hardware,
ports, and Wi-Fi credential references. Consult `--help` and the target guides
before running them. Firmware compilation alone cannot establish their result.

## Choose the next step

- [Try plots](try-plots.md): inspect the reviewed programs and run conformance.
- [Current status](../STATUS.md): what works and the boundaries of its evidence.
- [Roadmap](roadmap.md): active work and remaining gaps.
- [Contributing](../CONTRIBUTING.md): choose and validate a useful change.
