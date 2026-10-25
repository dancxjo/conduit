# Std HostCall receiver retirement

The focused `installed_std_tests::remote_vision_conformance` tests use canonical
Source checking, expansion, exact planning and public remote-fragment admission
on persistent source and finite Vision Hosts. They are part of the ordinary std
package test corpus. The cancellation variant dispatches A's real Vision request,
stages an output, cancels A, and observes `HostCallCompletionRejected` on A while
its independently admitted source sibling remains live. Public release then
consumes A's receiver before B is admitted on the same Host and Boot with a newer
Play identity. The source sibling is also explicitly released and re-admitted;
its persistent Host and Boot survive, while its Play identity is fresh. It is
not one continuously admitted source sibling across A and B.

B reaches its own pending request with the complete same receiver-local
descriptor and exact input bytes. While B waits, bounded idle steps publish no
completion or motion output routing; B's identity, input and retained Signs and
its fresh source sibling's identity and Signs stay unchanged. Fresh B completion uses
the actual finite Vision provider exactly once, records one corresponding kernel
completion and drains. A compile-fail example on `release_remote_fragment`
checks that the released A receiver cannot subsequently be invoked.

These are receiver ownership and cancellation controls. The finite Vision
provider completes synchronously. The durable product owner likewise holds its
fragment while completing supported portable/voice calls synchronously; it does
not install a detached Vision callback dispatcher. No actual asynchronous A
completion after B is delivered by this fixture. That completion class, callback
revocation and Watch are unsupported by this scoped mechanism. Local numeric
request/value descriptors are not bearer authority: copying A's fields into a
trusted caller's mutable B receiver is not an old-owner authorization attempt and
is not tested as one. The compile-fail check proves consumption of a Rust owner,
not memory isolation, external cancellation, DMA quiescence or hard termination.

Run the focused installed fixture and public ownership example with:

```sh
cargo +stable test -p conduit-std-host --lib installed_std_tests::remote_vision_conformance -- --nocapture
cargo +stable test -p conduit-std-host --doc release_remote_fragment
```

This narrow proof does not complete every mechanism or fault class in #6409.
