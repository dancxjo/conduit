# Panel interface declarations and implements references

Status: candidate

Panel grammar version: 0

Source-AST schema version: 0

## Purpose and boundary

This specification defines the lossless, deterministic source syntax for declaring named node interface contracts and referencing them via `implements` claims within `.panel` source modules.

It builds on:
- Specification 014 / 015 (`.panel` source and typed source lowering);
- Specification 052 (`NodeInterfaceContract` identity and satisfaction algebra); and
- the current source semantic identity (`conduit.panel-source`).

The source grammar addition connects authored `.panel` documents to named node boundaries without introducing inheritance, macro substitution, or runtime state.

## Grammar extensions

### Interface declaration syntax

Top-level declarations permit `interface` blocks:

```ebnf
InterfaceDeclaration ::= "interface" Word "{" InterfaceMember* "}"
InterfaceMember      ::= DirectionalName ":" Word ["optional"]
```

Example:
```panel
panel 0

interface speech/recognizer {
    > audio : audio/pcm-stream
    > cancel : conduit/cancellation
    partial > : speech/transcript-delta optional
    final > : speech/transcript
    fault > : speech/asr-fault
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
panel 0

node local : tongues/whisper implements speech/recognizer

node moderated() implements speech/recognizer {
    node child : tongues/whisper
    export > audio = child.audio
    export > cancel = child.cancel
    export partial > = child.partial
    export final > = child.final
    export fault > = child.fault
}
```

#### Portable bounds
- `MAXIMUM_INTERFACE_CLAIMS` = 32 per node or composite boundary. Exceeding returns `CND-SEC-001`.

#### Uniqueness
Duplicate interface references in a single `implements` clause return diagnostic `CND-SRC-002`.

## Lossless source representation and AST identity

1. **Concrete Syntax Tree (CST)**: All comments, whitespace, and formatting within `interface` declarations and `implements` clauses are preserved losslessly in `SourceDocument::tokens` and round-tripped bit-identically by `SourceDocument::round_trip()`.
2. **Semantic source hash**: Formatted AST serialization includes interface declarations and implements claims. `semantic_source_hash` uses the `conduit.panel-source\0` domain. Formatting, trivia, and span changes do not alter the semantic hash.

## Diagnostics

- `CND-SRC-001`: Malformed syntax in `interface` or `implements` block.
- `CND-SRC-002`: Duplicate interface ID, duplicate member within interface, or duplicate claim.
- `CND-SRC-003`: Qualified interface claim `alias.interface` absent from imported module.
- `CND-SEC-001`: Interface declaration count (> 256), member count (> 64), or claim count (> 32) exceeds security ceiling.
