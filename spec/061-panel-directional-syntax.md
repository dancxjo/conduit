# Conduit logographic directional syntax

Status: C3 normative source contract

The current Panel grammar marker is `0`. Directional declarations use
logographic spelling and every cord endpoint names an explicit semantic member.
Direction remains a semantic-AST and PortContract fact; parsing never consults
a catalog, infers direction from a member name, or invents a port.

## Grammar

The lexical grammar includes the exact tokens `>`, `<`, `<-`, and `->`.
Longest-token matching distinguishes `<-` from `<`, and `->` from `>`.

```ebnf
receiving-declaration = ">", word
                      | word, "<" ;
outgoing-declaration  = word, ">"
                      | "<", word ;
directional-name      = receiving-declaration
                      | outgoing-declaration ;

interface-member      = directional-name, ":", qualified-name,
                        [ "optional" ] ;

export                 = "export", directional-name, "=", endpoint ;

port-group             = "port-group", directional-name, ":",
                         qualified-name,
                         ( "indexed", "max", number
                         | "keyed", "max", number, "{",
                           group-member, { group-member }, "}" ) ;

cord                   = "cord",
                         ( endpoint, "->", endpoint
                         | endpoint, "<-", endpoint ),
                         [ cord-policy ] ;
```

For a reverse cord, the first authored endpoint is the consumer and the second
is the producer. The semantic AST always stores producer then consumer.

The `<` declaration spellings and `<-` cord spelling are input equivalences.
Canonical examples and generated source use:

```text
> receiving
outgoing >
producer.semantic_output -> consumer.semantic_input
```

`input` and `output` have no directional meaning in the current grammar. They
may be ordinary identifiers where `word` is permitted. Every cord endpoint is
an explicit `instance.member`; an endpoint without a member is rejected even
when a contract has only one port.

## Semantic endpoint names

Published node contracts name the value or role carried, while direction stays
explicit in the complete PortContract. A source may have no receiving ports, a
sink may have no outgoing ports, and a valid node may have no ports.

Browser-visible text terminates at `display/text.text`. Process standard output
is the byte sink `io/stdout.bytes`. Text reaches it only through an explicit
checked encoder with receiving member `.text` and outgoing member `.bytes`.
Matching payload types never authorize an implicit adapter.

## Current-only acceptance

The parser accepts `panel 0` and this grammar only. English directional
declarations, missing endpoint members, and displaced Panel markers are
rejected. Repository-owned source was rewritten to the current form before
the displaced parser branches and one-time rewrite helper were deleted.

Malformed, missing, or doubled sigils and member-less cord endpoints fail as
`CND-SRC-001`. A document marker other than `panel 0` fails as `CND-SRC-007`.
No old source hash is selectable under the current hash domain.

## Normative requirements

| ID | Obligation |
|---|---|
| PDS-001 | Accept only `panel 0` and the current directional grammar |
| PDS-002 | Preserve direction explicitly in the semantic AST and PortContract |
| PDS-003 | Require an explicit semantic member on every cord endpoint |
| PDS-004 | Never infer direction from `input`, `output`, or another member name |
| PDS-005 | Keep display text distinct from process byte output |
| PDS-006 | Require an explicit checked encoder from text to bytes |
