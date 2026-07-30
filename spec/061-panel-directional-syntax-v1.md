# Conduit logographic directional syntax version 1

Status: C3 normative source contract

This document adds `.panel` grammar version 3. It changes only authored
directional declarations and cord spelling. Direction remains an explicit
semantic-AST field; source parsing does not consult a catalog, infer direction
from a name, or create a port.

Grammar versions 1 and 2 remain readable with their frozen source identities.
Version 3 selects source-AST schema 5. Its semantic hash domain is
`conduit.panel-source/v5`.

## Grammar

The lexical grammar adds the exact tokens `>`, `<`, and `<-`. The existing
`->` token remains indivisible. Longest-token matching distinguishes `<-` from
`<`, and `->` from `>`.

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

The `<` declaration spellings and `<-` cord are accepted input equivalences.
The canonical migrator emits only:

```text
> receiving
outgoing >
producer.port -> consumer.port
```

`input` and `output` have no directional meaning in version 3. They may still
be ordinary identifiers where `word` is permitted. Every cord endpoint is an
explicit `instance.port`; an endpoint without a member is rejected even when a
contract has only one port.

## Migration

`migrate_directional_syntax_v3` is deterministic and idempotent. It:

- changes the document declaration to `panel 3`;
- rewrites interface members, composite exports, and port groups to canonical
  one-glyph declarations;
- normalizes reverse cords to producer-to-consumer order;
- preserves comments, string contents, configuration, policies, explicit
  endpoint members, and all nondirectional declarations;
- reparses the result before returning it.

The migrator changes authored-source identity intentionally. Previously stored
schema 1 through 4 hashes remain selectable and are never reinterpreted under
schema 5.
## Diagnostics

Malformed, missing, or doubled sigils are source syntax failures
(`CND-SRC-001`). Using version-3 directional syntax in an older document is a
frozen-grammar failure (`CND-SRC-007`). A cord endpoint without an explicit
dotted member is a source syntax failure (`CND-SRC-001`).
