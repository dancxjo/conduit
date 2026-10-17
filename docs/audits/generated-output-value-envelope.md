# Generated-output semantic refusal before storage

Development slice of #5231 pressure/ownership preservation, 17 October 2026.

Generated canonical and prepared output was stored before ordinary output
preflight checked each cord's `maximum_value_bytes`. With available storage the
kernel ultimately returned semantic refusal and released the generated value.
With storage pressure the same invalid value instead returned a storage error,
masking the immutable semantic limit and performing avoidable materialization.

Generated-output materialization now checks the exact payload routes' semantic
envelopes before storing bytes. Prepared output still first proves its exposed
byte length matches its staged declaration. Existing materialization ownership
and rollback behavior remain in place. The responsibility moves from the large
scheduler root into `payload_admission.rs`. Existing `StepIo` output allowance
and refusal spellings stay unchanged; clipping the allowance would have changed
the established semantic refusal into `OutputBlocked`.

The change adds one bounded traversal of the generated output's admitted route;
there is no new persistent index or allocation. This is a refusal-order and
ownership improvement, not a performance speedup claim. Already-stored outputs
retain their existing preflight path. Non-payload terminal tracks do not constrain
ordinary payload values in this check.

Four cases cover canonical vs prepared output and empty vs full value storage.
A five-byte generated value sent toward a four-byte semantic envelope returns
`SemanticValueBoundExceeded` in every case. The source stays ready/uncommitted,
the cord remains empty, Sign count remains the pre-Step fuel record, and a
preexisting held value preserves its bytes and reference count. Disabling the
new check reproduces `Storage(ByteCapacityExceeded)` instead of the semantic
refusal. The check is restored before final validation.

Both full local kernel suites pass with packed and linear ready selectors;
strict all-target kernel Clippy, formatting and diff checks pass. No CI was
dispatched or awaited. Browser/physical/stable acceptance and parent completion
remain outside this native kernel claim.
