# RISC-V64 virt USB feasibility

For #5579, the verified candidate uses QEMU 10.2.1, package
`1:10.2.1+ds-1ubuntu3.2`, machine `virt`, one vCPU and 512 MiB RAM.
OpenSBI package `1.8.1-1` supplies the explicit RISC-V64 `fw_dynamic.bin` BIOS.
EDK2 package `2025.11-3ubuntu7.3` supplies 32 MiB `RISCV_VIRT_CODE.fd` and
fresh copied `RISCV_VIRT_VARS.fd` pflash images. Exact hashes, command, PCI
observation and generated device-tree properties are retained in
[the receipt](../../proof/usb/riscv64-feasibility.json).

## Firmware and actual resources

The probe selects `qemu-xhci`, one USB2 port, zero USB3 ports, MSI/MSI-X disabled,
and an emulated USB mouse. Eight seconds of execution reach OpenSBI and EDK2's
shell. QMP reports `00:01.0`, PCI `1b36:000d`, subsystem `1af4:1100`, BAR0 at
`0x400000000`, extent 16384 bytes, non-prefetchable 64-bit memory encoding.
The selected BAR lies above 4 GiB and must never be truncated to a 32-bit address.
A preceding `-bios none` attempt with S-mode EDK2 pflash alone produced neither
boot output nor assigned BAR; that combination is not supported by this receipt.

Generated device-tree ECAM spans `0x30000000..0x3fffffff`. PCI memory windows
include `0x40000000..0x7fffffff` and `0x400000000..0x7ffffffff`.
The PCI node declares `dma-coherent`. Slot 1/pin A maps to PLIC source 33,
parent phandle 3. The PLIC is at `0x0c000000`, extent `0x600000`, with machine
and supervisor external-interrupt contexts for the single hart. QMP's PCI
interrupt-line byte 255 is not the guest interrupt route. The actual Root must
configure the selected PLIC context, priority/enable/claim/complete sequence and
prove delivery. The AIA/MSI and IOMMU variants have no execution proof here.

## Root/Boot/Plan/Play boundary

Root must retain exact controller registers and attachment slot/generation,
PCI memory/bus-master initialization, finite command/event/transfer rings and DMA
storage in possessed guest RAM. Device-tree coherency does not grant DMA memory
or replace required device-ordering and completion fences. Rings cannot be reused
on revocation, timeout or provider loss before acknowledged hardware quiescence.

The existing `usb_base/control_factory.rs` checks selected Base, capability,
implementation, artifact, typed ports and semantic laws. `control_owner.rs` binds
exact resource possession, one in-flight operation and the separately granted
`conduitos.authority/usb-control@1` authority. The serialized envelope permits
4096 bytes; the current native attachment data ceiling is 256 bytes. Bulk endpoint
read/write offers have their own bounded contracts. A descriptive offer cannot
create an attachment or initialized native provider. Host/Boot identify observed
resources; Plan selects exact resources/authority; Play retains leases and their
retirement. Device-class policy remains with its existing owners.

No ordinary RISC-V64 ConduitOS USB provider is admitted by this probe. Follow-on
#5741 requires actual Root controller initialization, safe DMA/ring possession,
IRQ delivery, and bounded control/bulk completion before executable offers.
USB3, MSI, isochronous traffic, other machines and physical devices cannot inherit
this evidence. Compiler success, firmware PCI enumeration and assigned resources
are distinct from ConduitOS USB transfers; only the firmware/resource class is
proved here.
