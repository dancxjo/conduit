# Typed text formatting contract current form

Status: proposed portable contract

Depends on: specifications 003, 005, 006, 007, 008, 011, 012, 015, 022,
023, 035, and 046

## Boundary and identities

The only formatter semantic identity is `std/text/format`. Its public Panel
spelling is identical. The former provisional `std/format` config-only source
is not an alias, successor accepted by lowering, or second active contract.

`std/text/format` is an ordinary finite filter:

```text
template : std/text --------\
                              +-- std/text/format --> out : std/text
values : std/format-values --/
```

The `.panel` document, lowered source, type descriptors, node contract,
implementation manifest, artifact, host observation, exact binding, plan,
run, evidence, and Patchbay projection retain their separate identities.
Knowing this contract does not install its provider. `std/format-values/literal`
is the exact finite source specialization used by checked panels; it is not a
second formatter.

## Exact types

`std/text` is a sequence of Unicode scalar values encoded as UTF-8. No Unicode
normalization is implied. Invalid UTF-8 is rejected. Its exact schema-one
descriptor hash is
`sha256:79dd1d77e2cf6459bc3a8f96c65a915adc10db516dcac039f781bee5c1cab5ab`.
Every concrete value byte bound comes from its port and plan envelope.

`std/integer` is the mathematical signed integer. Its canonical semantic
encoding is minimal two's-complement big-endian. The semantic range is
unbounded; every host representation limit is a separate finite observation.
Crossing a selected representation boundary is a typed terminal outcome,
never wrapping, saturation, or truncation. Its exact schema-one descriptor
hash is
`sha256:161d9106bcff3ea645da0f89570c1c43fe87a50299f2725720dc5c75f10cd12e`.
The version-one Panel and formatter transport represent the inclusive
`i128::MIN..=i128::MAX` subset as exactly 16 signed big-endian bytes.

`std/format-values` is an ordered collection of at most 32 values. An entry
has an optional unique ASCII name of at most 64 bytes and exactly one scalar:
UTF-8 text of at most 1024 bytes, boolean, or the integer subset above. The
whole encoded value is at most 16,384 bytes. Its exact schema-one descriptor
hash is
`sha256:ba23e276b70b1b0c747d2b4ada100d72fa5b3874e4fa2baa250cf07149795cc0`.

The hosted version-one encoding is:

1. bytes `43 46 56 01` (`CFV` plus current form);
2. one unsigned byte entry count;
3. for each ordered entry, one name-length byte and those UTF-8 name bytes;
4. one kind byte: 1 text, 2 boolean, or 3 integer;
5. text: unsigned big-endian `u16` byte length then UTF-8 bytes; boolean:
   exactly 0 or 1; integer: exactly 16 signed big-endian bytes.

Unknown kinds, trailing bytes, invalid UTF-8, noncanonical booleans, duplicate
names, invalid names, and any exceeded bound fail closed. This encoding is a
hosted transport representation, not the type or node semantic identity.

## Placeholder grammar

Formatting operates on UTF-8 bytes without locale or ambient state:

- `{}` consumes the next ordered index;
- `{0}` and later canonical decimal indexes address collection order;
- `{name}` addresses one unique named entry;
- `{{` and `}}` emit literal braces.

An index has no leading zero except `0`. A name begins with ASCII letter or
underscore and continues with ASCII letters, digits, underscore, or hyphen.
Explicit indexed/named references do not advance the automatic index. A value
may be referenced repeatedly, but every supplied value must be referenced at
least once. Text is copied exactly, booleans become `true` or `false`, and
integers become locale-independent canonical base-10 text with an optional
leading minus.

There are no format specifiers, expressions, field traversal, conversion
hooks, locale rules, scripts, or implementation-language interpolation.

## Bounds, terminal behavior, and evidence

The node accepts exactly one finite value on each input and emits exactly one
finite value on success. It has no semantic configuration. Version-one
ceilings are:

- template: 4,096 bytes;
- values: 32 entries, 1,024 scalar bytes, 16,384 encoded bytes;
- output: 16,384 bytes;
- retained implementation state: 3 values and 36,864 bytes (both inputs plus
  one pressure-blocked output);
- work per step: 87,040 byte/comparison units, including worst-case named
  placeholder scans and duplicate-name validation;
- catalog evidence: 32 events.

Every live cord independently declares positive item, maximum-value,
maximum-queued-byte, watermark, and pressure bounds. Checked panels use
capacity one and blocking FIFO pressure. The exact execution profile exposes
two input leases, the retained-state ceilings, output and scratch ceilings,
work, memory claims, and one-tick bounded cancellation. The exact run rejects
evidence whose serialized bytes exceed the plan budget.

Success completes both inputs and the output normally. Cancellation emits no
formatted output and terminates as cancelled. The normalized failure codes
are:

- `format/template-too-large`;
- `format/invalid-text-encoding`;
- `format/too-many-values`;
- `format/name-too-large`;
- `format/invalid-name`;
- `format/duplicate-name`;
- `format/scalar-too-large`;
- `format/malformed-placeholder`;
- `format/missing-value`;
- `format/extra-value`;
- `format/unsupported-value-kind`;
- `format/invalid-values-encoding`;
- `format/output-overflow`.

Failures emit no output and terminate the formatter. They are never panics,
locale fallbacks, partial output, wrapping, saturation, or silent truncation.

## Stable requirements

- FMT-001: publish only `std/text/format` with two typed inputs and one text output.
- FMT-002: keep text, integer, and format-values descriptors exact and independently compatible.
- FMT-003: enforce the finite grammar and normalized outcomes without delegating interpolation.
- FMT-004: expose queue, byte, value, retained-state, work, cancellation, and evidence bounds.
- FMT-005: deterministic and hosted providers agree on normalized results.
- FMT-006: keep known contract, installed provider, current host resolution, and unsupported host distinct.
- FMT-007: remove the provisional config-only formatter atomically without an alias.
- FMT-008: prove the final ports through checked production execution and Patchbay projection.

The normative fixture is `conformance/c4/text-format.json`.
