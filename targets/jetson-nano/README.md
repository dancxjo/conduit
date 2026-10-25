# Jetson Nano native media tooling

This directory supplies internal helpers for the ConduitOS P3450-0000 /
P3448-0000 port tracked by [#6478](https://github.com/dancxjo/conduit/issues/6478).
It does not itself establish a supported bootable product or physical proof.
The development entrance is `cargo +stable xtask make conduitos jetson-nano-image
--output /absolute/path/image.img.gz`. The Python entry points are internal
helpers, not the user flashing interface. The command builds the exact native
board binary from clean committed source, acquires/reuses the official donor
and includes NVIDIA license text.

## Firmware source and layout

The original 4 GB Nano requires NVIDIA firmware to initialize DRAM and hardware.
NVIDIA's [R32 partition contract](https://docs.nvidia.com/jetson/archives/l4t-archived/l4t-3275/Tegra%20Linux%20Driver%20Package%20Development%20Guide/part_config.html)
places required boot-stage firmware on the P3448-0000 SD card as well as in
on-board memory. Creating a single new filesystem partition would discard those
prerequisites. This helper retains the complete official card's partition table
and every byte outside its APP partition. It does not write QSPI or any device.

`make/donor.py` uses the official [JetPack 4.6.1 4 GB Nano SD archive](https://developer.download.nvidia.com/embedded/L4T/r32_Release_v7.1/JP_4.6.1_b110_SD_Card/Jeston_Nano/jetson-nano-jp461-sd-card-image.zip).
The archive is 6,561,246,401 bytes and contains `sd-blob-b01.img`, a
13,816,037,376-byte disk image. Its APP partition spans LBA 28672 through
26980351. Required TBC, RP1, EBT, WB0, BPF, BPF-DTB, FX, TOS, DTB, LNX, EKS, BMP
and RP4 partitions precede APP. These details were checked against the actual
archive's ZIP64 directory, local header and decompressed primary GPT.

The importer streams the entire archive, validates its ZIP64 entry identity,
lengths, full uncompressed CRC32 and both GPT copies, and records SHA-256 of the
original archive and disk stream. NVIDIA's published SHA-256 was not located;
the recorded digests are acquisition receipts, not independently published trust
anchors. Linux APP bytes are deliberately omitted. The resulting sparse donor
has the original logical image size and retains firmware, with zero holes
through APP; it is explicitly a scrubbed derivative, not the original archive.
Acquisition needs roughly 6.56 GB of network transfer but only the retained
firmware and filesystem metadata in disk allocation, rather than a full ZIP and
13.8 GB extraction. Never flash the scrubbed donor by itself.

The original board's current QSPI firmware remains a physical compatibility
condition. Keeping official SD firmware avoids inventing a replacement boot
chain; it cannot prove compatibility with an unknown installed QSPI version.

## Native media assembly

`make/media.py` accepts the donor, a freestanding ARM64 Image with a nonzero
effective image size, an exact source commit, and the donor release identity.
The optional `--firmware-license` input is a nonempty regular file bounded to
2 MiB; final deliverables supply NVIDIA's actual license text. It is copied into
APP as `/boot/conduitos/NVIDIA-LICENSE.txt` and beside the image, with its digest
in the manifest. It validates both GPT CRCs and partition extents, creates a fresh ext4 APP with
conservative features for the older U-Boot reader, then reads the native Image
back and checks its digest. It copies all donor bytes outside APP unchanged.
No Linux executable, initrd or root filesystem is included in the new APP.

NVIDIA documents [extlinux boot and firmware DTB retention](https://docs.nvidia.com/jetson/archives/l4t-archived/l4t-3275/Tegra%20Linux%20Driver%20Package%20Development%20Guide/uboot_guide.html).
The generated `/boot/extlinux/extlinux.conf` loads `/boot/conduitos/Image` and
omits both `FDT` and `FDTOVERLAYS`, retaining the DTB supplied by the NVIDIA boot
chain. The native handoff parser must validate that actual DTB; packaging does
not establish parser compatibility, display availability or a correct boot.

Assembly emits a compressed image, SHA-256 sidecar and JSON manifest. The
manifest also lives inside APP and binds the donor digest, source commit,
Image digest and partition boundary. Output paths must be new regular files;
there are no implicit block-device writes. NVIDIA firmware license and notices
must accompany any redistributed deliverable; producing the native APP does
not eliminate those obligations.

## Integration handoff

The xtask owner should add a Nano build/image entrance that:

1. Builds the actual native board binary and emits its ARM64 Image plus source
   artifact manifest. A test fixture or AArch64 virt image is insufficient.
2. Acquires or validates a cached official donor through `make/donor.py` and
   retains its acquisition receipt. Do not silently treat an arbitrary cached
   image as that donor.
3. Invokes `make/media.py` with donor path, native Image, output `.img.gz`, exact
   source SHA, `JetPack 4.6.1 / L4T R32.7.1` donor identity, and the official
   [NVIDIA license text](https://developer.download.nvidia.com/embedded/L4T/r32_Release_v7.1/T186/Tegra_Software_License_Agreement-Tegra-Linux.txt)
   supplied through `--firmware-license`.
4. Retains required NVIDIA notices, checks the generated manifest and checksum,
   and publishes the image with explicit experimental/physical proof status.
5. Provides local flashing instructions for a microSD card on the user's own
   machine, requiring no UART. Successful HDMI and ordinary native execution
   still require physical verification.

Host dependencies are Python 3, `mkfs.ext4`, `e2fsck` and `debugfs`. Five focused
conformance tests in `make/media_tests.py` and `make/donor_tests.py` exercise
corrupt GPT refusal, native Image header refusal, real ext4 payload readback,
outside-APP firmware preservation, scrub boundaries, and complete-stream CRC
refusal before output publication. They do not execute NVIDIA firmware or the
native Image.
