# ConduitOS

**[See the current ConduitOS visual journey →](https://dancxjo.github.io/conduit/current/conduitos/x86_64/)**

The narrated gallery shows the actual QEMU graphical product: Tour, Forms,
Patchbay inspection, pointer interaction, and USB Line delivery and loss. Its
checkpoints link to their provenance. This is freestanding emulator
evidence; physical laptop acceptance is separate.

ConduitOS is Conduit's freestanding Host. The x86_64 PC image is a graphical
`no_std`, `no_main` live system. IA-32 PC and AArch64, RISC-V64, and LoongArch64
virt images offer long-lived serial experiences. Each realizes ordinary Forms
through the production `conduit-kernel`, with a checked PROFILE and finite
Host resources.

## Try the graphical system

From a repository checkout:

```sh
cargo xtask conduitos live x86_64 --locked
cargo xtask conduitos live-boot x86_64 --locked
```

The first command builds `target/conduitos/live/x86_64-pc/conduitos-x86_64.iso`;
the second boots that artifact in QEMU. The build needs the Rust toolchain,
`curl`, `make`, `tar`, and `xorriso`; boot also needs `qemu-system-x86_64`.
The builder acquires pinned boot dependencies. An interactive boot is useful
for exploration; the proof commands below validate specific behaviors.

## First arrival on x86_64

The graphical image opens the Crèche before a Body exists. Edit the suggested
name, choose a naming tradition, and select the initial Forms. Enter births the
named Body and requests Wake, exact planning, and Play in order. Each successful
stage has its own lifecycle evidence; a refusal remains visible at the stage
that could not finish. F2 suggests another name in the Crèche and opens exact
details after birth. F9 visits the existing Tour.

This development slice offers the native **Keyboard canvas** Form. One Play
keeps listening and shows uppercase output until F8 explicitly stops it; F7
then Lulls the Body. The screen retains a fixed window of recent output and
reports when older output or kernel evidence leaves its bounded history.
Resident Tour and Patchbay Forms and durable Body restoration across a new
Boot remain open work. The startup sound and reusable first-wake behavior are
tracked in [#3152](https://github.com/dancxjo/conduit/issues/3152).

## Available images

Paths below are relative to `target/conduitos/`.

| Host | Artifact | Experience |
| --- | --- | --- |
| `conduitos/x86_64/pc` | `live/x86_64-pc/conduitos-x86_64.iso` | Graphical front door, compositor, keyboard and pointer |
| `conduitos/ia32/pc` | `live/ia32-pc/conduitos-ia32.iso` | Serial product |
| `conduitos/aarch64/virt` | `live/aarch64-virt/conduitos-aarch64.iso` | Serial product |
| `conduitos/riscv64/virt` | `live/riscv64-virt/conduitos-riscv64.iso` | Serial product |
| `conduitos/loongarch64/virt` | `live/loongarch64-virt/conduitos-loongarch64.iso` | Serial product |

`cargo xtask conduitos live-matrix` reports current formats, emulator profiles,
and exclusions. Raspberry Pi and Orange Pi image fabrication has separate
board contracts; presence of an image does not establish a usable physical
Host. See [Raspberry Pi](../raspberry-pi/fabrication/README.md) and
[Orange Pi](../orange-pi/README.md).

## Reproduce and inspect the evidence

| Command | What it checks |
| --- | --- |
| `cargo xtask conduitos journey-proof` | Graphical Body/Wake/Plan/Play journey, pointer actions, USB Line state, and correlated screenshots |
| `cargo xtask conduitos front-door-proof` | The normal image's initial surface and long-lived interaction |
| `cargo xtask conduitos prove --arch x86-64 --locked` | Architecture appliance, image reproducibility, fresh boots, kernel execution, and Observatory evidence |
| `cargo xtask conduitos architecture-matrix --locked` | Architecture backends and their earned proof rungs |
| `cargo xtask conduitos product-readiness-matrix` | Product readiness independently of architecture bring-up |
| `cargo xtask conduitos std-gap` | Portable catalog coverage and remaining ConduitOS implementation gaps |

The journey writes PNGs and `manifest.json` under
`target/conduitos/x86_64/journey-frames/`. The [visual evidence guide](../../docs/visual-evidence.md)
explains publication and how images correlate with semantic assertions.
USB Line attachment and delivery in that journey do not imply Body membership;
the records explicitly retain `membership: not-requested`.

For device work, `cargo xtask conduitos --help` lists focused xHCI, USB,
HID, keyboard/text, hotplug, rescue, and sound proofs. Each owns a smaller
contract: controller readiness, device enumeration, HID reports, and a portable
keyboard offer are different steps. These tests use real emulated device
paths and retain failures and identity changes as distinct results.

`cargo xtask conduitos keyboard-repeat-proof --locked` boots the normal image,
births a Body and sends 320 keyboard transitions through one Play before
Stop and Lull. It checks retained identities and the bounded recent-output
window, and retains its receipt and screenshots under
`target/conduitos/x86_64/keyboard-repeat-*`. The native USB keyboard reuses
two report buffers and a fixed 64-entry transfer ring across session input;
one ring entry links back with the xHCI cycle toggle. This is emulator proof,
not physical keyboard qualification.
## Headless startup boundary

The x86 headless profile excludes the native graphical Presenter, compositor,
and display resources. Build and inspect its exact final artifact with:

```sh
cargo xtask host build targets/conduitos/profiles/conduitos-headless.profile.json --output target/headless-proof
cargo xtask conduitos headless-proof target/headless-proof
```

The artifact boots without a framebuffer or USB input device. It validates
its fabricated inventory and finite arena, derives fresh Host/Boot identities,
and reports `headless-workload-entry-unavailable`. This is an explicit
unsupported workload entry: no providers are initialized, no offer is published,
and no Body, Plan, or Play is invented. Compiled implementation inventory is
reported separately. The boot stops with failure status after that receipt;
it does not fall back to the graphical demonstration or claim a ready workload.

The proof verifies artifact digests and excluded graphical symbols, then boots
the same image twice in QEMU and checks exact provenance, fresh identities,
zero allocation, and the expected unsupported disposition. The retained
`headless-proof.json`, two serial logs, and ELF symbol inventory live in the
supplied output directory. Passing this proof establishes that refusal contract,
not working headless Form execution or physical hardware acceptance. The
ordinary graphical journey remains `cargo xtask conduitos journey-proof`.

## Where to contribute

- `src/` owns machine adapters, Host composition, and kernel integration.
  Limine-specific types stay in `src/boot/limine.rs`; other code consumes
  boot-neutral observations.
- `firmware/` owns product linker scripts and boot configuration.
- `fabrication/` owns target descriptors; `fabrication/xtask/` builds images
  and runs repository proofs.
- `proof/appliances/` owns architecture bring-up programs. They are separate
  from the normal live product images.

The scheduler remains cooperative. Two admitted execution regions can make
logical progress through one kernel; this does not establish SMP, physical
parallelism, preemption, or hostile-code isolation. General physical laptop
storage, networking, audio, and input acceptance remain in the
[laptop workstream](https://github.com/dancxjo/conduit/issues/2300).
Native shell work includes [retained compositor surfaces](https://github.com/dancxjo/conduit/issues/3043),
[scrolling and clipping](https://github.com/dancxjo/conduit/issues/3047), and
[rendering polish](https://github.com/dancxjo/conduit/issues/3049).
Use [STATUS](../../STATUS.md) for recorded proof boundaries and the linked
issues for the current scope of each contribution.
