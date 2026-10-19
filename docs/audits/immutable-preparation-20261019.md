# Immutable expression and numeric preparation

Development measurements on the full FARGAN graph reduced fresh owner
preparation from about 2.8 seconds to 263 milliseconds per frame. The change
shares exact expression-fragment validation across owners and clones only the
selected Type or Kind from the existing immutable numeric catalogue. Each
frame still creates fresh owners; prepared streaming sessions and compact
numeric execution regions remain separate work.

## Preparation behavior

`ExpressionPreparation` borrows one immutable Plan fragment, its exact lowering
and its active Play identity. Construction performs the same complete fragment,
active identity and lowering checks previously repeated by every expression
owner. Preparing an owner still validates its exact placement, selected offer,
program and input bounds, and allocates an independent evaluator. Request and
cancellation state belong to each owner. The original single-owner preparation
entrance delegates to this context and retains its checks.

The production pure-protocol preparation loops and FARGAN proof driver create
one context for their expression owners. Selectors retain their existing
validation. There is no cross-Plan cache: immutable borrowing prevents changes
to the checked dependencies during preparation, and a new context validates a
different fragment, lowering or active identity.

With `hosted-catalog-cache`, numeric lookup now clones only the selected checked
Type or Kind. The existing compiled-Source catalogue and its initialization
remain unchanged. Full-catalogue APIs still return independent owned catalogues;
the uncached profile retains its original parse-and-select behavior. Mutating a
returned contract cannot modify the cached base. Numeric operations, precision,
Source and Play execution are unchanged.

## Full graph measurements

The reference source is `63ce47e6adbc13ee9306929a364176382650cd15` and the measured
implementation is `58449e1873fe24bbed2c419c71ab662366911f83`. Both workers were
compiled at the same filesystem path with the pinned Rust toolchain. The
supported capture entrance is `cargo xtask prove runtime-performance` with
`--fargan-fixture`, `--output` and `--repetitions 2`. The pinned private model and
oracle binaries are excluded from the retained evidence.

Three process pairs each contain two 24-frame trajectories per side. The first
pair used the supported capture command with inherited CPU affinity. The other
pairs replayed retained worker executables on CPU 4, in after/before and
before/after order. State accumulates within each trajectory and resets between
repetitions. These are descriptive observations on one shared Intel i7-12700
host, without a universal timing threshold or confidence interval.

| Process pair | Before preparation ms/frame | After preparation ms/frame | After/before |
|---|---:|---:|---:|
| 1 | 2884.999 | 262.762 | 0.0911 |
| 2 | 2792.904 | 258.072 | 0.0924 |
| 3 | 2830.339 | 268.056 | 0.0947 |

The mean paired preparation ratio is 0.09273, approximately a 90.7% reduction.
Play wall time ranges from 23.09–23.55 ms before and 21.41–22.24 ms after;
variation in unchanged Play code is not credited as an execution optimization.
Preparation excludes caller input encoding, and these measurements do not
establish interactive or real-time audio latency.

All 144 paired frames match exact Plan, output and Sign hashes, numerical error
trajectories and work counts. Each frame retains 768 nodes, 1,211 Cords, 3,165
Steps and 6,976 Signs, drains structurally, finishes without pending Host Calls
and performs zero Play allocations or reallocations. The Source and numerical
contracts are unchanged. Intermediate canonical values remain materialized.

The [comparison](evidence/immutable-preparation-20261019/combined-evidence.json)
binds the source and executable identities and the digest of the
[raw captures](evidence/immutable-preparation-20261019/combined-raw-captures.tar.gz).
The adjacent internal verification script checks the archive and per-artifact
digests, source custody, frame identities, exact evidence and zero Play
allocation. It also rejects twelve deliberate comparison mutations across the
three pairs. Retained manifests establish capture integrity; independent
reproduction still requires the private fixture.

## Validation and remaining work

Expression preparation tests cover independent request/cancel state, changed
Plan/lowering/active identities and foreign placements. Nine expression tests
and 110 protocol library tests pass. The numeric catalogue tests pass with
caching enabled and disabled. The complete AI release package with
`kernel-operation-owners,hosted-catalog-cache` passes 340 tests; three optional
private-fixture tests are ignored in that package run. Both supported FARGAN
captures separately pass their conformance checks and actual 48-frame workers.

Workspace lint and integration acceptance remain required before admission.
This development result is preparation reuse within one invocation. Reusing
owners across accepted frames, dependency-aware checking across edits and
avoiding internal canonical materialization are still required for the broader
performance audit. Stable acceptance follows the automated release train.
