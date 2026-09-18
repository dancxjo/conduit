# Raspberry Pi fabrication family

This project owns exact Raspberry Pi board descriptors plus the firmware acquisition, `config.txt`, FAT partition, SD-image verification, UART proof, and guarded removable-media FLASH mechanics used by `cargo xtask conduitos ... --arch armv6`.

The fabrication package keeps two intentions distinct. Raspberry Pi OS Bookworm 64-bit on the exact Pi 4 Model B rev 1.5 (4 GB) profile installs a reviewed aarch64 native package onto existing machinery. Bare-metal ConduitOS fabricates an SD image; the current Crèche path names only the ARMv6 Model B+ v1.2 substrate. The underlying builder also retains its exact original Zero v1 descriptor, but the Crèche does not infer that or any other Pi model from the B+ path.

The browser may download either reviewed release and bind it into a Body spore. It does not thereby gain package-manager credentials or raw block-device authority. Package installation and removable-media writing require separate explicit local helpers, while physical boot and UART evidence remain separate proof classes. Shared ConduitOS runtime code remains in `targets/conduitos`; Pi machine fabrication does not pass through Limine or EFI.


## Raspberry Pi OS native release prerequisites

The reviewed Raspberry Pi OS package is cross-compiled from a Linux development
host as part of the Linux Host release. The checkout pins Rust 1.98.1 and the
`aarch64-unknown-linux-gnu` Rust target in `rust-toolchain.toml`, and pins
`aarch64-linux-gnu-gcc` as that target's linker in `.cargo/config.toml`.

On Debian or Ubuntu, prepare the remaining development-host packages once:

```sh
cargo xtask setup linux-release
```

`just setup` is the thin convenience entrance to the same command. Verify the
environment without building the release:

```sh
cargo xtask doctor linux-release
```

Then compile and seal the Linux and Raspberry Pi OS Host releases:

```sh
cargo xtask host release --platform linux --output target/creche-host-releases
```

The setup command installs the GNU AArch64 cross toolchain and the headless X
runner used by packaged native journey checks. Host-device audio support inside
Tongues is not a prerequisite for this cross build; Conduit consumes Tongues'
device-free DSP/TTS path for this release.
