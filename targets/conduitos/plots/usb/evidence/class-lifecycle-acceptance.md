# CDC and ECM lifecycle proof boundaries

The successful product guest receipts and deterministic fault witnesses serve
different purposes. `current-usb-classes-a783be37/` retains actual emulated USB
traffic, exact class bytes, finite-buffer reuse, allocator observations and
acknowledged Stop. Each guest receipt binds its original kernel and media.

`tests/usb_cdc_serial/kernel.rs` prepares the actual selected Serial Back, Plan
and Kernel with production pure expression and selector execution. Canonical
physical-call replies exercise pending bulk cancellation, original-token late
completion rejection, terminal STALL/provider loss/timeout and partial actual
byte counts. The physical replies are simulated. This witness does not prove
xHCI Stop acknowledgments or native DMA retirement.

`tests/usb_ecm_lifecycle.rs` prepares the actual checked ECM Source, selected
Plan and Kernel with separate USB and Ethernet offers and grants. Canonical
provider replies exercise pending IN/OUT cancellation, late original tokens,
provider loss, short and oversized input, stale receive ordinals, malformed
Ethernet source identity and partial OUT. Terminal cases must fence further
USB effects and network delivery and retire the actual graph. Its explicit
network fixture grant is no claim of physical network possession.

Native cancellation and stale-attachment enforcement remain covered by
`src/arch/usb_xhci/usb_native_host/paired_lease_tests.rs`: real production native
owners and Kernel tokens with simulated controller events, acknowledged stop,
late original-event rejection and failed-stop quarantine. Separate Ethernet
owner/installation tests reject unrelated authority and stale sessions.

The retained `class-lifecycle-tests/` logs record five selected serial tests
and two ECM lifecycle tests passing with no failures. Exact-head CI and
merged-tree acceptance remain required before closure. These layers
complement the guest proofs without converting simulations into physical USB
fault injection or certifying unbounded traffic and arbitrary class profiles.
