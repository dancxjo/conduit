# Physical literal scratch isolation

Issue [#6323](https://github.com/dancxjo/conduit/issues/6323) owns the literal
dispatch change. Unit and Quantity parsing/encoding use separate non-inlined
helpers; ordinary literal dispatch retains no local physical value carriers.
Typed quantity validation and exact encodings remain unchanged.

All 476 Plot library tests pass (one pre-existing ignored test), as does strict
Plot library lint. The final ESP32 build and frame measurement remain pending;
these CPU checks do not prove target stack closure, task custody, Body start,
Face publication or physical recording. The code is held on the construction
branch while existing shared-core review slots are occupied.

The revised proof command passes strict dispatch-package lint. It prebuilds the
admission verifier before flashing, then opens UART without rebuilding it, and
retains at most 64 KiB of raw startup output plus 32 parsed observations. This
diagnostic capture is limited to the read-only readiness phase.

While the final literal image builds, the prior verified Root ELF was restored.
A fresh UART offer confirms responsive control and an unstarted new Boot, with
no Body/native plan installed and no appliance kernel prepared. It is not the
new worker image and does not claim active climate sampling.

The final release build passed. Exact linked disassembly measures ordinary
literal evaluation at 432 bytes (previously 4,416), quantity literal parsing at
3,328 bytes, and Unit literal parsing at 1,616 bytes. Those physical carrier
frames are entered only by their respective typed literal paths. The linked CPU
stack span remains 28,988 bytes; no heap backing or numerical bound was enlarged.
`frames.json` binds this measurement to the exact ELF digest. The next physical
preparation must still prove startup and actual held-task custody.
