# Native workset runtime core

This crate contains the existing production `PlannedBack` implementations and
exact `FixedScheduler` specialization used by `NativeWorksetPlay`: 14 nodes,
10 Cords/queue slots, 16 ports, 10 values of at most 3072 bytes, and 1024 Signs.
The Root keyboard/text adapter reexports these same Backs. There is no second
scheduler algorithm or semantic execution model.

`Admission` contains only already-lowered numeric tables. It constructs the
same scheduler, including initial empty values, active prefixes, sealed routes
and Host Call bindings. Planning, Body admission and physical effects are not
performed by this crate. The standalone guest owns the scheduler and all of its
Cord, ready, value and Sign storage for the x86_64 native workset path.

Wire version 1 uses explicit little-endian fields and correlated nonzero
instance/request envelopes. Init transfers at most the fixed 16 KiB window;
Host Call values are bounded to 3072 bytes. No Rust layout or pointer crosses
the wire. Every reply carries the current exact Sign retention gap; Root keeps
no second Sign log. Scheduler errors retain their explicit canonical variants.
