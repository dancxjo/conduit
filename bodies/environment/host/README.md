# Environmental observer joining

The provisioned owner issues one private invitation over the thermometer's USB UART:

```sh
conduit-environment-host --issue-private-invitation OWNER_STATE /dev/ttyUSB0 PRIVATE_BOOTSTRAP.json
```

Keep the bootstrap and its parent directory private (file mode 600, directory mode 700). The invitation expires after five minutes. The public `/body/join` page supplies instructions; it never distributes the bearer secret.

A Linux Host, including a Raspberry Pi running Linux, can consume that invitation:

```sh
conduit-environment-host --join-observer PRIVATE_BOOTSTRAP.json PRIVATE_STATE DEVICE_HOST:80
```

This creates a signed `PRIVATE_STATE/recorder-binding.json` and retains private continuity material. Repeating this command resumes the same admission and its original Boot; it does not create a new Boot. An actual Raspberry Pi run remains a separate physical acceptance requirement.

To return the same admitted Linux Host and part with a fresh explicit Host-process Boot, use a separate output directory:

```sh
conduit-environment-host --return-observer PRIVATE_STATE/recorder-binding.json PRIVATE_STATE/continuity-private.json NEW_PRIVATE_STATE DEVICE_HOST:80
```

The command authenticates the Root's return challenge, signs the canonical part-return proof, verifies the returned membership receipt, and writes `NEW_PRIVATE_STATE/recorder-binding.json`. It preserves the original binding and does not restart an existing recorder. Select the new binding explicitly when starting its replacement. Run this on the original Linux machine: its `/etc/machine-id` must match the admitted Host identity.

Retry with the same output directory after interrupted transport. The retained advertisement preserves the fresh Boot and the signed completion request; read-only recovery can retrieve a committed return whose reply was lost. A completed output directory is idempotent. Up to eight immutable challenge rounds may reconcile signed expiry or a signed authority restart; a local timeout cannot renew a challenge. Exhaustion refuses without editing the retained proofs. A further new observer Boot requires a new output directory.

For a browser, generate a private local helper and run the loopback proof proxy on that browser's machine:

```sh
conduit-environment-host --browser-invitation PRIVATE_BOOTSTRAP.json PRIVATE_HELPER.html
conduit-environment-host --browser-join-proxy DEVICE_HOST:80 8787
```

Open the helper locally. Its **Join this Body** and **Reconnect this browser** actions verify Root signatures through WebCrypto. The helper contains the continuity secret; do not publish it. The loopback proxy holds no invitation secret.

These commands admit observation-only Hosts. They do not create an execution offer, Plan, Play, or browser Mask. Joining, recording signed evidence, and installing the Body's durable recorder are separate acceptance boundaries.

## Following producer restarts in retained inspection

The continuing joined collector accepts `--retained-projection current PRIVATE_OUTPUT`
in place of an explicit `boot/...` pin. After successful collection, it uses its
existing private continuity input to sign a fresh nonce-bound membership recovery
query. The existing producer recovery receipt must match the pinned Root, producer
Host and exact observer credential. A three-second socket budget and sixteen-exchange
limit bound this read-only probe; transient socket failures retry the same signed
query under that shared budget, while invalid protocol or identity evidence refuses. synchronous DNS remains outside that budget.
Only a complete signed history prefix for the verified Boot can replace the view.
An outage, replayed nonce, mismatched identity, absent prefix or ambiguous Play
retains the previous view. Original per-Boot files and membership bindings remain
unchanged. This selection does not grant recording custody, refresh Face leases,
establish shared clock synchronization or commit observations to a selected recorder.

`--inspect-current-producer PRIVATE_BINDING PRIVATE_CONTINUITY HOST:PORT` runs
the same bounded read-only probe without changing the collector or its projection.
Success reports the authenticated producer Boot; failure retains only the HTTP
status and bounded public refusal code in its diagnostic.

## Finite automatic observer return

An operator can explicitly reserve return-state capacity for the continuing collector:

```sh
conduit-environment-host --record-joined-continuing ORIGINAL_BINDING PRIVATE_CONTINUITY HISTORY_DIRECTORY HOST:PORT --retained-projection current PRIVATE_OUTPUT --observer-return PRIVATE_RETURN_DIRECTORY 8
```

The capacity must be 1 through 8. Its private immutable manifest pins the exact
original binding, continuity input and capacity; restarting the collector cannot
replenish it. Each successful canonical return retains a separate immutable
binding. Interrupted attempts resume the same advertisement, Boot and proofs.
Completed generations reopen locally without a transport entrance. Capacity
exhaustion leaves collection and the last verified view available.

Only the exact continuity-refusal code schedules a return attempt. That unsigned
HTTP response is a hint, not evidence of a restart or authority: the existing
Root-signed challenge and receipt checks still gate every credential change.
Collection runs before recovery; each return step has one three-second socket
budget. DNS, local signature verification and durable file operations remain
synchronous, so this is not a hard real-time deadline.

New observation requests use the verified returned binding. Already signed
requests and a pinned network-custody transaction keep their original bytes and
owner; return cannot authorize their replacement or renew an ACK window. An
idle observation-custody lane can resume after verified return; an unresolved
possibly accepted request still requires explicit reconciliation. This is a
finite host-tool recovery policy, not proof of an installed selected recorder,
execution-Host admission, or a checked HostDescription storage grant.

## Retaining a recovered network completion

If the original acknowledgment's response was obtained through another admitted
route, retain that response without starting another network transaction:

```sh
conduit-environment-host --retain-network-completion PRIVATE_BINDING ORIGINAL_PRODUCER_BOOT PRIVATE_SNAPSHOT_DIRECTORY PRIVATE_RESPONSE_JSON
```

The original snapshot, acknowledgment and reserved session must already exist.
The command requires their exclusive file lock, verifies the independently pinned
Body/Root, original acknowledgment and Root-signed Clean cursor, then fsyncs and
reopens the completion. Pending, corrupt or unrelated receipts refuse before
publication. Repetition preserves the original completion. This filesystem-only
entry has no signer or transport and creates no retry allowance or erase request.
A continuing collector subsequently reopens that completion using its original
identity, even after an observer return.
