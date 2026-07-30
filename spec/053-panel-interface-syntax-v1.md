# Versioned panel interface declarations and implements references version 1

Status: stable

Panel grammar version: 2

Source-AST schema version: 4

## Purpose and boundary

This specification defines the lossless, deterministic source syntax for declaring named node interface contracts and referencing them via `implements` claims within `.panel` source modules.

It builds on:
- Specification 014 / 015 (`.panel` source and typed source lowering);
- Specification 052 (`NodeInterfaceContract` identity and satisfaction algebra); and
- Schema version 4 domain tag (`conduit.panel-source/v4`).

The source grammar addition connects authored `.panel` documents to named node boundaries without introducing inheritance, macro substitution, or runtime state.

## Grammar extensions

### Panel grammar v2 requirement

`interface` declarations and `implements` claims require `panel 2`. Authoring them in `panel 1` produces diagnostic `CND-SRC-007` ("`interface` requires `panel 2`; grammar version 1 is frozen"). Grammar version 1 remains frozen.

### Interface declaration syntax

Top-level declarations permit `interface` blocks:

```ebnf
InterfaceDeclaration ::= "interface" Word "{" InterfaceMember* "}"
InterfaceMember      ::= ("input" | "output") Word ":" Word ["optional"]
```

Example:
```panel
panel 2

interface speech/recognizer {
    input audio : audio/pcm-stream
    input cancel : conduit/cancellation
    output partial : speech/transcript-delta optional
    output final : speech/transcript
    output fault : speech/asr-fault
}
```

#### Portable bounds
- `MAXIMUM_INTERFACE_DECLARATIONS` = 256 per panel document. Exceeding returns `CND-SEC-001`.
- `MAXIMUM_INTERFACE_MEMBERS` = 64 per interface declaration. Exceeding returns `CND-SEC-001`.

#### Member uniqueness
Within an `interface` declaration, the combination of `(direction, member_id)` must be unique. Duplicate member keys return diagnostic `CND-SRC-002`.

### Implements reference syntax

`implements` claims may be authored on composite definitions and node instances:

```ebnf
CompositeDefinition ::= ("node" | "composite") Word [ParameterList] [ImplementsClause] "{" DefinitionBody "}"
NodeInstance        ::= "node" Word ":" Word [Constraint] [ImplementsClause] [ConfigBlock]
ImplementsClause    ::= "implements" Word ("," Word)*
```

Examples:
```panel
panel 2

node local : tongues/whisper implements speech/recognizer

node moderated() implements speech/recognizer {
    node child : tongues/whisper
    export input child.audio as audio
    export input child.cancel as cancel
    export output child.partial as partial
    export output child.final as final
    export output child.fault as fault
}
```

#### Portable bounds
- `MAXIMUM_INTERFACE_CLAIMS` = 32 per node or composite boundary. Exceeding returns `CND-SEC-001`.

#### Uniqueness
Duplicate interface references in a single `implements` clause return diagnostic `CND-SRC-002`.

## Lossless source representation and AST identity

1. **Concrete Syntax Tree (CST)**: All comments, whitespace, and formatting within `interface` declarations and `implements` clauses are preserved losslessly in `SourceDocument::tokens` and round-tripped bit-identically by `SourceDocument::round_trip()`.
2. **Semantic Source Hash v4**: Formatted AST serialization under schema version 4 includes interface declarations and implements claims. `semantic_source_hash_v4` uses domain prefix `conduit.panel-source/v4\0`. Formatting, trivia, and span changes do not alter the semantic hash.

## Diagnostics

- `CND-SRC-001`: Malformed syntax in `interface` or `implements` block.
- `CND-SRC-002`: Duplicate interface ID, duplicate member within interface, or duplicate claim.
- `CND-SRC-003`: Qualified interface claim `alias.interface` absent from imported module.
- `CND-SRC-007`: `interface` or `implements` authored under `panel 1`.
- `CND-SEC-001`: Interface declaration count (> 256), member count (> 64), or claim count (> 32) exceeds security ceiling.
