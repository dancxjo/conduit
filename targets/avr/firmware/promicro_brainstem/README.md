# Pete Pro Micro Brainstem firmware

Issue #1926 owns this target. The exact initial board is a SparkFun Pro Micro,
ATmega32U4 5 V / 16 MHz, identified by application USB PID `0x9206`.

The initial image is intentionally non-actuating. It provides a bounded native
USB CDC identity/status seam while keeping the hardware UART uninitialized and
its RX/TX pins high impedance. It contains no Create OI command bytes, motion
operation, automatic retry, or UART discovery. USB enumeration alone does not
establish a Host/Boot, Create attachment, authority, or physical communication
claim.

Host tests live outside this sketch directory. Arduino compiles every `.cpp`
beside an `.ino`; placing a test `main()` here would replace the Arduino core
entry and would produce a test executable rather than the claimed firmware.

Do not flash or open CDC while the Create is attached until the operator has
confirmed it is stopped, attended, and physically unable to propel itself. A
later physical slice must separately qualify the exact electrical attachment,
legacy bytes, finite authority, cleanup, and HIL proof.

The compiled-disabled codec is checked against `conduit-create-oi` and the
pinned Netherwick Brainstem revision
`f43ff13846b47b05e133d0321bdbaafffd1bcdbe`. The valid legacy vectors retain
Create 1 ordering and masks; broader historical LED input bits are deliberately
masked to the Create 1 PLAY/ADVANCE contract already established by Conduit.

The assigned-obligation slot stores only one exact plan-fragment/operation
identity and its finite group-0 request, response, deadline, and disposition.
It is not a Body graph, planner, scheduler, UART authority, or retry policy.

The USB CDC command grammar is newline-terminated, uppercase, and bounded to 64
bytes. Numeric fields are fixed-width canonical hexadecimal; lowercase, missing
fields, extra fields, carriage returns, and noncanonical separators are
malformed rather than normalized:

```text
B HHHHHHHH:BBBBBBBB:GGGGGGGG
A HHHHHHHH:BBBBBBBB:GGGGGGGG:FFFFFFFF:OOOO:PPPPPPPP:AAAAAAAA
```

`B` binds the Host (`H`), current Boot (`B`), and offer generation (`G`) in
RAM. `A` admits an observation activation for that same placement plus Plan
fragment (`F`), operation (`O`), active Play (`P`), and authority grant (`A`).
Replies echo every identity for exact post-flash verification. Even an admitted
activation reports `execution=disabled create_uart=isolated`; this seam does
not authorize or perform Create I/O.
