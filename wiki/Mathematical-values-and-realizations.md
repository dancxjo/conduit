# Mathematical values and numerical realizations

Numbers denote mathematical values. A consuming Plot declares numerical
requirements; the Planner chooses an eligible bounded realization before it
compares costs. Approximation is an inspectable projection.

`Scalar` is the canonical finite mathematical carrier. Ordinary decimal literals
are exact rationals: `0.1 + 0.2 == 0.3` is true. Reduced rationals, large integers,
the named constant `pi`, supported roots, and bounded symbolic expressions share
one canonical value protocol. Small integers use an inline representation.
General symbolic equality can be unknown; an unknown comparison is never an
ordinary false result.

Finite admission profiles bound limb count, expression nodes, depth, admitted
forms, and encoded bytes. A source bound such as `Scalar <= 2048` selects a
finite wire profile. A bounded representation is not a claim that all real
numbers or all symbolic identities are supported.

## Requirements are ordinary values

`math/evaluate` captures the original ordered computation and an independently
typed requirement. For example:

```conduit
plot main () {
    math/evaluate(
        value = 0.1 + 0.2,
        requirement = {
            semantics: mathematical(empty),
            representation: automatic(empty),
            rounding: nearestEven(empty),
            accuracy: exact(empty),
            budget: {
                storageBytes: 1048576,
                workSteps: 100000,
                limbs: 128,
                symbolicNodes: 128
            },
            parameters: [],
            reproducible: true,
            exceptions: {
                invalid: false,
                divisionByZero: false,
                overflow: false,
                underflow: false,
                inexact: false
            }
        }
    )
}.
```

Representations are `checkedInteger`, `rational`, `symbolic`, `interval`,
`binary64`, `ieeeBinary64`, and `automatic`. Automatic selection authorizes no
implicit loss: the original requirement remains part of every candidate proof.
Accuracy can be `exact(empty)`, `absolute(bound)`,
`relative({ relative: bound, zeroAbsolute: bound })`, `ulps(count)`, or
`enclosureWidth(bound)` where the selected semantics admit it.

Each offered algorithm carries a versioned identity and finite execution limits.
The numerical engine must prove eligibility before policy or cost can select it.
The Plan seals the original computation and requirement, selected algorithm,
result, error or enclosure, operational flags, and checked resource evidence.
An advertised algorithm name and a requested tolerance are not proofs.

Work is measured in `numeric/bounded-primitive-call@1` units. Each reviewed
primitive also has a finite internal work bound. These units are not processor
instructions or a latency measurement. Storage and limb/node capacities are
separate admission limits; an engine cannot silently substitute its larger
workspace for an offered smaller one. The Host offer also reserves runtime memory
for engine preparation, retained computation and evidence, verification copies,
and prepared output. The Planner checks this reservation before cost selection;
the Back checks the selected resource binding again during preparation. The
ledger describes bounded implementation storage, not an operating-system RSS
measurement.

## Preserve operation order

For `(10000000000000000 + 1) - 10000000000000000`, exact mathematical evaluation
is one. Ordered binary64 evaluation is zero because the intermediate addition
rounds. A binary64 candidate with an exact requirement refuses. A permitted
approximate candidate retains that intermediate rounding and its verified error.
Rounding only the exact final answer is a different algorithm.

Software binary64 records signed zero, NaN payload handling, infinity, rounding
mode, and invalid/division-by-zero/overflow/underflow/inexact flags. Explicit
`ieeeOperational` semantics require the `ieeeBinary64` representation and exact
operational behavior; a mathematical tolerance cannot authorize reassociation
or different IEEE bits. Ordinary mathematical values do not contain NaN or
infinity.

## Physical meaning and uncertainty

Unit laws and Quantity coordinates use the same mathematical values. `turn` is
Angle's reference unit; the radian scale is exactly `1 / (2 * pi)` relative to
turn. Unit and Quantity remain distinct values. Celsius point offsets apply to
temperature points, while TemperatureDelta conversions apply scale alone.
Captured unit definitions remain immutable through transport and later source
changes.

Trigonometry requires Angle. Bare numbers, Length, and cyclic Frequency are not
implicit radians. Cardinal identities can be exact; unsupported concrete
projections and undefined domains have typed refusals. Cyclic and angular
frequency require an explicit semantic operation even when their dimension
vectors resemble inverse time.

Measurement uncertainty retains observation provenance separately from the
numerical projection's error. Exact arithmetic on a sensor reading does not
remove uncertainty. The existing shared projection report records source
denotation, ordered computation, fidelity, range assumptions, exceptions, and
uncertainty; there is no second numerical-only projection authority.

Owned exact arithmetic and certification run during admitted preparation.
Prepared runtime outputs use their sealed finite storage. Domain-specific
fixed-point controls use an explicitly checked realization, `MicroScalar`; they
do not reinterpret the canonical Scalar bytes as a fixed-width grid.
