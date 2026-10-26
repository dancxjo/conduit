# Trusted train controller command

This command composes the reviewed App session, exact train selection,
dispatch, artifact acquisition and landing adapters. It is source implementation
with local conformance proof, not a deployed service or live GitHub acceptance.
The [dedicated App and two independent server gates](train-machine-writer.md)
must be installed and verified before use. Cheap PR activation remains off.

Run the supported entrance from a pinned, reviewed controller checkout:

```sh
cd /absolute/trusted-controller
cargo xtask ci pipeline train-control /absolute/request.json /absolute/reviewed-machine-policy.json /absolute/private/app.pem CONDUIT_TRAIN_POLICY_OBSERVER
```

The last argument names an existing observer credential environment variable;
it is never the credential value. The private key must be an external canonical
owner-only, single-link RSA file. Neither key nor observer token belongs in a PR,
request, manifest or Git. Review the controller, its Cargo configuration, runtime
and dependencies before supplying credentials. Build/execute only this trusted
checkout. Proposed source is passed as Git/data and is never imported or built
with these credentials. The controller refuses overlapping source directories.

Each invocation opens one short-lived session and performs one requested
operation. JWT signing, exact App/installation/policy validation, scoped token
issuance and one final revocation attempt use the [session contract](train-app-session.md).
Credentials remain in memory and explicit trusted child environments; the global
environment is not changed. Git reads do not receive the App token, and source
operations do not receive the owner observer. Artifact downloads use an explicit
scoped token and fixed working directory, not ambient `gh` authentication.

The credentialed Git push runs from a new private controller-owned bare
repository, borrowing only the proved source's object directory. Its empty
hooks/template and independent configuration prevent target-local hooks and URL
rewrites from running with the token. Global/system Git configuration and
ambient Git directory/config/transport overrides are disabled. The temporary
context is removed after success or failure; the original exclusive-ref lease,
new-branch acknowledgement and remote readback still govern dispatch ownership.
Target source Git metadata is never the credentialed push working directory.
The older `train-dispatch` entrance requires an explicit `GH_TOKEN` and uses
the same scoped child bridge; it cannot fall back to stored human GitHub login.
It does not itself mint or validate a dedicated App session. Use `train-control`
for production machine identity and session validation.

An assemble request explicitly selects ready PR numbers:

```json
{
  "schema": "conduit.train-controller-request/v1",
  "operation": "assemble",
  "targetSource": "/absolute/source-object-checkout",
  "prs": [6476, 6496],
  "worktree": "/absolute/new-train-worktree",
  "manifest": "/absolute/new-train.json"
}
```

The numbers above illustrate explicit selection, not current admission or a
promise that those PRs are ready. All paths are absolute and canonical in syntax.
Existing destination, stale readiness, changed head/run/attempt/jobs, conflicting
merge or moved dev refuses through the existing adapters. Request files are
bounded regular single-link JSON; unknown fields and operations refuse.

Subsequent requests use the assembled train worktree as `targetSource`:

| Operation | Other required fields |
|---|---|
| `capture` | `prs`, exact combined `source` commit, fresh `manifest` path |
| `dispatch` | `manifest` |
| `acquire` | `manifest`, positive integer `runId`, fresh `archives` directory |
| `land` | `manifest`, `runId`, authenticated `archives`, fresh `evidence` directory |

Every request also includes the schema, operation and target source fields.
Dispatch returns its exact run identity or an explicit newly dispatched state;
observe that run outside the credential session. There is no polling loop,
automatic retry, token refresh, forced update or human mutation fallback.
Acquisition and landing start new authenticated sessions after proof completes.
The finite JWT/token lifetime still bounds an individual invocation; an expired
credential refuses rather than changing authority during the operation.

Cheap selection refreshes independently authenticated writer/proof/activation
observations at each controller validation. The opaque grant never enters the
manifest. Hosted `train-admit` has a separate read-only role: it verifies current
ready PR/head/run/job custody and reconstructs the exact selected tree without
privileged secrets, then schedules the complete exhaustive proof. It cannot
dispatch, restore accepted products, publish or land. Controller authorization
and server landing gates remain mandatory after that read-only check.

Failures retain the existing manifest, acquisition and landing intent/evidence
boundaries. A callback or revocation failure is not permission to repeat a
mutation. Inspect actual remote refs/run and retained intent after an ambiguous
outcome; do not automatically reissue dispatch or landing. The session reports
bounded error codes and revocation state without raw response or credential
output. A successfully changed ref can still be followed by a revocation failure;
that failure must not be described as a rolled-back operation.

Local tests compose actual RSA signing/scoped issuance fixtures with a real
combined Git capture, and exercise policy/identity, stale-source, secret
redaction, byte/timeout and child-credential refusals. These are local protocol
proofs. Registration, hosted API compatibility, correct/wrong-actor gates,
positive exact landing, actual dev push adoption and publication reuse remain
distinct live acceptance work.
