# Finite retained parser transaction integration

The generated Source receipts own semantic correspondence. The Rust owner owns
complete immutable evidence, finite counts and canonical body charges, and atomic
publication. Canonical evidence limits do not claim an allocator or executor
output-allocation bound.

Current unselected drafts:
- parser_protected_origin.rs: complete original admission, exact fixed Source
  projection program/input/output, compact edge and Source correlation.
- parser_protected_custody.rs: complete origins, initial/insert/rebase receipts,
  current set, explicit counts/bytes/peak and monotonic cancellation.
- parser_protected_custody.rs test: actual already-active original preserved;
  new separately admitted empty-reference initialization, exact rebase, stale
  predecessor, pressure, cancellation.

These drafts currently admit already-produced receipts before local publication.
They do not yet reserve before Flow consumption, and are not Session authority.

Required Session transaction order:
1. Admit Session-wide limits before creating executable ingress: origin slots4,
   fact/rebase/snapshot/commit counts, retained canonical bytes, candidate peak,
   Source Flow invocation/input/post-consumption-output limits, and pinned full
   model/profile/policy ownership. No eviction.
2. Preflight the exact old custody and selected transaction kind. Reserve count
   slots and a caller-admitted finite canonical candidate envelope before calling
   Source Flow. Cancellation or pressure refuses before invocation.
3. Execute the exact fixed Source contract once. Consumption failure poisons the
   ingress; the old published custody remains but execution cannot be replayed.
4. Readmit complete Native output and relation-specific receipt. For protection,
   check every retained whole origin against the next set, retaining the complete
   Source rebase context/output. For facts/snapshots/commit, use retained frontier
   receipts rather than a caller Boolean or digest-only anchor.
5. Require actual candidate charges within the reserved envelope. Post-consumption
   output overflow refuses publication and poisons ingress; it does not claim the
   executor avoided allocation. Reserve vector capacity while still unpublished.
6. Recheck cancellation; publish all associated set/origins/rebases/facts/snapshot
   and committed beam through infallible moves in one owner transaction. No fact
   push before commit, no protection mutation before joint rebase succeeds.
7. Observer I/O follows publication and records delivery failure separately. It
   cannot make an already executed transaction replayable or retroactively undo
   publication. Parser commitment remains separate from played acknowledgement.

Private Session integration sites (parent owns mutation):
- stabilize: current retained_facts.push precedes Flow6. Stage fact, protection
  acquisition and commit output together before publishing any of them.
- initialize: stage protection rebase and JointRebase output together.
- snapshot: stage full NativeAvailableState plus observed committed frontier;
  replace growing committed_snapshots with bounded retained snapshot receipts.
- Protection: replace growing origins/rebases with the concrete finite owner;
  retain original full admissions and every full lineage context/output.

Meaningful final gates:
- exact original Native canonical decode/encode and Source projection parity;
- positive first reference insertion and real first acquisition kept separate;
- foreign original/current basis, altered subtype/choice/occurrence and stale
  full prior set refused by Source before publication;
- each finite count, retained byte and peak cap refuses without changed current
  set/beam/receipt counts or invocation for pre-consumption pressure;
- cancellation before invocation and after prepared candidate leaves prior
  published custody; independent monotonic latch cannot re-enable ingress;
- ambiguous consumed executor failure cannot replay;
- atomic joint protection/fact/commit publication, including late Native refusal;
- actual private Session uses this owner, not fixture-only storage tests.
