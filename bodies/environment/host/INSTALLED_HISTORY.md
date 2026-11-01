# Installed CPU History provider

Run `cargo xtask prove environment history-installed-cpu` for the owning
installed CPU integration tests and strict adapter lint.

The optional `history-source` library entry
`installed_history_source::install_cpu_history_source` consumes a real
`ReadyHistoryBase`, an independently authorized current grant and an actual
`StdHost`. It installs through the existing BaseRegistry, resource ledger and
selected-operation issuer. Refusal retains the actual owner. An observer or
recorder membership receipt cannot supply execution authority.

The adapter retains the same ready owner. Attachment admission keeps the
current provider retained-layout plus fixed raw-buffer minimum; it does not
revert to a fixed 2048-byte provider budget. It validates the prepared canonical
History request, reads at most 2048 raw signed bytes, and encodes the existing
2304-byte History result. Original signatures, Body/Boot/Play, sequence and source
time remain opaque unchanged bytes. The existing environmental Source's 1/128
limits remain unchanged. Base implementation identity and operation
implementation identity are separate; exact registry binding, grant, artifact
and provider identity are still independently checked. The native definition
retains the actual independently configured capability envelope.

This first installation is explicitly CPU-only: an already retained finite
in-memory archive, no file/network IO, allocation, sleep or unwind during read.
Codecs and a refusal message are prepared before installation. The sole Std
`BoundBaseOperation` authorizes each invocation; the adapter neither creates a
second capability table nor calls the legacy History issuer. The unsafe native
handoff in `ReadyHistoryBase` requires this actual selected authorization.

One child Play admits one operation, two bounded value slots, input/result bounds
2304/2304 and the existing 16-Sign Back budget. The same native archive persists
across child Plays. This does not inherit the proof kernel's 240-operation scope,
4096-Sign store or 9216-byte store. A larger retained session needs independently
admitted work, not silent enlargement of this profile. Retained native-owner
correspondence is a trusted initialization obligation, not whole-process RAM
containment.

The owning CPU tests execute the authored History Source through the ordinary
planner, actual Std registration/selection, kernel HostCallBack, Cord and opaque
committed-Fore witness. They cover independent grant, Boot, artifact, pool and
Base-binding refusal, malformed canonical input, oversized native output and
affine installation refusal. A thread-local allocator probe around the actual
selected native callback verifies zero allocations inside request validation,
provider read and prepared encoding on success and refusal; it makes no claim
that the whole Std run allocates nothing. The separately owned environmental Source/worker tests cover their own
limits, cancellation, continuity and backing. This CPU route does not run
or establish those worker contracts.

This is installed native-provider admission, not Body admission, an installed
NAT route or recorder acceptance. The application still needs actual Body/Boot
execution authorization, an admitted two-Cord Plan, the observed outbound
protected Line, bounded transport work and the durable collector. Transport
acceptance and delivery never authorize durable-custody ACK or evidence erasure.
