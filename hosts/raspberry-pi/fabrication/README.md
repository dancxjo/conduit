# Raspberry Pi fabrication family

This project owns exact Raspberry Pi board descriptors plus the firmware acquisition, `config.txt`, FAT partition, SD-image verification, UART proof, and guarded removable-media FLASH mechanics used by `cargo xtask conduitos ... --arch armv6`.

The lightweight contribution in `fabrication-package` advertises only the Model B+ v1.2 and original Zero v1 targets already represented by the builder. Shared ConduitOS runtime code remains in `hosts/conduitos`; Pi machine fabrication does not pass through Limine or EFI.
