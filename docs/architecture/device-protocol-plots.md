# Device protocols in plots

Owner: [#4833](https://github.com/dancxjo/conduit/issues/4833).
USB integration: [#4831](https://github.com/dancxjo/conduit/issues/4831).

Rust owns physics-facing primitives and invariants; reviewed checked plots own
ordinary protocols. Add device support by composing a protocol plot over an
already capable base. A new register map, opcode, checksum, calibration formula
or probe sequence is not a reason to add a device driver to ConduitOS.

This is the implementation direction authorized by #4833. Existing device Rust
remains temporary debt until replacement plot execution is implemented. This
contract does not claim current device coverage or physical compatibility.

A base owns bounded bus or endpoint operations, exact resource possession,
controller limitations, electrical ownership, interrupts, DMA lifetime and
mandatory local safety. It returns truthful typed refusals and provider loss.
It neither recognizes a peripheral nor invents discovery, retries, authority,
attachment or connectivity. A probe consumes an already admitted attachment;
its protocol-level recognition cannot grant permission to use the device.

A protocol plot owns register and packet meaning, initialization, framing,
checksums, byte-order interpretation, finite request/response state, calibration
and conversion into typed observations. It retains explicit bounds and exact
malformed, short, overflow and protocol-refusal outcomes. It cannot suppress a
mandatory local stop when stalled, cancelled, failed or removed.

The same protocol source must work with every honest implementation of its
required low-level Fore. Planning selects controller, resource, provider and
Host/Boot truth. Source contains no MMIO address, controller name, device path,
GPIO assignment or constructible authority. Protocol register numbers and wire
fields describe the exchange; they cannot select an attachment or exercise an
effect outside the exact selected base. The canon's prohibition on authored
addresses and device/resource binding facts still applies to realization facts;
#4833 explicitly requires protocol-level addressing inside a bounded exchange.

## Source package preparation

The development implementation accepts bounded packages using
`conduit.conduitos/protocol-source@1`. A package contains Source and exact typed
specializations of generic state, merge and pair Backs. It contains no controller
selection, resource possession or authority. Preparation checks the Source and
retains those Back owners before publishing their exact offers.

The native caller supplies host offers, selected placements, bases and grants to
`PreparedProtocolSource::plan_artifact`. Ordinary planning seals the exact Cords
and finite queues; artifact admission binds the distinct Source, checked,
expanded and artifact identities. Input fan-out retains the planner's atomic
routing and exact common Fore contract. The resulting package owner consumes
actual I2C and clock possession together when preparing Play.

Package a reviewed JSON Source package through the repository entrance:

```sh
cargo xtask make conduitos protocol-source \
  --package protocol-source.json --entry device-protocol \
  --output-dir checked-protocol-package
```

The new output directory retains the exact package bytes and a receipt binding
its SHA-256 digest, Source identity, checked plot identity and expanded plot
identity. This is checking and expansion evidence; packaging neither rebuilds
the target nor admits a controller or starts Play.

This production preparation chain is exercised by deterministic native kernel
conformance through `cargo xtask check device-protocols`. The x86_64 product Root
also has a standalone native boot entrance: a local administrator supplies an
exact Source package and a separate bounded Root request. The request names the
entry, approved package digest, selected PCI function, address interval, finite
operation/poll/step budgets, clock lifetime and canonical Fore input bytes.
Firmware handoff, electrical approval and exclusive machine ownership must be
established independently by the administrator before installing this profile.
Decoded flags and Source identities cannot establish those facts.

Package those inputs over an already capable product kernel, without rebuilding:

```sh
cargo xtask make conduitos protocol-image \
  --kernel product/conduitos --build-record product/build.json \
  --package protocol-source.json --root-request protocol-root-request.json \
  --output-dir protocol-media
```

This command checks the exact x86_64 product-kernel digest and Source binding,
then retains the boot image and packaging receipt in a new directory. At boot,
Root checks compiled implementation inventory, actual controller configuration
and real calibrated clock availability before issuing narrowly bounded native
possession. One canonical Body runs the admitted Source entry through the
ordinary kernel; typed output bytes are diagnostic records. Retirement keeps
its biography and resource reservations alive instead of starting a second Body.
This path does not yet have a retained freestanding execution result. Packaging
receipts establish preparation, not hardware compatibility or execution.
Native Limine observation selects bounded, exactly named modules without itself
granting trust or execution authority.
The x86_64 ICH5–ICH9 realization validates an explicitly selected PCI function,
its I/O window and live configuration before constructing the finite provider.
It refuses busy hardware, SMI routing, I2C mode and auxiliary CRC/buffer modes;
the native root must already own firmware handoff and exclusive access.
A package's descriptive identities
do not establish review, firmware release, attachment ownership or permission to
use a controller.

Boot preparation and the packaging command retain the same selected entry:
checked Source, exact expansion, generic operation owners and a SHA-256 artifact
identity for the package bytes. Repackaging the same Source can preserve its
resident meaning while changing its artifact identity. Limine observation uses
the exact module command `conduit.protocol/source@1`; absence remains absence,
and duplicate or oversized modules are refused before Source preparation.
The IA-32 boot entrance currently reports unsupported for this module path.
The retained entry contributes its exact partition to ordinary body planning;
successful preparation still does not start Play or authorize a controller.

## Existing Rust responsibility audit

The categories below describe responsibilities, not whole-file exemptions.
“Temporary debt” means retained production behavior awaiting an equivalent
plot path. “Fixture/history” is proof material, never an available production
Back. No package should acquire permanent parallel protocol implementations.

| Existing surface | Base/invariant responsibilities | Protocol responsibilities retained as temporary debt | Fixture/history |
|---|---|---|---|
| `mechanisms/devices/mpu6050/src/device.rs` | Provider execution and platform deadlines belong to the selected I2C base; the device-named provider traits are migration debt, not the final generic bus contract. | Address/identity validation, wake/configuration register writes, sample reads, initialized state, big-endian decoding and scale conversion. | The module's providers, expected transaction sequences and refusal tests are deterministic fixtures. |
| `mechanisms/devices/mpu6050/src/derive.rs` | A mandatory local hazard consumer must retain fresh trusted safety input independently of ordinary plot scheduling. The calibration and derivation routines themselves do not establish that enforcement. | Gravity calibration, orientation/tilt/impact interpretation and arithmetic are candidates for ordinary bounded plots. Exact calibration authority/generation remains Host/Plan truth in `bodies/pete`. | Derivation tests prove arithmetic and refusal cases; they do not qualify a mounting, threshold or physical safety envelope. |
| `mechanisms/devices/ssd1306/src/lib.rs` | The selected I2C provider owns electrical/resource access, deadlines and bounded transaction delivery. | Device address restrictions, command/data control bytes, initialization and addressing sequences, frame chunking and initialized state. | In-module framebuffer/transaction providers are deterministic fixtures. A delivered frame is distinct from a human seeing the display. |
| `mechanisms/devices/create-oi/src/device.rs`, `stream.rs`, `observation.rs`, `battery.rs`, `presentation.rs` | An actuator provider must independently fence unsafe traffic; generic UART ownership, bounded send/receive, closure and loss remain below plots. | OI packet/opcode tables, frame synchronization/checksum, packet decode, observation and battery interpretation, music/indicator commands. `presentation.rs` also contains a motion-free byte invariant: preserve that protection at the trusted boundary when migrating the ordinary sequence. | Device, stream and observation tests are deterministic protocol fixtures. Existing physical records remain evidence of their named Rust implementation. |
| `mechanisms/devices/create-oi/src/drive.rs`, `safety.rs`, `safety_latch.rs` | Mandatory stop on hazard, stale control, provider/link loss and expiry; fresh local safety observations, persistent hazard latches, conditional clear, authority and finite motion accounting. These cannot become author-wirable safety gears. | Ordinary requested wheel-command framing may migrate, but the trusted stop path and any protocol bytes indispensable to non-bypassable stopping stay below plots. | Drive/safety tests prove their specified enforcement cases, not all future hardware safety. |
| `mechanisms/devices/create-oi/src/mode.rs`, `power.rs`, `contact_withdrawal.rs` | Stop-first ordering where required for safe mode changes, electrical power-toggle bounds, and non-bypassable withdrawal limits/preemption remain below plots. | Ordinary mode queries/transitions, power requests and withdrawal policy are migration candidates only after preserving their local stop line. | Existing tests remain fixtures for exact current transitions, timing and refusals. |

The current consumers include `bodies/pete`, `targets/std/src/std_create_uart.rs`,
the Pico W Pete capstone and the AVR Brainstem. Retain their working paths until
equivalent plotted behavior is integrated; changing a README does not migrate a
consumer. The [Pete migration contract](pete-brainstem-migration.md) continues to
own its attachment and motion-safety obligations.

The [USB groundwork](../../targets/conduitos/plots/usb/README.md) separately
classifies xHCI resource/ring ownership and protocol interpretation. USB class
plots must consume bounded class-neutral endpoint/control operations rather
than arbitrary register-programming authority.

## Implementation validation and hardware evidence

#4833's owner revised closure to code completion on 3 October 2026. Normal code
validation remains required; physical/HIL enactment and accepted-release
receipts are not closure gates. Source-level completion cannot be reported as
physical compatibility. Deterministic parsing/calculation, production kernel
execution, emulator devices and physical devices remain separate proof classes.

A future hardware campaign should correlate source and checked plot identities,
selected low-level implementation/resource, Plan/Play and attachment identities,
bounded transaction transcript, probe/observation/refusal and lifecycle outcome.
Removal/substitution must distinguish missing hardware, wrong protocol identity,
malformed data, bus refusal, provider loss and stale attachment without default
observations. Preserve old evidence under the implementation it actually tested.
