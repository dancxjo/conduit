# Conduit directional and principal-path syntax current form

Status: C3 normative source contract

The current pre-release Panel marker is `0`. Directional boundary declarations
remain logographic. Graph connections use `>` exclusively; there is no second
connection keyword or arrow spelling.

## Directional declarations

```ebnf
receiving-declaration = ">", name | name, "<" ;
outgoing-declaration  = name, ">" | "<", name ;
directional-name      = receiving-declaration | outgoing-declaration ;
interface-member      = directional-name, ":", qualified-name,
                        [ "optional" ] ;
export                 = "export", directional-name, "=", endpoint ;
graph                  = graph-term, ">", graph-term,
                         { ">", graph-term }, [ cord-policy ] ;
endpoint               = name, [ ".", member ] ;
```

`input` and `output` have no implicit directional meaning and remain ordinary
contextual identifiers. Parsing never consults a catalog to determine a
direction.

## Principal paths

One exact semantic interface descriptor may name at most one principal
receiving member and one principal outgoing member. Bare use in producing
position projects only through its declared outgoing member; bare use in
receiving position projects only through its declared receiving member.

```panel
voice > speak.voice
sentences > speak > play
```

With descriptor-backed proof, lowering may produce:

```text
voice.current > speak.voice
sentences.sentences > speak.text
speak.audio > play.audio
```

This is projection, not inference. Declaration order, unconnected members,
type coincidence, import order, provider state, implementation shape, and host
observation cannot choose a port. An absent or ambiguous principal member fails
with `CND-LWR-016`; the source must then spell the named endpoint.

Adding or connecting an auxiliary port cannot change an existing principal
path. A composite may publish a principal path only through explicit exported
boundary members. No shorthand may bypass a hidden child.

## Exact downstream identity

Source shorthand never reaches execution as an anonymous/default port. The
lowered topology, exact plan, diagnostics, evidence subjects, inspection, and
Patchbay accessibility output carry full semantic endpoint paths and exact
contract identities. Each authored graph `>` retains its own source span.

At graph level `>` always means connect. Inside an expression island it always
means `GreaterThan`; those AST variants and spans are distinct before type
checking.

## Normative requirements

| ID | Obligation |
|---|---|
| PDS-001 | Use `>` as the only current graph connection operator |
| PDS-002 | Preserve direction explicitly in semantic boundary contracts |
| PDS-003 | Project a bare endpoint only through one exact principal path |
| PDS-004 | Never infer direction or principal members from names, types, order, providers, or hosts |
| PDS-005 | Preserve complete named endpoints and source provenance downstream |
| PDS-006 | Require explicit named auxiliary endpoints and prevent composite child bypass |
| PDS-007 | Distinguish graph and expression `>` by parse context and source span |
