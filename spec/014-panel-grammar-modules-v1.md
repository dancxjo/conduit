# Conduit `.panel` source grammar and modules version 1

Status: C3 normative source contract

This document freezes `.panel` grammar version 1. A panel is an authored
assemblage of ordinary node definitions and instances. Parsing creates source
structures, spans, and diagnostics only. It does not select implementations,
inspect hosts, allocate queues, execute nodes, record evidence, or create a
runtime `Panel` species.

Source bytes, lossless CST, semantic source AST, resolved module graph, lowered
semantic contracts, exact ExecutionPlan, run evidence, and presentation have
distinct identities.

## Encoding and lexical grammar

A source unit is UTF-8. Outside strings, version 1 accepts ASCII space, tab,
carriage return, and line feed as trivia. A `#` begins a comment through, but
not including, the next line feed. Comments and trivia are retained in the CST
and omitted from semantic source identity.

```ebnf
letter          = "A"…"Z" | "a"…"z" ;
digit           = "0"…"9" ;
word-start      = letter | "_" | "-" | "." | "/" | "@" ;
word-rest       = letter | digit | "_" | "-" | "." | "/" | "@"
                | "[" | "]" ;
word            = word-start, { word-rest } ;
number          = digit, { digit } ;
string          = '"', { string-character | escape }, '"' ;
escape          = "\", ( "n" | "r" | "t" | '"' | "\" ) ;
qualified-name  = word ;
endpoint        = word, ".", word ;
```

`number` is an unsigned 64-bit decimal integer. Individual productions impose
narrower portable bounds. A newline inside a string is preserved. Version 1
does not normalize Unicode string contents.

Names containing `/` are semantic namespaces. An imported name uses the exact
alias prefix `alias.symbol`. Group member syntax such as `routes[home]` remains
part of the port word after compile-time lowering; it does not denote a runtime
array-valued port.

## Syntactic grammar

Line breaks and indentation are trivia. Keywords and punctuation delimit
productions; version 1 has no semicolon insertion.

```ebnf
document        = "panel", number, { declaration } ;

declaration     = import
                | node-declaration
                | legacy-composite
                | cord
                | root
                | port-group
                | pool ;

import          = "import", string, "as", word, [ "pin", string ] ;
root            = "root", qualified-name ;

node-declaration
                = "node", word,
                  ( ":", qualified-name, [ constraint ], [ config-block ]
                  | [ parameter-list ], definition-body ) ;
legacy-composite
                = "composite", word, definition-body ;
constraint      = "using", word ;

parameter-list  = "(", [ parameter, { ",", parameter } ], ")" ;
parameter       = word, ":", qualified-name, [ "=", source-value ] ;

definition-body = "{", { definition-member }, "}" ;
definition-member
                = node-instance
                | cord
                | export
                | binding
                | port-group
                | pool ;
node-instance   = "node", word, ":", qualified-name,
                  [ constraint ], [ config-block ] ;

config-block    = "{", { word, "=", source-value }, "}" ;
source-value    = string | word | number ;

cord            = "cord", endpoint, "->", endpoint, [ cord-policy ] ;
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

export          = "export", direction,
                  ( endpoint, "as", word | word, "=", endpoint ) ;
binding         = "bind", word, "=", endpoint ;
direction       = "input" | "output" ;

port-group      = "port-group", word, direction, ":", qualified-name,
                  ( "indexed", "max", number
                  | "keyed", "max", number, "{",
                    group-member, { group-member }, "}" ) ;
group-member    = "member", word ;

pool            = "pool", word, ":", qualified-name, "{",
                  { pool-field }, "}" ;
pool-field      = word, "=", source-value ;
```

The pool field names and combinations are closed in version 1:

| Field | Rule |
|---|---|
| `maximum` | required positive `u16` |
| `admission` | required: `reject`, `block`, `queue_bounded`, or `fail` |
| `admission_queue` | required positive `u16` only for `queue_bounded` |
| `deadline_ms` | required finite `u64` |
| `idle_timeout_ms` | required finite `u64` |
| `supervision` | required: `fail_together`, `isolate`, `restart_bounded`, `fallback`, or `escalate` |
| `restart_attempts` | required `u16` for `restart_bounded` |
| `restart_backoff_ms` | required finite `u64` for `restart_bounded` |
| `fallback` | required qualified name for `fallback` |
| `cleanup` | required: `drain` or `abort` |

Underscore and hyphen spellings shown as alternatives by the parser are
lexical compatibility aliases only. Canonical source semantics use the enum
meaning, not the spelling.

The modern export form keeps direction explicit:

```panel
export input mic.trigger as trigger
export output tts.audio as audio
```

This is the deliberate grammar decision corresponding to the shorter
illustrative `export mic.trigger as trigger`: parsing cannot inspect a selected
implementation to infer a port direction.

## Definitions, instances, and roots

A source unit may contain multiple reusable definitions. A definition remains
an ordinary node contract at its boundary. Parameters are typed configuration,
not ports. `using ready` is an unresolved constraint and never a stored
implementation selection.

Every instance, definition, parameter, group, pool, import alias, export, and
binding is unique in its owning namespace. Duplicate symbols produce
`CND-SRC-002`.

`root name` declares a selectable definition or top-level instance. One
declared root selects itself. More than one declared root requires an explicit
caller selection; absence or mismatch produces `CND-SRC-006`. Imported source
units may publish multiple roots because importing does not execute or select
one. Legacy version 1 files with top-level instances and no root remain
parse-compatible; they preserve their already-explicit top-level graph and do
not synthesize a hidden root.

An endpoint must name an instance in its current boundary. Direct access such
as `outer.inner.port` bypasses an export and produces `CND-SRC-009`.

## Compile-time groups and bounded pools

A keyed group has one or more unique authored keys and a positive finite
maximum. An indexed group expands deterministically to indices `0..maximum`.
The qualified name after `:` identifies one complete semantic `PortContract`,
not merely a value type. Lowering applies it to every member and produces
ordinary complete typed ports with stable derived identities. Member
insertion, removal, reordering where order is semantic, or maximum changes
source and lowered semantic identity.

A pool names a template and every finite admission, deadline, idle,
supervision, restart, and cleanup fact needed by later lowering. No callback or
configuration array may hide handlers. Queue-bounded admission has an explicit
positive queue maximum. Pool and group zero/unbounded forms produce
`CND-SRC-008`.

Parsing does not allocate a pool or lower group members. The current hosted
runtime fails with `CND-PLN-005` if imports, roots, constraints, groups, pools,
or parameterized definitions reach it without the explicit compiler/lowering
stage.

## Modules, aliases, and content identity

Import resolution is an explicit operation over a caller-supplied
`ModuleLoader`. The parser and resolver perform no implicit filesystem or
network access.

Resolution applies these rules:

1. An absolute path or URI is normalized lexically.
2. A relative target is joined to the importing canonical URI.
3. `.` is removed and `..` is rejected when it would escape the URI root.
4. The loader must return the exact requested canonical URI.
5. UTF-8 source bytes receive `sha256:<hex>` content identity.
6. A `pin` must equal that identity or resolution fails with `CND-SRC-005`.
7. Imports are visited in source order and emitted dependency-first.
8. Duplicate aliases fail with `CND-SRC-002`.
9. Missing modules or qualified symbols fail with `CND-SRC-003`.
10. A cycle fails with `CND-SRC-004` and the complete ordered cycle path.

The resolved graph records canonical URIs, exact content hashes, parsed source
ASTs, and entry-root selection. It is not an ExecutionPlan or lockfile.
Packaging tooling may persist those identities in a separate lock artifact;
parsing never silently updates a pin.

## CST, semantic AST, and compatibility

`SourceDocument` retains every UTF-8 byte as contiguous CST tokens with exact
byte and line/column spans. `round_trip()` returns the original source. Its
separate AST excludes comments, trivia, and spans. The stable
`semantic_source_hash` domain-separates and hashes normalized AST fields, so
equivalent formatting has equal identity.

Version 1 parsers MUST continue accepting the frozen valid fixtures. Adding a
keyword in a location previously accepted as a `word` is potentially breaking.
New optional syntax is compatible only when old documents keep the same AST.
Changing defaults, implicit bounds, name resolution, or diagnostic meaning is
a grammar-version change.

## Diagnostics and recovery

| Code | Meaning |
|---|---|
| `CND-SRC-001` | lexical or syntactic error |
| `CND-SRC-002` | duplicate source symbol |
| `CND-SRC-003` | missing module, alias target, qualified symbol, or source reference |
| `CND-SRC-004` | import cycle |
| `CND-SRC-005` | content pin mismatch |
| `CND-SRC-006` | absent, ambiguous, or unknown root selection |
| `CND-SRC-007` | unsupported grammar version |
| `CND-SRC-008` | zero, missing, overflowed, or unbounded group/pool maximum |
| `CND-SRC-009` | inaccessible child or boundary bypass |

Recovery never guesses semantics. The lossless scanner continues through the
complete byte string, so editors can always preserve and display malformed
source. The strict semantic parser reports the first stable diagnostic and
withholds the AST. An editor seeking additional diagnostics may synchronize at
the next top-level declaration keyword or the closing brace at the current
depth, but recovered declarations remain provisional and MUST NOT be lowered
until a strict parse succeeds.

## Conformance

`conformance/c3/panel-grammar-v1.json` is normative. It contains complete UTF-8
source cases rather than implementation snapshots. It covers every EBNF
production above, exact round trips, formatting equivalence, imports, aliases,
pins, cycles, duplicate symbols, malformed cords, boundary bypass, root
selection, compile-time groups, bounded pools, pressure policies, and
unsupported versions.

## Normative requirements

| ID | Obligation |
|---|---|
| SRC-001 | Parse only the exact versioned lexical and syntactic grammar |
| SRC-002 | Reject duplicate symbols in every owning source namespace |
| SRC-003 | Resolve imports deterministically through explicit caller-supplied bytes without implicit I/O |
| SRC-004 | Reject cycles with the complete deterministic import path |
| SRC-005 | Content-identify every module and enforce authored pins exactly |
| SRC-006 | Require explicit selection among multiple roots and never synthesize a hidden root |
| SRC-007 | Reject unsupported grammar versions without reinterpretation |
| SRC-008 | Keep every group, pool, admission queue, deadline, restart, and cleanup bound explicit and finite |
| SRC-009 | Permit boundary access only through explicit exports |
| SRC-010 | Preserve exact CST bytes/spans while keeping trivia outside semantic source identity |
| SRC-011 | Recover without guessing or lowering provisional semantics |
