# General-purpose computation under explicit finite bounds

Status: durable direction under the revised September 6, 2026 architecture
contracts; executable acceptance remains separately tracked.

Owners: [#2682](https://github.com/dancxjo/conduit/issues/2682),
[#2685](https://github.com/dancxjo/conduit/issues/2685),
[#2686](https://github.com/dancxjo/conduit/issues/2686),
[#2687](https://github.com/dancxjo/conduit/issues/2687),
[#2688](https://github.com/dancxjo/conduit/issues/2688),
[#2689](https://github.com/dancxjo/conduit/issues/2689),
[#2690](https://github.com/dancxjo/conduit/issues/2690), and
[#2691](https://github.com/dancxjo/conduit/issues/2691).

## Decision and provenance

Conduit targets general-purpose computation under explicit finite bounds.
Strict mathematical Turing completeness and semantically infinite memory are
not current requirements. Finite and admitted by default, and preferably always.

This corrects the earlier universal-extension premise in
[the note preserved at 7f7e6dea1](https://github.com/dancxjo/conduit/blob/7f7e6dea1d9dd6ad2b7123b1dcbf3fa7c58aec36/docs/architecture/decidable-default-universal-extension.md),
originally proposed in #2692 and admitted to dev through #2697. That premise is
a superseded experiment, not an active obligation. The path is retained to keep
existing issue links valid. Promotion #2701 was stopped before acceptance so
its old premise would not become the stable architecture claim.

Do not add a universal, unsafe, or unbounded mode to express large values,
long-running services, or general computation. Revisit mathematical
universality only through a new architecture decision motivated by a concrete
useful computation that cannot be expressed honestly with parameterized finite
bounds. It is an unresolved theoretical question, not a missing prerequisite
for useful Conduit software.

## Meaning, specialization, and realization

Every checked executable Form has explicit finite semantic/resource capacities
after specialization. A reusable Form may parameterize capacities instead of
copying an algorithm for each size. Checking chooses exact finite values;
checked/expanded identity retains those values. Planning then binds exact
finite implementation, storage, queues, outstanding operations, Resources,
Lines, mandatory Sign storage and work obligations before Play starts.

For example, a natural-number algorithm may use a capacity parameter of 32 or
4096 bytes, a syntax tree may admit two million nodes, and a database may admit
10 TiB. These are large finite instances, not implicit infinite domains.
Bounds belong to meaning or realization according to their contract. An
admitted buffer limit does not redefine an authored integer type, and an
integer-domain bound does not guarantee enough physical storage for a Play.

Increasing a semantic capacity can create a different checked specialization.
Increasing only realization storage need not change checked meaning. Neither
operation is permission for hidden allocation or automatic expansion during
Play. Exhaustion is explicit; a larger instance requires fresh checking or
admission as appropriate.

## Practical computational generality

The engineering target is to express computation executable on a finite
conventional machine with finite memory/input/output bounds as ordinary finite
Conduit composition. It does not require a grand theorem before useful slices
can land, and it does not allow a hidden Python/WASM interpreter Gear to stand
in for Conduit computation.

General typed State retains finite typed values with explicit initialization,
current/candidate-next semantics, ownership, transition evidence, reset,
cancellation, failure, and resource cost. Same-generation ordinary dataflow
remains acyclic. Recurrence crosses explicit State/delay boundaries.
Comparison, Boolean logic, selection, arithmetic, indexing, projection, and
bounded sequence/tree/map operations supply control and structured memory.

No ambient mutable variables, assignment model, goto, hidden callbacks,
truthiness, dynamic graph mutation, universal Any, or second execution language
is introduced. Any future syntax sugar must lower losslessly to the same graph.

The #2682 specimens must expose missing machinery: a bounded parser/compiler
pipeline and a bounded evaluator with data-dependent recurrence through
ordinary semantic composition. Reusable capacity parameters, retained State,
and actual execution are required; an opaque privileged VM is not acceptance.

## Finite does not mean tractable

A finite transition system permits exhaustive reachability in principle when
all retained domains and transitions are specified. Its state space may still
be far too large to enumerate. A 10 TiB finite resource is not a practical
model-checking task merely because its cardinality is finite.

Under #2686, preserve exact capacity facts separately from practical analysis
eligibility. Retain acyclic regions, explicit recurrence, finite-state
properties, known work bounds, proved termination, symbolic invariants, and
exact timing bases where available. Unknown evidence must remain unknown.
An unrelated large finite region must not erase smaller regions' useful facts.

Prefer shape/range proofs, capacity accounting, small finite transition-system
checks, bounded model checking, protocol proofs, and symbolic invariants where
useful. Whole-state enumeration is not the default safety criterion. A work
budget is not a proof of semantic termination.

## Continuous lifetime

A controller, server, compositor, sensor pipeline, audio graph or UI may remain
active indefinitely with finite graph and retained State, finite work per
admitted interaction, and bounded instantaneous queues and resources. The
environment need not announce a final input count. Quiescence means awaiting
input; it does not mean semantic completion.

Continuous execution is ordinary Conduit, requiring no computational opt-out.
The same explicit State transitions and one kernel carry continuing work. A
timer-owned scheduler or hidden restart loop cannot masquerade as continuity.
Each active Play must retain honest finite admission and pressure semantics.

## Results and bounds

The machine-readable vocabulary under #2690 must distinguish:

- semantic completion / HALT;
- quiescent / awaiting input;
- Lull / Body-level suspension;
- cancellation;
- value/domain overflow or semantic refusal;
- State capacity exhaustion;
- other resource capacity exhaustion;
- work/fuel budget exhaustion where a budget exists;
- failure;
- Host/Boot/Resource/Line loss;
- Plan retirement or replacement;
- continued operation.

Computing 256 in U8 is domain overflow. A valid semantic result that cannot fit
an admitted output buffer is resource exhaustion. Neither is HALT. A full
bounded collection uses its declared full/overflow contract. Refusal must not
be hidden through retries, wrapping, silent truncation or automatic replan.

## Replanning and State continuity

Distinguish authored Form family/source identity, checked/specialized Form
identity, State identity/generation, Plan identity, and Play identity. Replacing
a realization changes Plan/Play truth without necessarily changing checked
Form identity. Changing a semantic capacity may change the checked and expanded
identities even when the higher-level workload continues.

Under #2691, continuity transfer is finite, typed and explicitly admitted. An
approved larger specialization may receive eligible State only under an exact
semantic migration contract. Source/destination identities, State generations,
value types and capacities must be validated. Insufficient capacity, stale
State, or incompatible specialization refuses or exposes reset/loss; it must
not silently initialize and call that continuation.

A transfer does not carry old Boot grants, Resource bindings, or initialized
implementation authority into a replacement Play. Fresh truthful offers and
authority are required. There is no general checkpoint magic and no assumption
that all State is durable.

## Deadline regions

Finite semantic domain, finite resources, known worst-case operation count,
WCET on an exact realization, and deadline admission are separate facts. A
bounded 16 MiB parser may be valid general computation without fitting a
50-microsecond control deadline.

Under #2689, every transitive dependency used by a deadline region needs a
compatible proved timing/resource basis. Unknown or excessive cost refuses;
large finite work remains permitted elsewhere. Replanning must validate the
replacement implementation's exact basis. Average measured runtime, a finite
capacity, or simulation alone is not a hard deadline guarantee.

## Authority and confinement

An implementation must not possess materially more effect authority than its
admitted realization. Finite computation, large values, long lifetime, Host
membership, and capability availability grant no additional effects. The
planner consumes authority; it does not mint privilege by writing a Plan.

Descriptive grant IDs and Signs are not unforgeable authority possession or
mechanisms that prevent unauthorized effects. Native code sharing a broadly
privileged std process can bypass cooperative checks. The
[confinement contract](implementation-confinement.md) defines separate std,
WASM, ConduitOS, remote, and physical trust boundaries. #2685 requires actual
authorized effects and adversarial sibling-effect refusal at an enforcement
boundary before stronger isolation can be claimed.

Finite resource containment helps bound consumption, but does not itself
isolate effects. No generic secure flag or second authority system is justified.

## Acceptance boundary

This is a documentation contract. It does not claim that all requested State,
structured memory, parser/evaluator, continuous execution, migration, analysis,
WCET, or confinement proof already exists. The linked implementation issues own
those slices. #2687 owns this note's documentation acceptance and requires its
corrected premise, issue links, and stable promotion evidence.
