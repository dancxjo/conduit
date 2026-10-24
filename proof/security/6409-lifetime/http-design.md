# HTTP request lifetime correction (#6409 slice)

The old public Copy ticket could be reused against a fresh client after cancel:
with identical transaction and request, client B accepted client A's stale ticket.
The retained old-source regression fails for that exact reason. This does not
claim detached successful-completion exploitation or hostile-code confinement.

`HttpRequestGuard` now exclusively borrows its exact client. Consuming exchange
and cancellation retire once; Drop clears fixed request/response storage. A
forgotten guard keeps the pending latch set and future requests refuse Pressure.
There is no detached identity counter to reset or wrap. Removing the split ticket
API is an intentional API change; synchronous `exchange` retains its signature.

A provider is attempted once. Failure may leave an unknown external network
outcome; retirement does not roll back that effect or silently retry it. A bounded
counting pass validates response encoding before writing the existing fixed
output buffer, preserving the previous output on refusal without new backing.
Codec and affine custody responsibilities are separate modules.

Local validation passes one allocation/lifecycle test, eight owning HTTP tests
including the ordinary Source/Plan/Play kernel fixture, and two ownership
compile-fail controls for E0499 and E0382. Strict owning Clippy, formatting and
diff checks pass. Source/log/executable identities and the historical negative
are retained in `http/`; actual executables are sealed outside the build cache.

This repairs the HTTP helper slice. Full #6409 still requires the other audited
mechanisms, persistent Host A→retire→B callback/lease/input/staged-output proof,
fault/race/disposal matrix, cross-target proof and applicable CI. This slice does
not close that issue or establish physical or runtime confinement.
