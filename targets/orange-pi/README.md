# Orange Pi Host fabrication

This target family owns the exact bare-metal `conduitos/aarch64/orange-pi-5-rk3588s` substrate. It does not install or depend on Orange Pi OS, Debian, Ubuntu, Linux userspace, or any other hosted operating system.

`cargo xtask conduitos orange-pi5-image` compiles the AArch64 ConduitOS product Host, verifies the Linux-style AArch64 `Image` header used only as the U-Boot handoff format, fetches one digest-pinned RK3588S U-Boot image, and assembles a bounded microSD artifact. The image keeps the Rockchip loader at LBA 64 and a bootable FAT32 partition at LBA 32768 containing `Image` and `boot.scr`.

The machine implementation is board-specific: RK3588S UART2 at `0xfeb50000`, the architectural counter, and the exact Orange Pi 5 identity. Orange Pi 5B, Orange Pi 5 Plus, hosted-OS identities, and LoongArch identities refuse rather than aliasing this target. Image construction is deterministic artifact proof; physical flash, boot, UART observation, Host join, and membership remain unclaimed until separate board evidence exists.
