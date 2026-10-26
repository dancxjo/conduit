# Body purpose and fulfillment

**Status:** current finite semantic and Body-side readiness contracts, with
separate lifecycle and physical acceptance boundaries. See
[STATUS](../../STATUS.md) for target-specific evidence and the
[canon](../conduit-canon.md#purpose-and-fulfillment-are-not-lifecycle-authority)
for the intended invariant.

## Why this distinction exists

A Body is Conduit's continuing logical computer, independent of any one Host
or Boot. It may carry a small stable workload or undergo explicitly admitted
workload revisions. Nothing about durable identity requires perpetual activity,
ever-growing capabilities, or a purpose that can never be satisfied.

A thermometer can remain a thermometer. It may complete finite work and enter
lull without being treated as a failed or insufficient Body. Conversely, a
Body can remain durably identifiable even while no Play is active.

## Three separate kinds of fact

| Fact | Owner | Not equivalent to |
|---|---|---|
| **Plot completion** | Authored Plot semantics and the Play that realizes it | Fulfillment of every Body obligation |
| **Purpose / fulfillment readiness** | Optional finite Body-purpose semantic state and deterministic evaluation | Lifecycle authority, Play completion, or deletion |
| **Birth, wake, lull, fulfillment** | Authorized Body lifecycle with its own evidence | Automatic consequences of a readiness value |

The canonical trailing `}.` marks semantic completion when the admitted Plot's
structural work drains. A live Plot without that full stop can be quiescent and
later resume; quiescence is not semantic completion. Neither condition by
itself changes the Body's purpose policy.

## The current finite purpose contract

The Body-side implementation is
[`architecture/body/src/character_purpose.rs`](../../architecture/body/src/character_purpose.rs).
The portable Types are in
[`semantics/purpose/types.conduit`](../../semantics/purpose/types.conduit).
The [`fulfillment-readiness` Plot](../../plots/fulfillment-readiness/main.conduit)
calls the purpose-readiness Kind and explicitly performs no lifecycle action.

A purpose has a nonempty identity, revision, summary, completion policy and a
finite collection of uniquely identified obligations. Present Body-side
limits admit at most 32 obligations and at most eight Sign identities for one
obligation's evidence. Each obligation is one of:

- `Pending`, with unfinished work;
- `Satisfied`, backed by bounded Sign identities;
- `RepairRequired`, with an exact failure Sign;
- `CompletionEvidenceMissing`;
- `Uncertain`, with evidence that does not establish satisfaction;
- `Disputed`, with distinct evidence of disagreement.

The `ExplicitFulfillmentReadiness` policy yields **Ready** only when all
obligations are satisfied. Any remaining obligation produces **NotReady**
with explicit reason identities. The `NoFulfillmentCondition` policy yields
**Unavailable**, not a synthetic readiness or failure. The evaluation does
not fill in missing evidence or infer that time alone satisfies an obligation.

## The explicit terminal Fulfilled state

The ordinary Body lifecycle already implements `BodyState::Fulfilled` and
`Body::fulfill` in [the lifecycle owner](../../architecture/body/src/lifecycle.rs).
Fulfillment is not an emergent decision made by a Plot, model, or renderer.
The Body must be **Lulled**. The transition records an exact Sign, authority
grant, attribution, final workload revision, and settlement evidence.
The [Body lifecycle session](../../architecture/body/src/lifecycle_session/mod.rs)
offers an operator-driven route after active execution has been settled.
A fulfilled Body cannot wake or accept mutable workload changes.

An optional `BodyPurpose` retains identity and explicit revisions across
replanning, host change, and lull. [Its contract](../../architecture/body/src/character_purpose_continuity.rs)
refuses *active* readiness once fulfilled, while keeping the last semantic
purpose inspectable. A ready purpose does **not** automatically call
`fulfill`: readiness and authority remain independent. The generic lifecycle
also does not infer the readiness of a particular application purpose;
correspondence between purpose obligations and administrative settlement is a
separately governed caller/policy responsibility, never an ambient guess.

Fulfillment concludes the Body's active useful continuity; it does not erase
biography or evidence. Physical cleanup, external persistence settlement, and
accepted end-to-end behavior require separate proof for each Host and product.
Do not infer automatic finalization from a checked readiness result.

## The boundary that must survive optimization

The complete semantic meaning can be checked and represented compactly at
runtime, but lowering cannot change which obligations count, the identity of
their evidence, or the distinction among Ready, NotReady, and Unavailable.
No host may turn readiness into a silent decision to Fulfill or erase a Body.

**What this document does not prove:** that all Hosts implement the complete
operator-driven Fulfill route or its external cleanup, automatic durable
restart, arbitrary state migration, subjective experience, or any obligation
for a Body to develop new desires or purposes. It records existing contracts
and their boundaries, not a theory of consciousness.

Related: [Bodies, hosts, plans, and plays](../../wiki/Bodies-hosts-plans-and-plays.md),
[the glossary](../../wiki/Glossary.md), and
[Body lifecycle boundaries](body-lifecycle-waists.md).
