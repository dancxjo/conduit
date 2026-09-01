# Raspberry Pi fabrication family

This project owns exact Raspberry Pi board descriptors plus the firmware acquisition, `config.txt`, FAT partition, SD-image verification, UART proof, and guarded removable-media FLASH mechanics used by `cargo xtask conduitos ... --arch armv6`.

The fabrication package keeps two intentions distinct. Raspberry Pi OS Bookworm 64-bit on the exact Pi 4 Model B rev 1.5 (4 GB) profile installs a reviewed aarch64 native package onto existing machinery. Bare-metal ConduitOS fabricates an SD image; the current Crèche path names only the ARMv6 Model B+ v1.2 substrate. The underlying builder also retains its exact original Zero v1 descriptor, but the Crèche does not infer that or any other Pi model from the B+ path.

The browser may download either reviewed release and bind it into a Body spore. It does not thereby gain package-manager credentials or raw block-device authority. Package installation and removable-media writing require separate explicit local helpers, while physical boot and UART evidence remain separate proof classes. Shared ConduitOS runtime code remains in `targets/conduitos`; Pi machine fabrication does not pass through Limine or EFI.
