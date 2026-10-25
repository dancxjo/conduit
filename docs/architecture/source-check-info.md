# Portable Source/check Info

Issue [#6446](https://github.com/dancxjo/conduit/issues/6446) owns this
contract and reference projection. It is descriptive preparation, with no new
execution front door. The reference uses the canonical Rust parser, checker,
`SourcePreparationCache` and authoring expansion. There is no alternate grammar.

## Identity and custody

| Fact | Meaning |
| --- | --- |
| Source bytes | Exact immutable UTF-8, admitted at at most 1 MiB |
| Snapshot owner + generation | Source owner's nonzero revision; meaningful in that owner's namespace |
| SHA-256 | Content hash of those exact bytes, independent of generation |
| SourceDocumentId | Existing canonical parser identity; **not** the differently prefixed generic diagnostic helper identity |
| Check context | Exact immutable Startup, Profile and Back snapshots retained by the reference owner, with grammar and catalog/Kind/Fore/Type/law/Back revision names |
| Check outcome | Document passed or failed canonical checking |
| CheckedPlotId | Requested entry's existing checked identity, retained when that entry exists after successful checking |
| ExpandedPlotId | Requested entry's existing expanded identity, separately retained |
| Plan / Play | Explicitly absent; this operation never invokes a planner or execution kernel |

`SourceCheckContext` consumes owned catalog snapshots and makes them immutable.
`SourceCheckProjection` retains that context, exact Source and generation. A
request must match all Source facts and all context revisions. Wrong Source
refuses `StaleSource`; unknown catalog or revision refuses `UnknownContext`;
unsupported grammar refuses `UnknownGrammar`. The only supported grammar name
is `conduitese-v1`, the current canonical reference grammar.

Revision labels are the trusted source/catalog owner's descriptive provenance,
not a hash of compiler-private catalogs, a certificate, or authority. The owner
must allocate a fresh revision for any changed catalog input and may not reuse a
Source owner/generation for different bytes. A remote claimant cannot import a
context by submitting matching labels: the constructor requires actual canonical
Rust catalog values, and checking always uses those retained values. This API
is an in-process reference-owner extraction, not a portable context verifier.
An integration wanting authenticated context exchange must supply the existing
catalog/declaration custody, rather than trust this Info.

The complete context is the retained catalog snapshots, including private Types,
compiled laws, defaults, literal owners and Back provenance. The public revision
names cover all those inputs. An absent Kind, Fore or Type is an actual missing
input: canonical check/expansion reports its real refusal; no default catalog,
external lookup, partial successful candidate or unconstrained-law substitute is
installed. A document can pass checking while its requested entry cannot expand;
that result has `check=accepted`, the checked identity when the entry exists,
an absent expanded identity and an expansion error. A missing entry has neither
identity. Consumers must inspect all three facts.

## Version 1 projection

Existing Native Types have no Source snapshot, compiler-context revision,
diagnostic/candidate presence or check-result vocabulary. The new bounded Types
therefore add those descriptive records and their finite request/refusal variants,
while reusing existing scalar/record/variant/sequence primitives. They live in
[`semantics/language/source_check_info.conduit`](../../semantics/language/source_check_info.conduit).
Existing Text, finite sequences, option and Native record/variant Types suffice;
no new language primitive is needed. Source-check labels and snapshot/context,
normalized diagnostic and candidate records were missing, so those are the only
new semantic vocabulary. Rust's serializable `SourceCheckInfoV1` is an extraction
DTO, not a second compiler ontology. It has no deserializer into compiler-owned
checked values. Native names use the ordinary Conduitese variant spelling;
JSON uses the Rust schema's explicit kebab-case state tags and optional spans.
JSON is a descriptive reference view, not a canonical Native byte codec.

A future ordinary `source/check` Kind can accept `SourceCheckRequest` and emit
`SourceCheckInfoV1` or the explicit `SourceCheckRefusal` through exact typed ports.
It must use an already authorized immutable Source owner and installed checking
context. This ticket defines that contract only; it does not install a Back,
create a runtime compiler or acquire Source access. No new checking Form is
needed for the reference extraction.

The reference currently returns at most one canonical first error, rather than
claiming a complete multi-error recovery pass. It copies only code, summary,
Source identity/hash and the compiler-owned span when available. It rederives
UTF-8 byte extents and one-based Unicode scalar line/column positions using
`source_span`; invalid byte boundaries refuse rather than round or invent spans.
Expansion errors have no owned span and retain explicit absence. No prose is
parsed to recover locations or dependency names.

Diagnostics use the existing structured diagnostic validator (4 KiB per text
field), with caller-admitted limits at most 16 diagnostics and 16 KiB aggregate
retained diagnostic text. Dependency traces are the cache's actual table/key
queries, including misses, not declarations inferred from Source. The combined
check/expansion trace admits at most 1,024 rows and 128 KiB table/key text. Caller
limits may be smaller. Overflow refuses the entire projection with
`DiagnosticOverflow` or `DependencyOverflow`; there is no truncated success.
The reference cache's finite fallback retains `whole-context` provenance when
an exact read set is unavailable (including failed check/expansion attempts). That means conservative dependency on the
entire immutable context, never an empty dependency set.

All Source and catalog inputs have preparation bounds owned by their existing
canonical admission. This contract bounds its output and cache; it does not
claim a byte-accurate whole-compiler memory reservation or allocation-free
preparation. The cache is finite (four 1 MiB sources, four expansions each) and
never a runtime resource or authority cache.

## Boundaries and conformance

Private parser syntax, AST, checker internals, native law witnesses, and actual
checked/expanded owners stay in Rust. IDs and diagnostics grant no executable
owner, Host effect, Body membership, workset edit, catalog mutation, Plan or Play.
The projection takes no Body, workset, Host, authority, planner or kernel input;
only its private preparation cache changes. Caller mutation of an exported Info
cannot feed back into canonical owners. Browser SDK checked Source (#3987),
Source-only inspection (#6257), and native law custody (#6407) retain their own
admission routes. An identity string here cannot replace their checked token or
custody verification. Native abstraction #4375 provides the Type vocabulary.

Future language self-hosting may author the finite descriptive values and
reproduce parser/checker behavior. It must establish canonical context,
declaration/law custody and conformance independently before its outputs can
become trusted checked owners. This projection is no such admission.

Focused tests use real authored valid/error Sources, canonical parser/checker,
actual cache dependency reads and authoring expansion. They cover stable
serialization, separate identities, Unicode-safe spans, absent realization,
missing entries, stale/wrong Source, unknown grammar/catalog revisions and
transactional diagnostic/dependency overflow. The native Type contract itself
is checked by that same Rust checker. Focused developer verification is
`cargo +stable test -p conduit-plot --test source_check_info`; the supported
repository suite is `cargo +stable xtask check plot-s3` and ordinary affected
package Candidate admission. Broader runtime or native proof is not claimed by
these preparation fixtures.
