# Pro Micro target adapter

Issue #1926 owns this target. The board is a SparkFun Pro Micro,
ATmega32U4 5 V / 16 MHz, with application USB PID `0x9206`.

This directory is not a second Brainstem runtime. The checked image is a
transmitter-disabled target adapter awaiting installation of an ordinary
Conduit `AssignedPlan`. It does not advertise offers, bind Boots, invent Play
or authority identities, schedule obligations, or execute a private HIL model.

Board facts are deliberately small:

- D0/PD2/RX1 receives Create UART from cargo-bay pin 2.
- D1/PD3/TX1 transmits Create UART to cargo-bay pin 1.
- D4/PD4 reaches the Create power-toggle input on cargo-bay pin 3.
- D5/PC6 observes the Create charging output on cargo-bay pin 13.
- UART framing is Create 1 OI v2 at 57,600 baud, 8N1.

At boot and throughout the uninstalled maintenance loop, UART TX and the power
toggle are inputs. `HELLO`, `STATUS`, and `ATTEST` inspect the target and build;
they are not a Conduit Host lifecycle or execution protocol.

The executable image must consume the generic compact `AssignedPlan` defined
by `conduit-core`, derived by the ordinary checker/planner/lowering path. If
AVR C++ cannot compile the shared Rust `conduit-create-oi` crate, board-local
bytes must be generated or conformance-locked to that crate rather than
defining another Create semantic API here.
