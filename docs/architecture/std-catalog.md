# `conduit.std` catalog truth boundary

`conduit-std-catalog` contains two deliberately separate sets:

- `supported_nucleus_contracts()` is the current nine-operation, exactly typed
  executable standard nucleus. The std reference host advertises these exact
  revisions and executes them through `conduit-kernel`.
- `standard_contracts()` retains eight pre-S5 compatibility contracts. Those
  revisions use `value/any`; their optional hosted fixtures run through
  `conduit-runtime::HostRuntime` and are not supported standard operations.

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

## Supported executable nucleus

The authoritative code inventory is `supported_nucleus_contracts()`. Every
entry has exact typed ports, startup configuration, finite limits, terminal
behavior, and one installed std-host realization selected before activation.

| Operation | Exact contract revision | Public contract and terminal behavior |
|---|---|---|
| `time/tick` | `conduit.std/time-tick@2` | optional bounded `count` and `period-ms`; closing `value/tick@1` flow; completes after `count` admitted waits |
| `time/every` | `conduit.std/time-every@1` | required `freq: Duration`; four-value closing `value/tick@1` flow; completes after four admitted waits |
| `presentation/tick` | `conduit.std/presentation-tick@1` | closing tick input; bounded stdout presentation; completes when input closes |
| `text/literal` | `conduit.std/text-literal@1` | bounded UTF-8 startup text to one `value/text@1` value |
| `text/upper` | `conduit.std/text-upper@1` | one bounded text value in/out; Unicode uppercase with overflow rejection |
| `text/join` | `conduit.std/text-join@1` | bounded prefix plus one bounded text value in/out; combined overflow rejects |
| `presentation/text` | `conduit.std/presentation-text@1` | bounded text input and stdout presentation |
| `state/count` | `conduit.std/state-count@1` | bounded startup count, closing tick input, current count output; completes when input closes |
| `presentation/count` | `conduit.std/presentation-count@1` | at most five current count observations presented to stdout |

The std host's `reference()` composition advertises exactly those nine
`conduit.std/*` revisions, plus the separately owned typed Signal family. Its
`minimal()` composition advertises none, and text/time/state families can be
selected independently. Runtime advertisement—not compilation or category
membership—is planner truth.

UI-independent conformance includes the typed tick observer vector and the
checked-in canonical Programs 1–4. Those programs compose the text, time, and
state families through lossless source checking, exact checked-face planning,
static installed-operation resolution, the existing kernel, admitted timer or
presentation effects, terminal results, and bounded evidence. The canonical
tests also reject malformed values, bound overflow, temporal mismatch, and
selected-realization mutation before effects.

All nine contracts and their catalog data remain `no_std` compatible. Their
installed execution adapters are hosted separately under `hosts/std`. Their
browser and Pico manifestation flags remain false: compatible faces or related
Signal operations do not transfer implementation proof.

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

Adding another supported operation requires its own reviewed typed contract,
installed realization, planning/lowering proof, bounded kernel execution, and
terminal evidence. A name, matching face, compatibility fixture, or narrower
platform implementation does not transfer proof provenance.

The legacy compatibility catalog and its `HostRuntime` receipts may remain as
historical fixtures until a separately owned cleanup removes or fences them.
They must not be used in generated/status-facing output as evidence of current
standard-library support.

## Audit checks

```text
cargo test -p conduit-std-catalog --test truth_inventory
cargo test -p conduit-std-catalog
cargo check -p conduit-std-catalog --no-default-features --target thumbv6m-none-eabi
```
