# Separate structured equality preparation frame

Revision `2dc324d05` extracts structured equality preparation into a non-inlined
helper. Primitive Binary nodes retain the existing primitive branch and no
longer need the structured evaluator's local preparation frame. No equality
semantics, admitted bounds or validation conditions change.

Six structured equality tests and three scratch-storage tests pass. All 19
32-bit environmental publication/kernel tests pass, including the seven
960-sample workloads. Plot library Clippy passes with warnings denied.

The complete ESP32 image built and flashed successfully. Physical readiness
passes: 13 actual initialized software owners lend 55,655 bytes of offers,
with unchanged Root heap use and native dispatch counts across the request.
The primitive binary path is now inlined into the 592-byte prepare_node frame;
the structured branch has a separate 1,248-byte frame. The Source constructor
frame is 3,104 bytes. Root heap retains 81,084 bytes, peaks at 90,564 bytes;
SDK peak is 34,024 bytes in this boot before network provisioning.

This proves Source-first initialization and read-only availability on hardware.
It starts no Body, installs no climate kernel, and proves no sensor sampling,
Face, station association, mDNS or recording. Network fallback with the software
still resident is checked separately. The retained image includes the resident
slot backing change; it is not a standalone physical proof of this extraction
against dev without the earlier environment construction work.

A physical failed-association fixture then passed with this software still
resident. Exactly one association and one AP restoration dispatched in the
same Boot. Root peak was 110,920 of 114,688 bytes and SDK peak 37,904 of 73,728.
A second read-only software availability proof passed after recovery in the
same Boot, retaining all 13 offers and unchanged heap/dispatch counts across
the query. Root retained 86,536 bytes after the network control installation.
This is negative association/recovery proof, not a successful network join.
