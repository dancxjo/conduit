# Bounded learned-model lifecycle

Status: current pre-release contract.

This specification owns optional dataset, training, checkpoint, evaluation,
and promotion boundaries above `conduit-learned` inference. Inference-only
hosts remain conforming and do not inherit lifecycle providers, storage, jobs,
or promotion authority.

## Identity and authority

`LML-001` Dataset snapshot, dataset revision, feature schema, label schema,
provenance, training job, checkpoint, evaluation suite, metric version,
evaluation report, approval, promoted target, and promotion receipt are
distinct identities. A favorable report is never an approval.

`LML-002` Training and evaluation are bounded computations. Promotion is a
separate effect requiring an exact `conduit.action/promote` grant, resource
lease, commit profile, and acknowledged receipt. Missing, revoked, or stale
authority fails before the promotion provider runs.

`LML-003` An inference provider, a training/evaluation provider, and a
promotion provider are independently installable. Contract knowledge does not
install any of them.

## Finite lifecycle

`LML-004` Dataset records and bytes, job steps/work/deadline, retained
checkpoints, checkpoint bytes, storage, evaluation cases/work, promotion
attempts, receipt bytes, and evidence events are finite and plan-visible.

`LML-005` Dataset revision or schema mismatch, sensitivity denial, resource
exhaustion, stale provider, cancellation, provider loss, incompatible
checkpoint, metric-version mismatch, evaluation leakage, denied promotion,
unknown commit, and duplicate commit are exact outcomes. They never fabricate
a checkpoint, report, approval, or receipt.

`LML-006` Cancellation and provider loss are terminal for the current job.
Unknown promotion commit requires reconciliation under the pinned commit
policy; retry does not imply global exactly-once behavior.

## First proof

The deterministic proof uses four public fixture records, one four-step exact
training job, one retained checkpoint, four evaluation cases under
`accuracy@1`, and one independently granted promotion into
`learned/reference`. The complete composition executes through the production
executor and emits only the acknowledged promotion receipt.

Owned nodes are `learned/dataset/literal`, `learned/train`,
`learned/evaluate`, `learned/promote`, and bounded dataset/evaluation/promotion
inspectors. `examples/learned-evaluation.panel` proves that training and
evaluation run without a promotion provider or grant.

## Conformance

`conformance/c4/learned-lifecycle.json` owns the complete first matrix.
`examples/learned-lifecycle-standalone.panel` is the dataset proof;
`examples/learned-evaluation.panel` is the unprivileged training/evaluation
proof; and `examples/learned-lifecycle.panel` is the authorized composition.

## Non-goals

No universal quality score, evaluation-as-approval, ambient dataset or model
download, hidden experiment tracker, framework object, unbounded checkpoint
store, automatic promotion, or requirement that inference providers train.
