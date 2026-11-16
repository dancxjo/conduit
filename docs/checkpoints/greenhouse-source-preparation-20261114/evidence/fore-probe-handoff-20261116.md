# Authorized original Fore probe

Incremental patch after the fresh execution entrance (`c46f5ccae2`). The result is an authorized, callback-scoped probe, not a Started/Ready handle. No actual device read or remote Start receipt has been proved by this patch.

## Consumer entrance

```rust
retained_execution.probe(&owner, &exact_member_host, |probe| {
    let binding = probe.binding();
    // Validate actual authored request/result Kind contracts against these
    // original typed Fore pins and source Plot, before choosing codecs.
    // Then call probe.exchange(request, output, timeout, consume_before_ack).
    Ok(())
})
```

The callback is higher-ranked and synchronous. It cannot detach the original descriptor or return its mutable transport borrow. Merely selecting the borrow or reading its metadata performs no transport IO. A callback can attempt at most one exchange; timeout must be positive and no greater than the original reviewed selection's wait (already at most ten seconds). Original frame, payload and buffer bounds remain enforced by `PicoForeCarriage`.

`binding()` provides borrowed original Body, member Host/Boot/generation, Source Plot, reviewed image/configuration/USB serial, exact typed request/result Fore pins, session bindings and original approval transcript. These are descriptive bindings, not permission to create a new Play or retire observations.

## Custody chain

1. Direct native provider selected actual LOCAL USB on the original authenticated Return descriptor.
2. During the existing Owner approval callback, `DirectForeMaterial::prepare_carriage` validates the original approved payload, captures its original attribution transcript and exact bindings, and refuses replacement.
3. Consuming socket conversion now retains that original material. Missing input report, material, original approval, preparation or successful active handshake cannot yield a probe.
4. Every probe calls `Owner::verify_selected_fore_probe`. The hook requires actual recovered workset/original File guards/current local Fore/current native member and observed offer, checks actual Plan/Wake/Body/workset revision, and borrows the UNIQUE original retained Part/Host/transcript command matching the actual current issued/prospective BodyPlay identity.
5. The client recomputes original bindings from that borrowed original command payload and transcript before invoking the callback. Copied reports cannot supply this command.
6. Exchange consumes the application's result callback before the existing transport result ACK. Errors retain the same owned transport; no descriptor is reopened or cloned.

The existing public `ForeExchangeOutcome` is reexported by Std so callers can distinguish Pressure from Delivered. No parallel transport trait, frame format, type or authority protocol was added.

## Proof and remaining gates

- PASS: generated real child/projection/BoundFore fixture; missing approval, wrong transcript, altered payload and replacement refusal; exact borrowed pin metadata.
- PASS: nine fresh Source execution selection/claim tests.
- PASS: strict product binary/tests Clippy.
- Still required: N consumer integration and a real bounded LOCAL USB read; current-Owner rejection evidence under actual stale member/Body/Plan/Source transitions.
- The consumer must select/check actual application codecs, correlate exact requested record index and preserve original measurement clocks/provenance. Transport ACK is not durable recording commitment or evidence-retirement permission.
- Successful CNDF negotiation and even a useful authorized response do not manufacture a remote Start receipt.
