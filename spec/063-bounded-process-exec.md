# Bounded process exec boundary

## Contract

`conduit.host/process/exec` is the one current optional process primitive. It
has exactly three ordinary byte-stream ports:

- `stdin`: finite `std/bytes` input;
- `stdout`: finite `std/bytes` output;
- `stderr`: finite `std/bytes` output.

Spawn, exit, signal, deadline, cancellation, output overflow, provider loss,
and cleanup are typed lifecycle and execution-evidence outcomes. They are not
a fourth byte stream. The displaced `process/run` and `process/stream` drafts
are not aliases and are not accepted.

## Command and authority

The authored command names an executable resource, bounded literal argv,
bounded explicit environment additions, an optional working resource,
stdin-close behavior, output/chunk ceilings, a deadline, and a finite
termination escalation. Command text is never shell syntax. Metacharacters are
literal argument bytes and the base contract performs no interpolation.

Before spawn, exact compilation pins the executable resource and artifact,
host, provider implementation and artifact, grant, lease, working resource,
environment policy, command fields, and all finite limits. Checking,
describing, inspecting, and resolving never spawn a process. The active
executor performs no `PATH` search and inherits no ambient environment,
descriptor, filesystem, network, secret, or child-process authority.

## Streams and lifecycle

The three ports use ordinary cord bounds and pressure. Stdout and stderr are
independent and must be drained concurrently within their separate planned
queues. Neither stream may be silently discarded, merged, decoded as text, or
promoted to domain semantics. Stdin close, either output close, child exit,
signal exit, cancellation, overflow, and provider loss remain distinct.

Cancellation follows the exact finite sequence: close stdin, send the named
graceful signal, wait the planned monotonic interval, force termination when
needed, wait again, and record cleanup. There is no retry, restart,
daemonization, child adoption, or process-tree escape.

## Bounds

The exact plan exposes stdin, stdout, stderr, chunk, queue, process,
child-process, pending-operation, descriptor, environment, timer, work,
evidence, and cleanup ceilings. The allocator-free reference profile permits
one process, zero child processes, at most sixteen arguments, sixteen explicit
environment additions, 65,536 bytes per stream, 4,096 bytes per chunk, and 128
lifecycle evidence events.

## Conformance

The allocator-free deterministic provider covers literal shell
metacharacters, empty argv, finite stdin, independent stdout/stderr, zero and
nonzero exit, signal exit, output overflow, deadline, cancellation before and
after spawn, graceful and forced cleanup, ignored termination, spawn failure,
and invalid limits. Hosted conformance additionally covers exact executable
binding, missing or revoked grants, stale provider observations, concurrency
exhaustion, environment and secret redaction, child cleanup, and honest
unsupported browser/constrained hosts.

Higher-level providers may wrap this primitive only behind their own typed
semantic contracts. Process bytes never become media, speech, model, or other
domain values merely because a child program produced them.
