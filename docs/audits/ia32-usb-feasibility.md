# IA-32 USB controller feasibility

For #5578, the verified candidate is QEMU 10.2.1 (Debian package
`1:10.2.1+ds-1ubuntu3.2`), machine `pc-i440fx-10.2`, CPU `qemu32`, one vCPU,
512 MiB guest RAM, and explicitly selected SeaBIOS `/usr/share/seabios/bios-256k.bin`.
The receipt pins its 256 KiB extent and SHA-256 digest. This follows the existing
IA-32 product's legacy-BIOS machine family; it does not validate that product
image with USB. The exact command and firmware PCI observation are retained in
[the receipt](../../proof/usb/ia32-feasibility.json).

The probe attaches `qemu-xhci` with one USB2 port, no USB3 ports, MSI/MSI-X off,
and an emulated USB mouse. After two seconds of firmware execution, QMP
`query-pci` reports controller `00:03.0`, PCI `1b36:000d`, subsystem `1af4:1100`,
BAR0 at `0xfebf0000` with 16384 bytes, non-prefetchable 64-bit memory encoding,
and interrupt pin A assigned to IRQ 11. The BAR encoding is 64-bit, but this
observed address and its complete range fit below 4 GiB.

## Memory and interrupt constraints

An IA-32 implementation must validate the selected BAR's high half and complete
range before narrowing an address. It must reserve finite controller rings,
event rings and transfer storage within actual Root-owned guest RAM, below the
profile's 32-bit address ceiling. The observed profile supplies 512 MiB RAM;
this is not a DMA grant. Root must separately initialize PCI memory/bus-master
state, select the interrupt route, retain register possession, and establish
ordering/coherency for admitted DMA. IRQ 11 assignment does not prove delivery.
MSI/MSI-X, USB3, streams, isochronous transfers and different controller models
have no execution proof here and cannot inherit this receipt.

## Existing Root boundary

The class-neutral `machine/usb/control` contract admits finite 4096-byte request
and result material, one in-flight `conduit.host/usb-control@1` operation,
`machine/usb/control-attachment` resource possession, and the separately granted
`conduitos.authority/usb-control@1` authority. Its selected Base, exact capability,
implementation, artifact, typed ports and semantic laws are checked by
`control_factory.rs`. `control_owner.rs::bind_admitted` requires Root to own the
exact controller/slot and DMA envelope under the resource generation; requests
cannot choose a different slot or buffer. Rings remain owned until acknowledged
hardware quiescence, even after revocation, timeout or provider loss.

`endpoint_read_offer.rs` and `endpoint_write_offer.rs` similarly describe bounded
class-neutral endpoint operations. Describing an offer grants neither a device
attachment nor an initialized provider. Host/Boot and offer generation identify
observed resources; Plan selects exact resources and authority; Play owns active
leases and their retirement. HID, storage, audio and network class policy stays
in its existing owners rather than Root discovery.

## Supported disposition

Current IA-32 ConduitOS has no admitted ordinary native USB provider. Its product
must continue to refuse USB requests rather than advertising these descriptive
contracts as executable. Follow-on #5740 can implement only this verified
controller profile after Root initialization and DMA/IRQ possession are proved.
The IA-32 UEFI32/q35 route is separate: this environment lacks the required
`OVMF32_CODE_4M.fd` and `OVMF32_VARS_4M.fd`, so it is not the verified firmware
profile. A paused q35 enumeration alone supplied no assigned BAR or IRQ.

This receipt proves emulator firmware PCI assignment and documents the existing
checked Root seam. It proves no USB transfer, guest interrupt, DMA operation,
ConduitOS USB boot, or physical compatibility.
