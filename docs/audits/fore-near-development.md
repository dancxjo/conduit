# Absolute physical fore qualifications (development)

A fore such as `Frequency near 0.01Hz` now retains a checked physical source
kind and an absolute numerical tolerance. The qualifier accepts ordinary spaces
and tabs without changing its predicates or default byte bounds. It carries the existing `NearQuantity`
evidence record rather than silently treating a rounded target as exact
`Frequency`. This is development work for #5392; N5 #5534 and the parent remain
open until their full acceptance and exact-main gates are proved.

```conduit
plot accept (
    >> value: Frequency near 0.01Hz
    result: Frequency near 0.1Hz >>
) = .
```

The identity expression preserves the complete certificate. Its selected back
checks the incoming and outgoing predicates. A stronger result may satisfy the
weaker consumer; an inadequately certified result fails before it is emitted.
An exact alternative has zero numerical loss and can satisfy a tighter consumer.
A request or profile name alone never establishes the error claim.

The projection remains an ordinary explicit `math/near(limit=...)` Gear. Its
current fore takes `Quantity`, whose wire retains its physical family, unit and
role. The product fixture uses exact `1Hz / 3`, the explicit projection, a
`Frequency near 0.1Hz` consumer, and inspected source/error fields:

```sh
conduit run products/conduit/tests/fixtures/numerical_fore_near.conduit --await-terminal --report report.json
```

The actual product test requires one plan and one play, correlated observations,
matching source identity, the selected binary32 back, and the sealed predicate
work property. Installed-kernel tests additionally select exact rational and
binary32 realizations from one unchanged source document. They verify the retained
source, actual error, independent consuming tolerance, zero allocations after play
start, and unchanged value-store capacity.

## Retention and admission

The implementation extends the existing `CheckedValueContract` and its kind
semantic law. The canonical expression tree retains input/output contracts in a
value-contract boundary node. The selected back has that same callable contract;
planner equality remains intact. source inlining refuses a qualified fore whose
predicate disappears at an executable boundary. Other lowerings cannot silently
remove it.

Known physical source kinds must also agree across an identity expression or a
payload cord. A `Frequency near ...` packet cannot become `Distance near ...`
through their shared evidence carrier. These static mismatches refuse during
expansion; generic `Quantity` keeps its consuming runtime physical check.

Preparation validates definitions and owns the schema/storage allocations. play
uses borrowed numerical verification with finite work meters. Verification checks
canonical schema, source wire and mathematical coordinate, physical identity and
role, tolerance conversion, representation/rounding, and the recomputed error.
Original measurement uncertainty or empirical model confidence is not a numerical
error allowance. source-wire digests establish representation custody, not an
attestation of arbitrary producer execution.

The std offer seals and installation recomputes:

- the number of constraints and numerical validation work ceiling;
- retained evidence-schema bytes and canonical contract bytes;
- input/output byte ceilings covered by those validators.

Each `near` predicate has an independent 800,000,000-unit numerical meter; the
identity example has two, totaling at most 1,600,000,000 units. These are numerical
work counters, not elapsed-time guarantees. Schema/canonical payload figures
exclude allocator bookkeeping and the separately prepared expression tree and
input/output buffers. The certificate wire ceiling is 16,384 bytes. Missing,
modified or foreign facts, dropped semantic laws and stale programs refuse
installation.

Unqualified U64 identity programs retain their pre-existing v2/v3 canonical bytes.
The ordinary exact U64 addition fixture still selects the scalar tape and executes
without play allocation. There is no symbolic representation imposed per integer
operator. The new decoder reuses the existing value-contract identity encoding;
count, byte, definition and canonical-roundtrip checks bound preparation.

## Cost of checking the certificate

The retained optimized component capture uses the same real source-derived
one-third-Hz packet. It compares structural copying with copying plus two
numerical predicates. Structural copying does not establish fidelity and is not
an equivalent realization of the qualified fore.

| Prepared component | Median preparation CPU | Preparation allocations | Median warm CPU/value | Hot allocations |
| --- | ---: | ---: | ---: | ---: |
| Structural identity | 1.74 us | 5 | 6.13 us | 0 |
| Certified identity | 25.83 us | 341 | 134.01 us | 0 |

There are 32 trials per component. Every trial prepares a fresh evaluator in a
warmed process, then measures 1,024 structural or 64 certified calls. The native
capture uses Linux thread CPU and monotonic wall clocks, optimization level 3,
thin LTO and overflow checks. All calls preserve prepared capacity. The raw
records also retain wall times and allocation counts; the unoptimized test
capture is separate. This measures evaluator preparation and calls, not cold host
startup, whole-kernel throughput, WCET, another machine, or another target.
Certificate checking has substantial additional cost in this profile.

## Refusals and remaining scope

Tests cover negative and wrong-physical tolerances, implicit exact/qualified
connections, stronger-to-weaker acceptance, insufficient tighter results,
canonical forged targets, wrong-physical certificates, loss of a predicate during
source inlining, and a second projection that would discard incoming error.
The tighter binary32 case fails with output-predicate detail 214; its exact
alternative succeeds. Guarded prepared success and refusal loops allocate zero.

Browser offers explicitly refuse the new contract-boundary profile. A native
unit test executes that offer refusal; it is not browser numerical execution.
Installed no_std/device availability is unproved. The optional glyph alias is
outside this implementation.

General composite approximate arithmetic, two-operation intermediate-error
propagation and its cancellation-sensitive N5 cases remain unfinished. So do
other original #5392 cross-product gates. This bounded fore/profile slice does
not complete the parent or N5.

Local verification includes Core/Plot/Catalog libraries, installed fore tests,
actual CLI execution and numerical kernel integration. Complete command logs,
source identities, raw cost records and byte-custody manifests accompany the
validation receipt. Full local workspace testing has a retained disk-headroom
limitation from compilation attempts; no full local workspace pass is claimed.
Candidate, full integration and publisher acceptance remain separate gates.
