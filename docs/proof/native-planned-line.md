# Native planned USB Line prerequisite

This development prerequisite separates exact planned-Cord preparation from the
existing QEMU USB connectivity harness. It does not admit Body membership,
authorize a controller, execute a remote fragment, or render a remote Face.

The API is `conduitos::native_participant::planned_line::PlannedUsbLine`:

```rust,ignore
let prepared = PlannedUsbLine::admit(
    &sealed_plan,
    &current_host_advertisement,
    &exact_session_binding,
    &admitted_usb_basis,
    &current_usb_observation,
    session_role,
)?;
let carrier = prepared.attach(ftdi_ready)?;
```

The caller must already possess the current Host advertisement, an admitted USB
Line basis from `offer_usb_ftdi_line`/`AdmittedUsbLineBasis::from_offer`, and the
physical observation and initialized FTDI realization. These values describe
real discovered state; this API is not an authentication or membership protocol.
It does not infer authority from receiving a session Hello.

`admit` verifies the Plan seal and both fragment identities; the selected local
role must match current Host, Boot, and offer generation. The Cord must occur
exactly once in both fragments, with exact source/output and sink/input ports and
value meaning. Its selected Line must equal the supplied admitted basis. The
entire SessionBinding is reconstructed from that planned Cord and compared with
the supplied binding, including endpoint and per-Plot Play identities.

`attach` verifies the current observation and the FTDI interface, endpoint,
packet, and ring realization before preparing the carrier. `send` and `receive`
use the existing bounded `UsbLineSession` and canonical `SessionMachine`.
`observe` rejects stale or replaced realization; after observed loss the carrier
cannot be revived by resubmitting an old observation. The caller still owns
physical discovery and must report changes rather than retaining stale facts.

The existing profile remains unchanged:

| Limit | Value |
| --- | --- |
| In-flight values | 1 |
| Value payload | 64 bytes |
| Buffered bytes | 1024 |
| Encoded frame | 1024 bytes |

This profile cannot transport a general 4096-byte native Mask Presentation as
one value. No fragmentation protocol or larger Line profile is introduced here.

## Proof boundary

Deterministic tests use an ordinary planner-produced two-Host Boolean Cord and
check both session roles, stale Host/Boot/generation, substituted Play and endpoint
bindings, a tampered Plan seal, physical replacement, and observed loss. Existing
USB framing and offer tests remain applicable.

The existing full emulator regression enters through:

```sh
cargo xtask make conduitos journey-proof
```

That command builds an x86_64 architecture-proof appliance, boots QEMU, runs the
native product journey, exchanges 260 values with the explicitly named test
peer, and verifies removal/stale-session refusal. Its `product_usb_line` harness
now calls the shared carrier machinery. Its synthetic harness identities remain
confined to that caller; a passing harness does not establish remote participant
membership, production-owner planning, or remote graphical presentation.

The command produces `target/conduitos/x86_64/journey-proof.json` and serial/QEMU
diagnostics. Run it at the candidate source revision and preserve its exact image
hash and evidence. Compilation and deterministic tests alone do not establish
that emulator result.
