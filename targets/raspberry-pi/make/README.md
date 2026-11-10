# Raspberry Pi make family

This project owns exact Raspberry Pi board descriptors plus the firmware acquisition, `config.txt`, FAT partition, SD-image verification, UART proof, and guarded removable-media FLASH mechanics used by `cargo xtask make conduitos ... --arch armv6`.

The make package keeps two intentions distinct. Raspberry Pi OS Bookworm 64-bit on the exact Pi 4 Model B rev 1.5 (4 GB) profile installs a reviewed aarch64 native package onto existing machinery. Bare-metal ConduitOS makes an SD image; the current Crèche path names only the ARMv6 Model B+ v1.2 substrate. The underlying builder also retains its exact original Zero v1 descriptor, but the Crèche does not infer that or any other Pi model from the B+ path.

The browser may download either reviewed release and bind it into a body spore. It does not thereby gain package-manager credentials or raw block-device authority. Package installation and removable-media writing require separate explicit local helpers, while physical boot and UART evidence remain separate proof classes. Shared ConduitOS runtime code remains in `targets/conduitos`; Pi machine make does not pass through Limine or EFI.


## Original Model B+ ConduitOS product

The B+ image selects `conduitos-armv6-bplus.host.conduit` through the normal
ConduitOS Make pipeline. Its machine adapters provide PL011 serial, VideoCore
firmware framebuffer scanout, and BCM2835 PWM PCM output on the 3.5 mm jack.
The shared ConduitOS Face renders the initial read-only Crèche. The existing
`plots/startup-chime/main.conduit` is checked, planned, and played through the
production scheduler and admitted native audio Host Call. The Body currently
has boot-scoped lifecycle evidence; cross-boot retention and local input are
not claimed by this profile.

```sh
cargo xtask make conduitos image --arch armv6 --board rpi-b-plus-v1.2 --locked
cargo xtask make conduitos flash --arch armv6 --board rpi-b-plus-v1.2 --device /dev/sda --confirm-device /dev/sda --locked
cargo xtask make conduitos rpi-physical-proof --product --board rpi-b-plus-v1.2 --serial-device /dev/ttyUSB0 --timeout-seconds 300 --locked
```

Replace device paths with the inspected local devices. FLASH erases the selected
whole removable device, refuses mounted partitions, and verifies the written
bytes. Start the UART collector before powering the Pi. Use a 3.3 V TTL adapter
at 115200 baud, with common ground and Pi GPIO14/TX connected to adapter RX.
The collector binds boot evidence to the flashed image and records framebuffer
Show and PCM completion separately from a person's confirmation of visible
HDMI output and audible sound. These are local physical checks, not release
acceptance or GPU acceleration evidence.

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
