# Typed printer-pool simulator

Development proof for #5557 uses `cargo xtask check print-pool`. The fixture
prepares ordinary Source, checked/expanded Plot, sealed Plan and the existing
hosted shared-pool controller. Two advertised provider models expose the same
portable PrintRequest/PrintLifecycle Types. Mono-fast supports monochrome with
at most two copies; color-slow supports both colors with at most ten copies.
Both support A4 and Letter, one-sided. The portable owner validates attributes
and the declared document-reference binding before including a provider.

Eligibility is fixed at preparation for the retained request. Changing the
request class requires new preparation and a new envelope. Unsupported color
is a typed admission refusal, never a fabricated unavailable-health report.
Only the generic MinimumPredictedCompletionThenCapacity policy chooses among
eligible members. Fixture observations and bounded duration forecasts have
exact Host/Boot/offer, operation, request digest, calibration and expiry custody.
Queue duration can make the mono provider slower; provider unavailability can
exclude it without changing the sealed Plan. Expired costs are excluded from comparison. As in #5228, fresh hard eligibility
can still admit with the explicitly recorded CapacityReference fallback;
that fallback makes no predicted-completion claim.

The same generic waiting implementation bounds operation count, per-requester
count and admitted request bytes. Requester turns with aging run before the
same cost admission. Both occupied members retain a capacity-refused waiter;
explicit completion releases capacity and permits that waiter to enter. This
queue owns metadata only, never a document payload. The portable PrintAttempt
protocol independently validates the selected fixture's simulated dispatch,
acknowledgement and completion reports.

The Kind is explicitly `fixture/print-protocol-model`, not `print/submit`.
There is no document dereference, platform print Host Call, spooler, CUPS/IPP
connection, wall-clock timing measurement or physical delivery evidence.
The declared fixture document grant is input to portable admission only; it
is not runtime authority to read a real document. These tests do not implement
request deduplication/lost-ACK fencing (#5558), physical acceptance (#5559), or
close parent #5229. Forecast milliseconds are deterministic simulated inputs,
not measured printer service rates or completion promises.
