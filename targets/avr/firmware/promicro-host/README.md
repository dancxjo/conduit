# Rust Pro Micro Host image

This excluded firmware crate is the ATmega32U4 realization owned by #1926.
It uses the existing `conduit-create-oi` `no_std` contract directly. Board code
is limited to the SparkFun Pro Micro UART/GPIO mechanism and contains no Create
opcodes, Conduit lifecycle, offers, Plan identities, or private execution
protocol.

The current image is deliberately non-executing and transmitter-silent while
the ordinary compact `AssignedPlan` consumer is installed. D4 remains an input;
D5 is an input; UART construction alone emits no Create OI bytes.
