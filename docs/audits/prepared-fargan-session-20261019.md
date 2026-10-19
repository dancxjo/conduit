# Reusing prepared FARGAN owners

An opt-in prepared session retains one boxed execution kernel across finite,
explicitly admitted invocations. Immutable code, graph bindings, numeric owners,
weights, scratch reservations and consumed Host Request generations survive.
This extends the existing kernel lifecycle; it does not introduce a scheduler.

## Admission and ownership

Each Back opts into a read-only next-input preflight and an infallible bounded
begin operation. Every preflight succeeds before any owner changes. Rearm requires
normal drain, no pending Host work or queued ownership, an empty equivalent Sign
reservation, and no cancellation, abnormal event, retention gap or active debug
control. Old evidence transfers to the caller rather than disappearing. Resident
pins retain their checked ownership generations; a cancelled or failed session
cannot become healthy by being rearmed.

The first profile accepts local Cords only. Remote transport epochs and Fore
acknowledgements are not inferred from drain. Ordinary Body execution therefore
continues using its existing lifecycle. The ten fixed numeric owner families
expose reusable preparation while retaining their existing preparation API.

The FARGAN trajectory proof keeps the authored 768-node, 1,211-Cord graph. A
bounded controller binds exact Plan/Host/Boot active identities and admits a new
frame only after every required prior output is accepted. Tests reject stale,
partial and duplicate acceptance, invalid next input, exhausted frame reservations
and cancellation. Recurrent state and conditioning history advance after complete
output acceptance.

## Measured lifetime cost

The measured source is
[`e4fbac12b38f`](https://github.com/dancxjo/conduit/tree/e4fbac12b38ff0f787fe626f4c587cbd175ab5b8).
Three fresh processes pinned to CPU 4 each compared two 24-frame trajectories.
Including initial retained preparation, input binding, rearm, output decoding,
acceptance/state advance, old evidence release and final session destruction,
the median admitted-Plan trajectory cost was 0.110 of fresh execution, with
process ratios ranging from 0.1094 to 0.1108.

Attributing a standalone cold Source/model/Plan startup to each path gives a
ratio of 0.213. Counting shared startup once across both trajectories in each
process gives 0.199. These are sums of recorded phases; harness conformance,
artifact writing and common result destruction are excluded. They are descriptive
finite-trajectory measurements, not whole-process stopwatch comparisons.

The public capture and three independent processes cover 192 paired frames and
2,678,784 ordered Signs. Outputs match fresh execution exactly; every Sign matches
with its expected per-frame Request generation offset. Both paths allocate
nothing during Play. Final retained destruction takes about 2.1–2.3 ms per
24-frame session, with no allocation and 60,076 deallocations in the public run.

All canonical intermediates are still materialized. Play remains around 21–22 ms
for 160 samples, so these results do not establish realtime synthesis or numerical
region speedup. This is a full-graph proof controller using production owners,
not the ordinary streaming Std FARGAN Back required by issue #5715.

## Reproduction and review source

The supported entrance is:

```sh
cargo xtask prove runtime-performance --fargan-fixture PINNED_FIXTURE_DIRECTORY \
  --fargan-owners prepared --repetitions 2 --output NEW_DIRECTORY
```

It requires the explicit pinned development model/oracle accepted by the command.
The [evidence folder](evidence/prepared-fargan-session-20261019) contains full
captures, member hashes, phase accounting, the independent verifier and 17
rejected semantic mutations. All four archives passed verification after
relocation. Extract the public archive beside each replay archive, then run:

```sh
python3 verify-prepared-fargan.py EXTRACTED_CAPTURE \
  --source e4fbac12b38ff0f787fe626f4c587cbd175ab5b8
```

The refreshed review source applies the same 30 changed files
byte-for-byte onto dev base `7f9748092732ecf2fc04e8992a2a914247b73173`;
its source bridge records both identities. Refreshed-source validation and
combined integration remain separate gates from the measured reference.

Ordinary Body adoption, native FARGAN regions and stable release acceptance remain
open. This session profile alone does not complete the performance audit.
