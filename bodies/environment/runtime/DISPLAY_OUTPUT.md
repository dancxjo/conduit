# Ordinary display output construction

`PreparedDisplayKernel` executes the checked display-output Source in the shared
fixed scheduler. `prepare` retains the fixed-store conformance profile.
`prepare_with_storage` accepts independently prepared finite value and sign
stores, and puts the unchanged scheduler in boxed custody through its existing
boxed constructor. This avoids returning the large scheduler by value through
preparation frames. Heap-backed stores still have fixed admitted capacities;
Play does not allocate.

Both entrances validate the exact Plan and lowering, numeric kernel tables,
current retained software selection, and output shape before transferring the
software. Supplied stores must be empty, provide at least twelve value slots
and 12 KiB of live value capacity, cover the lowered sign budget, and reserve
256 remote-sign slots for the finite Fore profile. Root separately admits the
stores' backing, the scheduler allocation and temporary construction headroom.
This API is not an allocator admission guarantee or effect authority.

The public `cargo xtask prove environment display-output` proof runs the actual
Source with both fixed and finite hosted stores. It checks two successive
frames, invalid brightness, pre-service cancellation and zero allocation during
Play. Undersized values, insufficient sign or remote-sign backing, occupied
values and occupied signs refuse without consuming retained software owners.

This construction prerequisite is tracked by #6374. It does not install the
output graph on the MCU, authenticate a Fore caller, issue a native capability,
start a Body, or replace the live bench's coordination loop. Compact assignment
of this graph's external Fore, combined sensor/Mask/output admission and physical
execution remain required for the continuing native appliance.

## Compact native preparation

`PreparedAssignedDisplayKernel::prepare` accepts the canonical header and
borrowed records, a trusted linked Make profile, the current Root scope, retained
software and independently admitted finite stores. It does not deserialize a
rich Plan or reconstruct software from advertised descriptors. The shared
assigned-table builder and original kernel constructor preflight the complete
numeric topology before exclusive software transfer.

The retained-owner check matches current Host/Boot/offer epoch, Source family,
exact kind/implementation/artifact identities, actual program/selector text and
pure Host Call ABI. All three initialized portable owners are checked before
any is transferred. A dropped preflight keeps them available; a successful
transfer withdraws them from future offers and replay refuses as already
selected. There is no automatic epoch increment: Root owns that transition.

The compact result retains numeric Fore pins. Root must independently retain
and enforce their typed value contracts, bind the actual GPIO owner and native
authority, admit complete memory/work and issue Body start before stepping.
Preparation creates no native provider, capability possession, Line or Body.
The checked display CPU proof now uses this production preparation entrance,
including stale epoch, foreign Source/Boot, occupied storage and zero-frame
refusals followed by successful execution of the original owners.

## Trusted firmware layout provisioning

`PreparedDisplayOutput::display_layout` derives resident record pins, computation
configuration bytes and exact local Fore pins from the normally sealed output
Plan. The display-offer preparation example emits `display-layout.rs` alongside
its assignment. This build input contains no live Host, Boot or Plan identity;
the same source and selections produce identical layout material across Boots.
Incoming assignments still require current scope, retained owners and authority.

The ESP32 `display-ready-root` profile includes that generated module against the
production compact runtime types. This is preparation only: the module grants
no Fore access, issues no native possession and starts no output Play. The
existing live bench remains the deployed image until authenticated assignment,
complete admission and continuing Body execution are connected.

`ValidatedAssignedDisplay::admit` separates topology staging from software
transfer and kernel construction. The private token owns checked finite tables
and Fore pins without borrowing ingress bytes. Admission checks the linked
layout, current Host/Boot, local Fore, native call shape and ordinary scheduler
geometry without allocating. It issues no authority and does not verify current
software availability; retained-owner selection still occurs during preparation.

The ESP32 Root display reservation entry point accepts this validated token,
correlates full Plan/fragment identities with its compact identity and checks
the expected next Root Play before consuming the global sequence. The signed
owner wrapper additionally binds the assignment seal and Body/Wake provenance.
CPU tests exercise production reservation/authentication code with explicit
Boot/entropy substitutes. This API is not yet connected to display ingress or
native capability issuance, and reserves no physical output operation.

Root can now issue bounded display possession for the reserved validated output
assignment. It checks the exact local Fore pins, current retained provider,
selected native node and finite Root-selected frame budget before committing
issuance state. Failed checks preserve issuance; successful issuance is fenced
against replay and transfers no GPIO owner. The existing driver binder then
transfers the retained owner exactly once without electrical effects.

CPU evidence uses production Root issuance with explicit entropy/Boot/pin
substitutes. Display ingress, complete installation and start, physical execution
and display retirement/reoffer remain unconnected. This temporary issuance path
is conservatively one-shot; no automatic provider reuse or fresh generation is
inferred from dropping a handle.
