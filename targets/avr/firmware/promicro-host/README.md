# Pro Micro Host firmware

This standalone Rust firmware realizes an ATmega32U4 SparkFun Pro Micro Host.
It receives a bounded `AssignedPlan` and matching activation over its USB Line,
validates their Host/Boot identities, and executes the admitted Create contact
observation through the ordinary kernel's single-source executor. It returns
a compact execution receipt; it does not introduce another scheduler.

Create Open Interface commands come from `conduit-create-oi`. Board adapters
supply UART/GPIO and USB mechanisms. D4 and D5 start as inputs, and constructing
the UART does not transmit. An admitted contact observation can issue the
bounded Create operation; the image is no longer a non-executing placeholder.

Build through the repository entrance:

```sh
cargo xtask avr build
```

`cargo xtask avr --help` lists release, diagnostic, attended observation, and
flash commands with their device and physical prerequisites. Compilation and
image inspection do not establish execution on the attached robot.
