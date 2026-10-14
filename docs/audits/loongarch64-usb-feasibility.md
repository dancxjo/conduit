# LoongArch64 virt USB feasibility

For #5580, the observed profile uses QEMU 10.2.1, package
`1:10.2.1+ds-1ubuntu3.2`, machine `virt`, one vCPU and 512 MiB RAM.
EDK2 package `2025.11-3ubuntu7.3` supplies the 16 MiB `QEMU_EFI.fd` code
and a fresh copied `QEMU_VARS.fd` flash store. Exact firmware digest, command,
PCI result and generated device-tree properties are retained in
[the receipt](../../proof/usb/loongarch64-feasibility.json).

## Observed resources

The selected `qemu-xhci` has one USB2 port, zero USB3 ports and MSI/MSI-X disabled,
with an emulated USB mouse attached. After eight seconds of firmware execution,
EDK2 reaches its boot manager. QMP reports controller `00:01.0`, PCI `1b36:000d`,
subsystem `1af4:1100`, a non-prefetchable 64-bit BAR0 at `0x40000000`, extent
16384 bytes. The paused machine previously reported an unassigned BAR.

Generated device-tree PCI ECAM is `0x20000000..0x27ffffff`, with MMIO window
`0x40000000..0x7fffffff`. It declares `dma-coherent`. Its interrupt-map entry
for slot 1/pin A routes to phandle `0x8003`, interrupt 17, flags 4. That parent
is the Loongson PCH PIC at `0x10000000`, extent 1024 bytes. QMP's PCI interrupt
line byte is 255; it is not the guest interrupt route. Root must use the actual
firmware/device-tree routing, configure the interrupt hierarchy and prove delivery.
These observations establish neither register possession nor DMA authorization.

## Bounded Root contract and unsupported requirements

Root must retain the exact controller BAR/slot, current Host/Boot and attachment
generation, PCI memory/bus-master state, finite command/event/transfer rings,
and bounded DMA storage within owned guest RAM. The firmware coherency declaration
supports this specific board profile; required device ordering, DMA ownership and
completion fences still need execution proof. Revocation or timeout cannot reuse
rings until the exact attachment and transfer acknowledge hardware quiescence.

The existing class-neutral `usb_base/control_factory.rs` checks selected Base,
capability, implementation, artifact, typed ports and laws. `control_owner.rs`
binds the exact Root resource possession and attachment generation, one in-flight
operation, and a separately granted `conduitos.authority/usb-control@1` authority.
Its serialized transport envelope permits 4096 bytes, while its current native
attachment data limit is 256 bytes. An offer cannot silently widen that limit.
Endpoint read/write offers separately describe bounded class-neutral bulk
contracts; neither a descriptive offer nor compiler output initializes a provider.
Plan selects exact resources and authority; Play retains active leases and retirement.
Device-class policy remains outside Root discovery.

No ordinary LoongArch64 native USB provider is admitted by this feasibility probe.
Follow-on #5742 must prove Root controller initialization, safe DMA/ring possession,
actual IRQ delivery and bounded control/bulk completion before advertising executable
offers. USB3, MSI/MSI-X, isochronous traffic, different boards and physical devices
cannot inherit this receipt. This audit proves emulator firmware PCI assignment
and records the Root seam; it claims no ConduitOS USB transfer or physical run.
