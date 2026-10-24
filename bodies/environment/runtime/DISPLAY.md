# Prepared Source packet encoding

`PreparedDisplayPackets` executes the checked `tm1637-packets` expression from
`plots/device-protocols/tm1637.conduit`. Make compiles its canonical program;
preparation reserves the evaluator and complete Frame/packet composer storage.
Repeated Frame/packet encoding allocates nothing. The protocol's input and
result are closing Flows, so an admitted timed Mask can supply successive pages.

The input is six already-mapped registers, brightness and enabled state. Source
owns command construction and refuses brightness outside 0..7. `encode_frame`
preserves that invalid input for an installed Plot's refusal Fore;
`encode_canonical` returns the existing canonical packet Type or refusal.
`encode` borrows the same Source result and exposes three finite wire slices.
A returned canonical slice borrows prepared storage until the next mutable call.

The Face keeps first-class quantities; the Mask/Look owns unit selection,
formatting and page policy. This component creates no offer, Plan, capability,
scheduler, GPIO effect, acknowledgement or recording. Actual physical display
installation remains separate work under #6295.

`cargo xtask check tm1637-output` checks both the driver and this encoder. CPU
regressions cover all enabled/brightness controls, six mapped registers,
canonical driver Type and finite length/padding, invalid brightness and zero
recurring allocations. They do not claim physical delivery.
