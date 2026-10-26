# Repeated train batches on a trusted host

The host service runs one finite tick per invocation. Its supported command is
wired by the integration owner; it uses the existing supervisor and controller,
not an alternative landing authority. Neither this source proposal nor fixture
proof establishes a deployed service or a live accepted App train.

An external owner-only regular configuration file contains exactly:

```json
{
  "schema": "conduit.train-service/v1",
  "repository": "dancxjo/conduit",
  "repositoryId": 0,
  "policy": "all-ready-same-repository-dev-prs",
  "sourceObjectStore": "/srv/conduit/source-objects",
  "serviceRoot": "/srv/conduit/train-service",
  "maxBatches": 128
}
```

The zero repository ID is deliberately invalid; supply the reviewed real ID.
The source object store and service root must be separate canonical absolute
paths. The service root is private mode 0700 and outside the trusted controller.
Pin **one** state root per repository in the host deployment. The local exclusive
lock fences ticks using that root; it does not coordinate different roots or
hosts. Do not enable two independent deployments for the same repository.

The trusted ready-selection adapter opens a short App session, observes the
entire finite ready inventory and fresh development/cheap-policy gates, and
revokes before returning selection. Missing adapters refuse; no stored human
credentials are used. The existing supervisor then opens separate sessions for
its operations. Waiting never holds an installation token.

Each selected digest gets its own private queue, source worktree, manifest,
archives, evidence and core journal. The service durably records allocation
intent before creating directories. A crash there requires inspection, never
recreation. Core mutation ambiguity and failed revocation stop the service;
there is no automatic skip, retry or recycling. A batch becomes completed only
after the core authenticates actual development adoption and publication.
Completed directories remain intact. At the configured limit (at most 128)
selection stops; no evidence is automatically deleted. Operators must arrange
reviewed external retention and a separately reviewed next deployment before
capacity can be reclaimed. Changing configuration/runtime with existing state
refuses rather than silently migrating it.

Deploy from an immutable clean trusted controller checkout and pinned Node
runtime. Keep RSA keys and the separately named observer environment credential
outside source and outputs, using the existing controller credential contract.
A systemd service may use `Type=oneshot`, `UMask=0077`, a pinned trusted working
directory, and the supported `cargo +stable xtask ci pipeline train-service`
entrance once integrated. Its timer should use `OnUnitInactiveSec=5min` and
`Persistent=true`; do not add a shell polling/restart loop or automatic retry on
failure. Supply only external configuration/key paths and the observer variable
**name**, never credentials in arguments. These are deployment examples, not
instructions to activate machine policy or cheap admission.

Maintenance/drain prevents new selection and every core mutation. Read-only
observation of the existing batch may continue, including authenticated
completion; a subsequent batch cannot be selected while drained. Stop and drain
all writers before history maintenance, retaining every service/core journal.
After a source-identity migration, existing exact-source authority cannot be
translated or reused; reconcile deployment state explicitly.
