# Shared MCU control boundary

This `no_std` package shares the existing membership and signed preparation-input
protocols across native Host compositions. It contains no Root implementation,
planner, execution kernel, sensor/display loop, or application dependency.
It does not yet install a complete MCU Host or replace running firmware.

`membership::SpawnAdmission` borrows independently retained construction and
actual Host/Boot/IMAGE facts. Only the pinned owner's verified current-Boot
receipt establishes the returned membership. A retained credential refuses this
fresh-spawn entrance before carrier effects; retained-Part return is a separate
unfinished extraction. No fallback invitation is manufactured after interruption.

The arrival message preserves the existing admission wire representation while
borrowing the advertisement, friendly label and fixed public key. Serialization
therefore does not clone decoded capability metadata. Its byte-equivalence and
allocation tests cover only header serialization; signatures/transcripts,
receipt decoding and the caller's retained advertisement remain separate costs.
No whole-exchange or whole-firmware memory-fit claim follows.

`transport::Link` reuses Wire's finite stream decoder and bounded document
assembly. It preserves u16 big-endian framing, the 4096-byte payload ceiling,
64-byte carrier packets, 30-second read bounds and 2-second write bounds, including
an empty final USB packet. UART may implement that empty write as a no-op.
The physical Carrier must enforce deadlines and stop using borrowed buffers on
cancellation. Reset discards partial framing, not membership or execution state.

`ingress::receive` authenticates an exact challenge and payload, checks installed
preparation policy, then retains the bytes in the existing fixed
`HostCommandInput`. Its ACK explicitly reports RAM custody, resources not admitted,
and work not started. Lost ACKs do not withdraw custody, and a second receive
cannot overwrite it. A caller may report the same retained digest after reconnect.
This ACK is neither durable recording acknowledgment nor execution authority.

The native composition remains responsible for independently verified membership
and construction, an unpredictable unique nonce, a monotonic clock, and wrapping
the **whole** ingress future in its 10-second deadline. Per-packet read bounds are
not a transaction deadline. It must retire interrupted framing before a fresh
exchange and fund/control all caller buffers. Local execution owners must remain
independent of carrier lifetime; this package does not start or cancel them.

Body's `MemberBootstrap` is private construction material, not an additional Seed
framework or proof of current membership. Shape/scope validation does not validate
a retained credential or bind its secret to a Part. Prior-Boot credentials remain
historical until authenticated return. Pinned owner Boot/generation/key checks
remain exact; owner restart needs an authenticated refreshed selection. Dropping
bootstrap material wipes its owned secret copy only, not flash, input carriers or
separately derived signing keys. It deliberately has no Debug or Clone.

The normal Foundation shard includes this workspace package. Its tests exercise
actual signature/attestation checks, stale and wrong context refusal, bounded
framing, signed input custody, interruption and lost ACKs. They are hosted protocol
proof, not physical USB/UART, admitted Play, persistence or overnight proof.

This extraction reuses the control mechanisms from the existing MCU delivery
checkpoint `55d9f409ed`, refreshed onto current `dev` after #6886. It deliberately
omits the old application factories, Root wrapper, construction-carrier decoder,
image-return family and continuation subsystem. A configuration/profile fence
formerly supplied by the Root wrapper now lives in the shared preparation-policy
check; a signed wrong-profile regression verifies refusal before input retention.
