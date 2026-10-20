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

## Combined development validation

The review was refreshed onto development source
`c10e6a265d5e1022d3d470639e2b99d3a3ac0f18`, which includes the native U64
region and browser clock-proof repair. The validated combined implementation is
`ddaf3e23fe2ab71c3f8d2d2bd47c40d4f2aac246`. Forty-one of the 43 original
review files are byte-identical. The other two are exactly the new base's
scheduler plus the prepared-session module declaration, and the new base's
command help plus the prepared-owner flag. The byte checks and retained results
are in [the validation manifest](evidence/prepared-fargan-session-20261019/current-validation-manifest.json).

On that combined source, the full alloc-enabled kernel package passed 268 tests
with six optional tests ignored. The full cached AI package passed 342 tests
with four private-fixture tests ignored. The explicit private 24-frame reusable
trajectory separately passed against fresh owners and the pinned scalar oracle.
Kernel all-target Clippy and the actual AI/xtask all-target Clippy passed with
warnings denied. These are package scopes, not a full-workspace acceptance claim.

A new composition regression runs eight accepted invocations through the native
U64 chain and canonical reference. It compares outputs and every retained Sign,
checks both stores drain, retains the scheduler addresses, and refuses old
ValueRef generations after slot reuse. This exercises the merged mechanisms
together; it is not native FARGAN numerical-region evidence.

The preceding review source `6f108852608c3fb4dfa960da629709efb7a0bd5a` also
completed its entire uncached kernel/AI compatibility suite: 595 passed, zero
failed, ten optional tests ignored. That result belongs to the preceding source;
it is retained separately from the current cached-profile results. Logs are
lossless `.xz` containers with decoded byte digests in the manifest. Concurrent
validation/build activity excludes these test durations from performance claims.
Ordinary Body adoption and stable release acceptance remain open.
