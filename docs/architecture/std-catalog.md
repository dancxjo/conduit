# `conduit.std` catalog truth boundary

The current `conduit-std-catalog` crate is a pre-S5 compatibility catalog, not
an executable standard-library claim. Its eight contracts can be checked and
their matching offers can be planned, but their optional hosted implementations
run through `conduit-runtime::HostRuntime`. They do not lower or execute through
`conduit-kernel`.

The authoritative row-by-row audit is the checked-in
[`std-catalog-truth-inventory.tsv`](std-catalog-truth-inventory.tsv). Each row
names the exact catalog revision, ports and value kinds, configuration, hosted
fixture, planning boundary, execution path, proof, platform claim, truth
classification, and stop line. A test keeps the inventory in one-to-one
agreement with `standard_contracts()`.

## Audit result

Every current catalog row is classified `misdesigned / needs rearticulation`.
That result is about these exact revisions, not the worth of the semantic ideas.
The recurring gaps are:

- every port uses `value/any`, so the contract erases distinctions required for
  exact planning and execution;
- `flow/map`, `flow/filter`, and `text/format` use numeric selectors that are
  interpreted by host-side switches rather than binding portable semantic
  artifacts;
- several compatibility implementations complete after the first emitted
  value, so their hosted demonstrations do not prove the advertised stream
  behavior;
- `text/format` advertises a generic output even though its label promises
  text;
- historical browser and Pico booleans for `flow/pulse` and
  `presentation/show` conflated these generic revisions with narrower
  `conduit-signal` contracts; the audited catalog now reports every platform
  manifestation flag as false.

No catalog kind is classified `kernel-native and proven`, `contract-only`,
`fixture-only`, or `placeholder`: the architectural defects take precedence as
the single required classification. The inventory still names the fixture-only
implementation path explicitly so it cannot be presented as production
support.

## Resulting executable nucleus

The executable nucleus among the eight audited `conduit.std/*@1` revisions is
empty. Their matching planner fixtures and `HostRuntime` implementations are
not production support.

S5 now begins with one separate revision: `conduit.std/time-tick@2`. It emits
only `value/tick@1` on its exact `tick` port, accepts bounded `count` and
`period-ms` configuration, waits through the admitted timer host-operation
contract, and completes after exactly the configured count. The std host offer
binds `std/kernel-time-tick@2` and
`conduit-std-host/time-tick@2`; preparation resolves that implementation from
a static installed table before activation, lowers the exact fragment, and
executes it through `conduit-kernel`.

The UI-independent conformance vector uses a `cfg(test)` kind named
`conduit.test/tick-observer`. That fixture is only a typed sink for observing
ordered tick payloads; it is neither advertised by production builds nor a
second supported standard kind. With a capacity-one cord, the vector proves
three ordered ticks, three configured waits, pressure, exact request and
terminal evidence identities, stable preallocated storage, and zero allocation
after activation. A zero-count vector completes without a wait or value, and
mutation plus cancellation vectors reject stale executable identity and late
timer completion before either can become success.

This proof does not promote `conduit.std/time-tick@1`, the other seven audited
compatibility rows, the sealed six-node multi-value profile, browser/Pico
manifestation, dynamic provider installation, or general graph installation.

This does not mean S5 starts without evidence. The narrower profiles below have
already earned relevant kernel behavior, but S5 must deliberately adopt or
rearticulate an exact contract instead of transferring proof by semantic kind
name. The strongest inputs are typed `flow/pulse` and `presentation/show` over
`value/signal`, plus typed `time/tick`, `flow/tee`, `state/latest`, and
`presentation/show` over `value/tick@1`. The generic `flow/map`, `flow/filter`,
and `text/format` rows are not nucleus candidates; concrete operations such as
`text/uppercase` avoid their numeric-selector ambiguity.

## Existing proof that must remain separate

Some semantic kind IDs also appear in narrower, current profiles:

- `conduit-signal` defines typed `value/signal` revisions for `flow/pulse` and
  `presentation/show`, with std, browser, and Pico kernel/platform proofs at the
  proof classes recorded in `STATUS.md`.
- `hosts/std::kernel_multivalue` defines typed `value/tick@1` revisions for
  `time/tick`, `flow/tee`, `flow/filter-even`, `state/latest`, and
  `presentation/show`, with bounded hosted kernel conformance.

Those profiles differ in contract revision and exact port types. Their proof is
useful input to rearticulation, but it does not promote a
`conduit.std/*@1`/`value/any` row.

## S5 stop line

This audit adds no kinds and changes no parser, planner, runtime, kernel, or
platform semantics. S5 should rearticulate one concrete kind per reviewed pull
request, beginning with the sequence owned by issue #353. Each promoted item
must bind exact typed ports, limits, lifecycle and terminal behavior to an
installed implementation that plans, lowers, executes, and records evidence
through `conduit-kernel`.

The compatibility catalog and its `HostRuntime` receipts may remain as
historical fixtures until a separately owned cleanup removes or fences them.
They must not be used in generated/status-facing output as evidence of current
standard-library support.

## Audit checks

```text
cargo test -p conduit-std-catalog --test truth_inventory
cargo test -p conduit-std-catalog
cargo check -p conduit-std-catalog --no-default-features --target thumbv6m-none-eabi
```
