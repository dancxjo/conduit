# Pete Pro Micro Brainstem firmware

Issue #1926 owns this target. The exact initial board is a SparkFun Pro Micro,
ATmega32U4 5 V / 16 MHz, identified by application USB PID `0x9206`.

The initial image is intentionally non-actuating. It provides a bounded native
USB CDC identity/status seam while keeping the hardware UART uninitialized and
its RX/TX pins high impedance. It contains no Create OI command bytes, motion
operation, automatic retry, or UART discovery. USB enumeration alone does not
establish a Host/Boot, Create attachment, authority, or physical communication
claim.

Do not flash or open CDC while the Create is attached until the operator has
confirmed it is stopped, attended, and physically unable to propel itself. A
later physical slice must separately qualify the exact electrical attachment,
legacy bytes, finite authority, cleanup, and HIL proof.
