# `conduit.std` catalog truth boundary

`conduit-std-catalog` exposes one v1 set: `supported_nucleus_contracts()`.
Every entry has exact typed ports and a current implementation offer. The std
reference Host advertises those exact revisions and executes them through
`conduit-kernel`.

The pre-v1 erased `value/any` catalog, numeric selector implementations, hosted
compatibility executor, and browser/Pico simulation runtimes were removed. They
are not discoverable aliases, optional features, or alternate execution paths.

## Supported executable nucleus

The authoritative inventory is `supported_nucleus_contracts()`. Every
entry has exact typed ports, startup configuration, finite limits, terminal
behavior, and one installed std-host realization selected before Play start.

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
| `file/copy` | `conduit.std/file-copy@1` | exact protected source/destination roles; bounded chunked copy through one admitted filesystem operation family |

The std host's `reference()` composition advertises exactly those ten
`conduit.std/*` revisions, plus the separately owned typed Signal family. Its
`minimal()` composition advertises none, and text/time/state families can be
selected independently. Runtime advertisement—not compilation or category
membership—is planner truth.

UI-independent conformance includes the typed tick observer vector and the
checked-in canonical Programs 1–4. Those programs compose the text, time, and
state families through lossless source checking, exact checked-face planning,
static installed-operation resolution, the existing kernel, admitted timer or
presentation effects, terminal results, and bounded Sign. The canonical
tests also reject malformed values, bound overflow, temporal mismatch, and
selected-realization mutation before effects.

All ten contracts and their catalog data remain `no_std` compatible. Their
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
useful input to rearticulation, but it does not create an alias for a retired
contract revision.

## S5 stop line

Adding another supported operation requires its own reviewed typed contract,
installed realization, planning/lowering proof, bounded kernel execution, and
terminal Sign. A name, matching face, or narrower platform implementation does
not transfer proof provenance.

## Audit checks

```text
cargo test -p conduit-std-catalog
cargo check -p conduit-std-catalog --no-default-features --target thumbv6m-none-eabi
```
