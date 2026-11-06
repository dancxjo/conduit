# DWC2 controller mechanism

This module owns register sequencing and finite polling/PIO work. The B+ adapter
in `../armv6/usb.rs` owns its register window, firmware power operation and internal
LAN_RUN wiring. Compiling or preparing either module does not install a USB Base,
create an attachment capability, or advertise an input provider.

`prepare` keeps interrupts and DMA disabled, validates the core and FIFO geometry,
resets the root port and preserves its observed attachment state. `root_packet`
uses channel zero exclusively for one directly attached high-speed EP0 packet,
at most 64 bytes. It reports NAK, NYET, stall, receive-envelope failure, removal,
overcurrent, deadline and failed retirement separately. It never resubmits. FIFO
padding is not received data. Channel reuse requires acknowledged halt; a failed
halt fences the controller. There is no DMA or allocation in these operations.

`root_control` composes opaque setup, bounded data packets and the opposite
zero-length status stage. The 256-byte envelope permits at most 32 data packets;
short IN packets preserve their actual extent. DATA PID toggling is transport
mechanics. Request opcodes, addressing policy and result meaning stay outside.

Register fields follow the [DWC2 register definitions in the Raspberry Pi Linux
source](https://raw.githubusercontent.com/raspberrypi/linux/rpi-6.12.y/drivers/usb/dwc2/hw.h).
Deterministic register-model tests establish software bounds and sequencing;
physical controller compatibility remains unverified.

The existing admitted USB control/endpoint owners remain the integration boundary.
Hub power/status exchanges are checked Source in `plots/usb/hub-port-*.conduit`.
Descriptor interpretation, address/configuration request construction, class
recognition, retry policy and HID reports must remain there. The shared HID mouse
Source package also includes absolute-axis normalization
through `hid-absolute-pointer.conduit`, reusing its exact pointer state and output
contract. Descriptor/report decoding and the physical endpoint still have to
supply those observations; existing Face interaction is the intended consumer.

Remaining integration includes control-transfer dispatch through the existing
Host Call owner, normal Make/Base admission, the onboard hub's downstream
port lifecycle, split transactions for full/low-speed devices behind its TT, and
endpoint reads feeding the existing checked HID plots. There is no product input
offer or physical USB-input claim yet.
