# Trusted host supervisor: one explicit train batch

`train-supervise` advances one private, explicitly approved batch without five
operator-driven stage commands. It is a source prerequisite, not an enabled
service, live App setup, or accepted machine-train proof. Cheap admission stays
inactive. Register the dedicated App, provision the separate observer, and prove
the server writer/proof gates before supplying production credentials.

Run only from an independently reviewed, pinned, clean trusted controller
checkout. Proposed source supplies Git objects and data; its scripts never own
this supervisor. Each tick binds the complete controller Git HEAD/tree, actual
pipeline code inventory, Node executable hash/path/version, public machine
configuration and canonical queue digest. Changed identity refuses recovery;
these checks do not establish that the pinned code was reviewed or trusted.

```sh
cd /absolute/trusted-controller
cargo +stable xtask ci pipeline train-supervise /private/train/queue.json /private/train/state /private/train/machine.json /private/train/app.pem CONDUIT_TRAIN_POLICY_OBSERVER
```

The queue is owner-only, regular single-link JSON, bounded to 32 KiB:

```json
{
  "schema": "conduit.train-supervisor-queue/v1",
  "id": "reviewed-docs-batch-1",
  "repository": "dancxjo/conduit",
  "prs": [123],
  "targetSource": "/private/train/source-objects",
  "worktree": "/private/train/batch-worktree",
  "manifest": "/private/train/batch-manifest.json",
  "archives": "/private/train/batch-archives",
  "evidence": "/private/train/batch-evidence"
}
```

The PR number is illustrative. The explicit list expresses operator intent;
drafts, changed heads, stale Candidate custody, wrong bases and unmergeable
inputs still refuse through the authoritative admission adapters. Input order
is journal-bound; changing or reordering the queue does not grant a new batch.
Source and controller must be separate canonical directories. Outputs must be
outside both, mutually disjoint, with existing canonical owner-only parents.
Create the external state directory with mode 0700 before the first tick.
Worktree, manifest, archive and evidence destinations must initially be fresh.
No credential belongs in queue, configuration, journal, arguments or source.

Pin **one state directory per repository in the host service configuration**.
All that repository's ticks and batches must use it. Its exclusive crash-retained
lock serializes callers of that configured directory; it does not discover other
state roots or fence multiple hosts. Do not run another controller for the same
repository using a different directory or machine. This is a host deployment
obligation in addition to server actor/proof policy.

Each finite tick performs at most one assemble, dispatch, acquire or land, or one
read-only exact-run observation. It opens a fresh short-lived App session for the
operation and attempts revocation before returning. Tokens are never held across
ticks or waiting intervals. The original controller remains responsible for all
fresh policy, source, Candidate, product custody and landing checks.

A durable fsynced journal records intent **before** calling an operation. A crash,
unknown result, failed revocation or callback error retains that intent; later
ticks demand inspection and never replay the mutation. The dispatcher must
acknowledge exclusive immutable-ref creation. Its `runId:null` result advances to
observation, never redispatch. Observation authenticates a complete bounded run
inventory, exact repository IDs/workflow/manual event/branch/source/tree and
run attempt. Multiple matching runs, changed attempts or proof failure block.
A first listed reused run ID binds its attempt on the first actual observation.
Artifact acquisition and landing independently authenticate that custody again.

Observations are separated by at least five minutes, with no polling loop inside
a tick. Only explicitly recognized subprocess transport failures may defer;
after six consecutive such failures inspection is required. Unclassified API,
identity, policy, rate-limit or revocation refusals stop for inspection rather
than retrying with uncertain authority or ignoring a Retry-After instruction.
No token refresh, mutation retry, forced update or stored-human fallback exists.

`--drain` pauses **all** new mutations, including an already assembled train's
dispatch or landing. It retains current evidence and permits finite read-only
run observation. To finish a current train before maintenance, run normal ticks
until its landing/downstream boundary, then stop the timer and settle actual
publication before freezing writers. Do not describe `--drain` as automatic
completion of an accepted train.

After an acknowledged landing, the journal remains `awaiting-downstream` with
an explicit operator-recoverable gate. It does not claim complete train success.
The existing real dev-push Integration/adoption and publication workflows must
still be authenticated. Automated downstream observation and a reviewed
multi-batch dequeue/recovery lifecycle remain follow-on work. Never delete or
rewrite a journal to silently skip those gates or recycle a batch. For recovery,
stop the timer, retain journal/lock/evidence, inspect exact remote refs/run
attempts and outcomes, and record a separate reviewed recovery decision. PID
absence alone is not permission to unlink a crash lock or replay an operation.

The service/timer examples are inert templates. Replace every path with pinned
external controller/state/key/config paths. The external owner-only environment
file supplies only the named observer credential; the App key stays a separate
owner-only RSA file. Do not install/enable these examples from a PR checkout.
No live service, policy change, App registration, hosted acceptance or useful
cache speedup is established by local supervisor conformance tests.
