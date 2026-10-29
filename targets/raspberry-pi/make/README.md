# Raspberry Pi make family

This project owns exact Raspberry Pi board descriptors plus the firmware acquisition, `config.txt`, FAT partition, SD-image verification, UART proof, and guarded removable-media FLASH mechanics used by `cargo xtask make conduitos ... --arch armv6`.

The make package keeps two intentions distinct. Raspberry Pi OS Bookworm 64-bit on the exact Pi 4 Model B rev 1.5 (4 GB) profile installs a reviewed aarch64 native package onto existing machinery. Bare-metal ConduitOS makes an SD image; the current Crèche path names only the ARMv6 Model B+ v1.2 substrate. The underlying builder also retains its exact original Zero v1 descriptor, but the Crèche does not infer that or any other Pi model from the B+ path.

The browser may download either reviewed release and bind it into a body spore. It does not thereby gain package-manager credentials or raw block-device authority. Package installation and removable-media writing require separate explicit local helpers, while physical boot and UART evidence remain separate proof classes. Shared ConduitOS runtime code remains in `targets/conduitos`; Pi machine make does not pass through Limine or EFI.


## Mini PiTFT display diagnostic (in development)

The opt-in profile selects the original BCM2835 Model B+ v1.2 with the
Adafruit 1.3-inch 240×240 Mini PiTFT and two buttons. Build its whole-card
image and run its software conformance through the supported entrances:

```sh
cargo xtask check minipitft
cargo xtask make conduitos rpi-minipitft-image --locked
```

The image and identity/digest record are written under
`target/conduitos/armv6/rpi-b-plus-minipitft/`. The current diagnostic shows
text and four corner markers, and explicitly reports “Buttons pending”.
The checked button polling composition and native contact preparation do
not yet provide normal Face interaction in this image. Software conformance
uses scripted hardware; it does not establish physical boot or panel output.

Transfer the image to the machine with the SD writer and write the whole
`.img` to the selected removable card. The onboard ACT LED pulses once every
two seconds after successful frame delivery. On a diagnostic refusal it
repeats the last stage count: 1 arena, 2 checked Source, 3 native admission
and planning, 4 CPU frame, 5 display protocol, 6 completion. Firmware failures
before the diagnostic starts can have different LED behavior. Physical
display and independent/both-button acceptance remain tracked in #6513.

## Raspberry Pi OS native release prerequisites

The reviewed Raspberry Pi OS package is cross-compiled from a Linux development
host as part of the Linux host release. The checkout pins Rust 1.98.1 and the
`aarch64-unknown-linux-gnu` Rust target in `rust-toolchain.toml`, and pins
`aarch64-linux-gnu-gcc` as that target's linker in `.cargo/config.toml`.

On Debian or Ubuntu, prepare the remaining development-host packages once:

```sh
cargo xtask setup linux-release
```

Verify the
environment without building the release:

```sh
cargo xtask doctor linux-release
```

Then compile and seal the Linux and Raspberry Pi OS host releases:

```sh
cargo xtask make host release --platform linux --output target/creche-host-releases
```

The setup command installs the native build toolchain and CMake, both GNU AArch64
C and C++ cross compilers, the cross libc development files, and the headless X
runner used by packaged native journey checks. host-device audio support inside
Tongues is not a prerequisite for this cross build; Conduit consumes Tongues'
device-free DSP/TTS path for this release.
