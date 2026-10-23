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
