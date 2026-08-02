# Conduit `.panel` source grammar and modules current form

Status: C3 normative source contract

This document defines the only repository-supported `.panel` grammar. The
pre-release marker remains `panel 0`; it is a marker for the current draft, not
a compatibility promise. Git history contains displaced draft spellings.

Parsing creates source structures, spans, and diagnostics only. It does not
select implementations, inspect hosts, allocate queues, execute nodes, record
evidence, or create presentation state. Source bytes, lossless CST, semantic
source AST, resolved module graph, lowered semantic contracts, exact plan, run
evidence, and Patchbay presentation retain distinct identities.

## Encoding and names

Source is UTF-8. Identifiers use Unicode XID start and continuation characters,
plus the grammar punctuation used by semantic namespaces and temporal types.
Identifier spelling is normalized to NFC using the pinned
`unicode-normalization 0.1.25` implementation. Two distinct spellings that
normalize to the same identifier fail with `CND-SRC-011`; the lossless CST
still retains the exact submitted bytes.

Spaces, tabs, carriage returns, line feeds, and semicolons separate statements.
`#` begins a comment through the next line feed. A chain may continue across a
newline around `>`. Strings retain their Unicode contents without identifier
normalization.

```ebnf
xid-start       = Unicode-XID-Start | "_" ;
xid-continue    = Unicode-XID-Continue | "_" ;
name            = xid-start, { xid-continue | "-" } ;
qualified-name  = name, { "/", name }, [ "@", name ] ;
member          = name, { "[", name, "]" } ;
endpoint        = name, [ ".", member ] ;
temporal-type   = qualified-name, [ "...", [ "|" ] ]
                | "$", qualified-name ;
number          = digit, { digit } ;
integer         = [ "-" ], number ;
string          = '"', { string-character | escape }, '"' ;
separator       = newline | ";" ;
```

`T` is one ordinary value, `T...|` is a flow with a normal closing boundary,
`T...` is an open flow, and `$T` is an immediately available current value
whose newest replacement is retained. Closing alone implies no count, byte,
duration, or progress bound. `$T` grants observation only: it is not mutation
authority, history, durability, multi-writer state, or a CRDT.

## Current grammar

```ebnf
document        = "panel", "0", { statement } ;
statement       = import | interface | declaration | definition | graph
                | root | port-group | pool | supervision ;

declaration     = name, ":", qualified-name,
                  [ "using", qualified-name ], [ implements ], [ config ]
                | name, "=", source-value ;
implements      = "implements", qualified-name,
                  { ",", qualified-name } ;
config          = "{", { name, "=", source-value }, "}" ;

definition      = qualified-name, [ parameters ], [ implements ],
                  "{", { definition-statement }, "}" ;
parameters      = "(", [ parameter, { ",", parameter } ], ")" ;
parameter       = name, ":", qualified-name, [ "=", source-value ] ;
definition-statement
                = declaration | graph | export | binding | port-group
                | pool | supervision ;

graph           = graph-term, ">", graph-term, { ">", graph-term },
                  [ cord-policy ] ;
graph-term      = endpoint | source-value | qualified-name
                | expression-stage ;
expression-stage
                = ( "keep" | "map" | "stop" ), "{", expression, "}" ;

expression      = primary, { binary-operator, primary } ;
primary         = source-value | name | "(", expression, ")" ;
binary-operator = "+" | "-" | "*" | "/"
                | "<" | "<=" | ">" | ">=" | "==" | "!=" ;

cord-policy     = "{", { cord-field }, "}" ;
cord-field      = "capacity", "=", number
                | "max_value_bytes", "=", number
                | "max_queued_bytes", "=", number
                | "low_watermark", "=", number
                | "high_watermark", "=", number
                | "pressure", "=", pressure-name
                | "coalescer", "=", qualified-name
                | "sample_every", "=", number
                | "sample_offset", "=", number ;
pressure-name   = "block" | "reject" | "coalesce" | "sample"
                | "drop-disposable" | "disconnect" | "fail" ;

source-value    = "true" | "false" | integer | string | qualified-name
                | literal-call ;
literal-call    = "bytes", "(", string, ")"
                | "ref", "(", ( string | qualified-name ), ")"
                | "contract", "(", ( string | qualified-name ), ")"
                | "secret", "(", ( string | qualified-name ), ")"
                | "decimal", "(", string, ")"
                | "list", "(", [ source-value,
                  { ",", source-value } ], ")"
                | ( "record" | "map" ), "(",
                  [ record-field, { ",", record-field } ], ")" ;
record-field    = name, "=", source-value ;

interface       = "interface", qualified-name, "{",
                  { directional-name, ":", qualified-name,
                    [ "optional" ] }, "}" ;
directional-name
                = ">", name | name, "<" | name, ">" | "<", name ;
export          = "export", directional-name, "=", endpoint ;
binding         = "bind", name, "=", endpoint ;
root            = "root", qualified-name ;

port-group      = "port-group", directional-name, ":", qualified-name,
                  ( "indexed", "max", number
                  | "keyed", "max", number, "{",
                    "member", name, { "member", name }, "}" ) ;
pool            = "pool", name, ":", qualified-name,
                  "{", { name, "=", source-value }, "}" ;
supervision     = "supervise", name, "with", name ;

import          = "import", string, "as", name, [ "pin", string ]
                | "import", qualified-name, package-selection ;
package-selection
                = "as", name
                | "/{", package-name, { ",", package-name }, "}" ;
package-name    = name, [ "as", name ] ;
```

The parser context determines every operator before type checking. In
`ages > keep { it > 18 } > adults`, the outer operators are graph connections
and the inner operator is `GreaterThan`. Catalog contents, operand types,
providers, and host observations cannot change that classification.

## Concise declarations and chains

`:` declares a logical semantic instance. It does not allocate it or select an
implementation. `=` supplies an immutable literal/configuration binding.
`>` creates a bounded semantic cord. Explicit cord policy remains available;
omitted policy uses the current finite source defaults and is resolved exactly
before execution.

```panel
panel 0

answer-aloud {
  voice: controls/voice
  generate: llm/generate
  sentences: text/sentences
  speak: speech/synthesize
  play: audio/play

  voice > speak.voice
  "Explain photosynthesis simply." > generate > sentences > speak > play
}
```

A bare endpoint is source shorthand only. Lowering must obtain the required
receiving or outgoing member from one exact semantic interface descriptor's
principal path. If that proof is unavailable or ambiguous, lowering fails with
`CND-LWR-016` and the author must spell `instance.member`. Lowered graphs,
plans, diagnostics, evidence, inspection, and accessible presentation always
retain the complete named endpoint and source provenance.

An inline namespaced stage receives a deterministic semantic occurrence
identity derived from semantic AST order, never byte offsets or formatting.
Use a named declaration when it needs cross-reference, independent
configuration, multiple cords, or identity stable across structural edits.

## Definitions, roots, groups, and pools

A definition is an ordinary composite contract at its boundary. Parameters are
typed configuration, never live ports. `using ready` remains an unresolved
semantic constraint and never a stored implementation selection.

Every name is unique in its owning namespace. One declared root selects itself;
multiple roots require caller selection or fail with `CND-SRC-006`. Child
access through an unexported composite boundary fails with `CND-SRC-009`.

Port groups and instance pools retain their finite current contracts. A keyed
group has unique keys and a positive maximum. An indexed group expands only
`0..maximum`. Pools require positive maximum, finite deadlines and idle
timeouts, explicit admission, supervision, and cleanup. Zero or unbounded
forms fail with `CND-SRC-008`.

## Modules and package imports

Parsing performs no I/O. Module resolution consumes an explicitly supplied
`ModuleLoader`, normalizes paths lexically, rejects escape above a URI root,
checks exact optional SHA-256 pins, visits dependencies in source order, and
rejects cycles with the complete import chain. Package resolution similarly
consumes only caller-supplied immutable package bytes and exact lock data. It
does not fetch, install, select providers, or grant authority.

## CST, AST, formatting, and recovery

`SourceDocument` retains exact UTF-8 bytes, trivia, comments, tokens, and spans.
Its semantic AST contains only complete source forms. `format_panel` is an
explicit canonical formatter; it emits the concise current grammar and
preserves semantic source identity. Formatting never replaces the lossless CST
round trip.

Recovery may retain incomplete declarations and chains for editing, including
`a >`, but never produces an executable `Panel`. Invalid or partial recovered
state is presentation-only and lowering remains unavailable.

## Diagnostics

| Code | Meaning |
|---|---|
| `CND-SRC-001` | malformed current grammar |
| `CND-SRC-002` | duplicate symbol or field |
| `CND-SRC-003` | unresolved imported/local symbol |
| `CND-SRC-004` | module import cycle |
| `CND-SRC-005` | content pin mismatch |
| `CND-SRC-006` | invalid or ambiguous root selection |
| `CND-SRC-007` | unsupported panel marker |
| `CND-SRC-008` | zero or non-finite group/pool bound |
| `CND-SRC-009` | hidden composite child bypass |
| `CND-SRC-010` | invalid typed source literal |
| `CND-SRC-011` | Unicode normalization collision |
| `CND-SRC-012` | invalid supervision relationship |
| `CND-SEC-001` | source byte/token/item bound exceeded |
| `CND-SEC-002` | source-value nesting bound exceeded |
| `CND-LWR-016` | bare endpoint lacks one exact principal-path proof |

## Normative requirements

| ID | Obligation |
|---|---|
| PNL-001 | Accept one current pre-release grammar under `panel 0` |
| PNL-002 | Retain exact bytes separately from normalized semantic identifiers |
| PNL-003 | Parse graph and expression operators from grammar context only |
| PNL-004 | Lower bare endpoints only through exact principal-path descriptors |
| PNL-005 | Keep every live cord finite and pressure behavior explicit in semantics |
| PNL-006 | Keep providers, devices, hosts, artifacts, resources, and authority out of source semantics |
| PNL-007 | Preserve advanced definitions, roots, imports, interfaces, groups, pools, and supervision without a displaced reader |
| PNL-008 | Keep recovery non-executable and formatting explicit |
| PNL-009 | Reject normalization collisions deterministically |
| PNL-010 | Delete displaced declaration and connection spellings from repository-owned sources |
