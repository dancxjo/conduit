# Physical dormant-kernel preparation on ESP32

The exact image in receipt.json was flashed to the attended HW-463 ESP32.
`cargo xtask prove environment kernel-prepare-esp32` captures actual initialized
native/software offers and policy, plans on the preparation host, checks refusal
before assignment staging, exercises the physical assignment refusal/acceptance
controls, and prepares the ordinary dormant fixed scheduler on the MCU.

The kernel object is 10,760 bytes. Root retained heap is 93,252 / 114,688 bytes;
preparation peak is 97,220, leaving 21,436 retained and 17,468 peak free bytes.
The additional preparation guard is 27,660 bytes (including explicit control
headroom), checked before transfer. Duplicate preparation refuses without growth.
Transferred software is no longer advertised as a second ready set; the
management loop survives that refusal. Native provider facts remain unchanged.

This is actual MCU scheduler/storage preparation, not Body installation or Play.
No native capability is issued, no Step runs and no sensor/timer is dispatched.
The result is retained as a preparation diagnostic. Full installation must still
admit actual native bindings/control custody before the software/table transfer
and retain an attributable Body start. The rich proposal remains host-side.
This receipt does not claim physical overnight recording or full live closure.
