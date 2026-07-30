# Bounded text lines and join v1

Status: candidate normative C4 contract

Depends on: specifications 003, 005 through 008, 011, 012, 022, 023, 046,
054, and 059.

## Purpose and boundary

This specification defines two graph-worthy text filters:
`std/text/lines` splits bounded UTF-8 text into logical lines, and
`std/text/join` joins one finite bounded sequence of text items. Both use the
canonical `std/text` identity. They do not normalize Unicode, change case,
apply locale rules, collate, match regular expressions, or perform rich
templating.

Known semantic contracts, installed implementations, host observations,
resolved exact plans, execution evidence, and Patchbay presentation remain
separate identities.

## `std/text/lines`

The input port `in` accepts one finite `std/text` value. The output port
`line` emits zero or more ordered `std/text` values. An implementation may
receive that value in arbitrary byte chunks; those write boundaries have no
semantic identity.

The provider treats LF as a delimiter and treats CRLF as one delimiter,
including when CR and LF arrive in different implementation chunks.
Delimiters are not retained. Consecutive delimiters emit empty lines. Terminal
input emits a non-empty unterminated prefix; empty terminal input invents no
line. Input chunking MUST NOT change normalized values, order, errors, or the
terminal result.

UTF-8 is validated for each complete logical line. A code point may cross
implementation chunks. Invalid UTF-8 terminates with `CND-TXT-002`; no
normalization is performed.

`maximum_line_bytes` and `maximum_retained_prefix_bytes` are required finite
unsigned bounds, each at most 1024 bytes. The provider retains at most that
prefix and emits no line larger than the authored line bound. Overflow
terminates with `CND-TXT-001`. The output cord owns pressure; this node neither
drops nor coalesces line values. Cancellation discards the retained prefix and
records ordinary ordered cancellation and terminal evidence.

## `std/text/join`

The input port `item` accepts a finite ordered stream of `std/text` values.
The output port `out` emits exactly one `std/text` value after terminal input.
The `separator` configuration is the separator's semantic identity; there is
no hidden separator input.

The required finite configuration bounds are:

| field | maximum |
| --- | ---: |
| `maximum_items` | 8 |
| `maximum_item_bytes` | 1024 |
| `maximum_separator_bytes` | 64 |
| `maximum_output_bytes` | 4096 |

Zero items emit the empty text value. One item emits itself. Many items retain
input order and place the separator only between adjacent items. Item,
separator, item-count, and output overflow terminate respectively with
`CND-TXT-005`, `CND-TXT-006`, `CND-TXT-004`, and `CND-TXT-007`.

The exact plan accounts for retained item handles, retained item bytes,
separator bytes, output bytes, cord queues, work, ticks, and evidence. An
open-ended input contract is incompatible and MUST be rejected at compile
time; the provider never waits while retaining an unbounded stream.
Cancellation discards retained items and records ordinary ordered cancellation
and terminal evidence.

## Providers and evidence

The hosted and deterministic reference providers execute these contracts
through the production exact executor and MUST agree on normalized success,
overflow, cancellation, pressure, and terminal evidence. Constrained profiles
that do not install them report honest unsupported availability; knowing the
contracts does not imply an installed provider.

Patchbay projects the exact node contracts, authored configuration, selected
provider, resolved bounds, cord occupancy and pressure, ordered values,
errors, cancellation, and terminal evidence. Its marble-like timeline is a
presentation of those same authoritative plan and evidence values, never a
second execution model. Keyboard operation, reduced motion, screen-reader
labels, and an ordered textual event table convey the same facts.

## Conformance

The normative cases are
[`conformance/c4/text-lines-join-v1.json`](../conformance/c4/text-lines-join-v1.json).
Complete checked examples are
[`examples/text-lines-join.panel`](../examples/text-lines-join.panel) and
[`examples/format-lines.panel`](../examples/format-lines.panel).
