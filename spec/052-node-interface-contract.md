# Named node-interface contract current form

Status: normative current contract

NodeInterfaceContract descriptor schema marker: `0`

NodeInterface satisfaction proof schema marker: `0`

## Purpose and boundary

A `NodeInterfaceContract` is a stable, named semantic requirement for an
ordinary node boundary. It lets a consumer depend on a capability boundary
without depending on one primitive, composite, implementation, artifact, or
host.

The following identities remain distinct:

- a `TypeContract` defines domain-owned value meaning and compatibility;
- a `PortContract` defines one complete directional live boundary;
- a `NodeInterfaceContract` defines a finite required node boundary;
- a `NodeContract` defines one concrete primitive or composite boundary;
- an implementation supplies executable behavior for a concrete
  `NodeContract`; and
- a composite definition is authored topology whose transparent exports
  produce an ordinary `NodeContract`.

An interface is not a runtime node species, implementation ABI, host-service
trait, editable Panel object, catalog probe, or source-language vtable.
Claiming an interface does not select an implementation, insert an adapter,
grant authority, allocate a queue, or mutate topology.

## Exact reference and descriptor

`NodeInterfaceContractRef` contains:

- a portable namespaced `contract_id`;
- exact descriptor `schema_version`; and
- exact canonical `semantic_hash`.

The descriptor kind is `conduit/node-interface-contract`, current schema.
Its body contains the namespaced `contract_id`, an explicit principal path,
and a canonical set of member and non-port requirement hashes. Source order,
comments, spans, labels, and presentation do not participate. Every principal
endpoint, member, or requirement semantic change changes the identity.

current form permits at most 64 port members and 16 non-port requirements. The
caller supplies one hash slot per combined fact. Duplicate semantic keys,
invalid identifiers, unnamespaced interface or requirement IDs, invalid
descriptor references, an unsupported schema, and insufficient scratch fail
closed.

Interface revisions have distinct exact identities. current form defines no
implicit revision substitution or inheritance. A future compatible revision
needs a reasoned directional provider decision or explicit migration at the
owning catalog/lowering layer; matching the name alone is never sufficient.

## Port members

Each `NodeInterfaceMember` contains:

- `requirement = required | optional`; and
- one complete `PortContract`.

The port ID plus direction is the member key. The same local ID may occur once
in each direction because input and output remain distinct boundaries; a
duplicate in one direction is malformed.

Optionality has closed semantics: an optional member may be absent from the
candidate. If present, every complete `PortContract` fact must satisfy the
same directional rules as a required member. Optionality does not weaken the
nested port's presence, connection or value cardinality, delivery, temporal,
terminal, sensitivity, or flow guarantee. It cannot conceal a changed
required guarantee.

Extra concrete ports are permitted because the interface is a required public
projection, not a closed implementation shape. Every concrete input must
still be in the input slice, every concrete output in the output slice, and
each slice must have unique valid IDs. Extra ports create no hidden interface
member, adapter, connection, queue, or authority.

## Principal path

`PrincipalPath` contains at most one receiving member ID and one outgoing
member ID. Either side may be absent, so sources, sinks, duplex boundaries,
and boundaries that prohibit shorthand all remain explicit. A present ID must
name a required interface member in that exact direction. Missing,
opposite-direction, and optional members fail descriptor validation.

The path is the sole authority for projecting a bare source endpoint. Bare
use in receiving position projects to `receiving`; bare use in producing
position projects to `outgoing`. An absent side fails with
`principal-path-unavailable`. Projection never considers member declaration
order, the number of ports, type compatibility, unconnected ports, catalog
order, implementations, providers, or host observations.

Adding, removing, or reordering auxiliary ports cannot select or change a
declared principal endpoint. Changing the principal path changes the exact
interface identity and therefore invalidates stale references and dependent
proofs. A composite may publish a principal path only on its exported
boundary; a child port that is not explicitly exported cannot be projected.

Stable validation reasons are `interface-principal-member-missing` and
`interface-principal-member-optional`. These are descriptor failures, not
requests to guess another endpoint.

## Non-port requirements

`NodeInterfaceRequirement` provides a finite, domain-open mechanism for
configuration, lifecycle, authority/effect, and other complete semantic facts.
Each requirement contains:

- a namespaced requirement ID; and
- an exact required descriptor reference.

Every listed requirement is mandatory. Satisfaction needs exactly one
reasoned `CandidateSubstitutesRequired` decision over the exact required and
offered descriptors. Missing facts are indeterminate, incompatible facts are
incompatible, and duplicate decisions are ambiguous. The portable core
retains but does not reinterpret provider-owned descriptor meaning.

An unlisted non-port facet is explicitly deferred: the interface and proof
make no claim about it. In particular, an interface with no authority/effect
requirement cannot be cited as proof that a node is effect-free or authorized.
When an interface does carry an authority/effect requirement, a widening
candidate is rejected by the same directional compatibility algebra. A claim
never grants the required authority.

This form permits configuration and lifecycle requirements where a domain has
a complete descriptor and fixtures, without embedding their concepts or an
accidental partial matcher in `conduit-core`.

## Directional satisfaction

The query is:

> Can this complete concrete `NodeContract` be used wherever this exact
> `NodeInterfaceContract` is required?

Primitive catalog contracts and composite-derived contracts call the same
`assess_node_interface` function. The function:

1. validates and recomputes the exact interface reference;
2. validates the concrete node boundary without discovering implementations
   or hosts;
3. matches each member by exact ID and direction;
4. distinguishes absent, opposite-direction, and malformed/ambiguous members;
5. calls the existing `assess_port_substitution` function for each present
   member;
6. uses exact type compatibility locally and otherwise requires one
   caller-supplied provider decision;
7. validates each non-port requirement through the common directional
   compatibility decision;
8. records member and requirement proofs in caller-owned fixed scratch; and
9. computes an order-independent aggregate proof identity.

The aggregate outcome uses the existing three-valued algebra. Any
incompatible fact dominates. Otherwise any indeterminate fact makes the claim
indeterminate. Only complete compatible facts produce `compatible`. A source
claim is never proof by itself.

## Proof identity and retained operands

`conduit/node-interface-satisfaction-proof` current schema retains:

- exact interface reference;
- exact concrete `NodeContract` descriptor reference;
- every required member and matched offered `PortContract`;
- nested type and port decisions;
- every required and offered non-port descriptor plus its directional
  decision;
- stable member, requirement, aggregate outcomes and reasons; and
- an order-independent canonical proof identity.

The caller supplies one member-proof slot per member, one requirement-proof
slot per non-port requirement, and one proof-hash slot per combined fact.
Insufficient storage fails before admitting a claim.

Equivalent primitive and composite boundaries produce equal normalized member
proofs. Their aggregate proof identities remain distinct when their exact
concrete `NodeContract` references differ.

## Stable reasons

Member reasons are:

- `interface-member-satisfied`;
- `interface-optional-member-absent`;
- `interface-required-member-missing`;
- `interface-member-wrong-direction`;
- `interface-member-incompatible`;
- `interface-member-provider-unavailable`;
- `interface-member-indeterminate`; and
- `interface-member-type-decision-ambiguous`.

Non-port requirement reasons are:

- `interface-requirement-satisfied`;
- `interface-requirement-fact-unavailable`;
- `interface-requirement-incompatible`;
- `interface-requirement-indeterminate`;
- `interface-requirement-ambiguous`; and
- `interface-requirement-decision-invalid`.

Aggregate reasons are:

- `node-interface-satisfied`;
- `node-interface-required-member-missing`;
- `node-interface-member-wrong-direction`;
- `node-interface-member-incompatible`;
- `node-interface-provider-unavailable`;
- `node-interface-member-indeterminate`;
- `node-interface-requirement-fact-unavailable`;
- `node-interface-requirement-incompatible`;
- `node-interface-requirement-indeterminate`; and
- `node-interface-ambiguous`.

Descriptor, reference, candidate, and scratch failures have separate stable
`interface-*` error spellings exposed by the portable API.

## Conformance and compatibility

`conformance/c2/node-interface.json` freezes exact and refined success,
primitive/composite parity, optional absence and present-member rejection,
missing and wrong-direction members, complete non-type port mismatches,
provider availability and ambiguity, extra concrete ports, authority/effect
widening, duplicates, revisions, order-independent identity, semantic
mutation, and insufficient scratch.

This current descriptor form makes the principal path identity-bearing. It
does not reinterpret or change the identities of existing `TypeContract`,
`PortContract`, `NodeContract`, implicit-satisfaction proof, composite, or
exact-plan schemas.
Source grammar, module resolution, lowering, and plan retention are owned by
the ordered follow-up issues and must consume these exact identities and reason
codes.

## Normative requirements

| ID | Obligation |
|---|---|
| NIF-001 | Keep interface identity namespaced, exact, schema-versioned, and independent of nodes, implementations, artifacts, hosts, and presentation |
| NIF-002 | Define a finite boundary from complete directional PortContracts rather than labels or value types |
| NIF-003 | Give optional members absence-only semantics and fully validate any present member |
| NIF-004 | Use the existing directional port and type compatibility machinery rather than a parallel matcher |
| NIF-005 | Admit extra concrete ports only after validating a public, unambiguous concrete boundary |
| NIF-006 | Retain compatible, incompatible, and indeterminate outcomes with stable reasons and exact operands |
| NIF-007 | Require exact reasoned decisions for every declared non-port configuration, lifecycle, or authority/effect fact |
| NIF-008 | Treat undeclared non-port facets as unproven, never as an empty or permissive guarantee |
| NIF-009 | Reject missing, wrong-direction, incompatible, ambiguous, provider-unavailable, malformed, and stale/wrong-reference cases |
| NIF-010 | Give primitive and composite-derived NodeContracts the identical satisfaction path |
| NIF-011 | Make member, requirement, and source ordering non-semantic |
| NIF-012 | Use caller-owned fixed scratch and fail before admission when it is insufficient |
| NIF-013 | Insert no adapter, queue, authority, implementation behavior, topology, or runtime interface object |
| NIF-014 | Preserve TypeContract, PortContract, NodeContract, composite, implicit-satisfaction, and plan schemas; interface references and their proofs change when the principal path changes |
| NIF-015 | Make receiving and outgoing principal endpoints explicit, directional, and identity-bearing |
| NIF-016 | Reject missing, wrong-direction, or optional principal members rather than inferring another endpoint |
| NIF-017 | Project bare endpoints from the exact descriptor only, independent of order, types, connections, catalog, implementation, provider, and host state |
| NIF-018 | Permit composite principal paths only through explicit exported boundary members |
