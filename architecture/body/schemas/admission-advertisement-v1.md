# Complete advertisement JSON over bounded admission carriers

The profile identified by
`conduit.body/admission-advertisement-deflate-json@1` carries the **complete
ordinary public JSON** of one `HostAdvertisement`. Its field envelope is:

```json
{"encoding":"conduit.body/admission-advertisement-deflate-json@1","expanded_bytes":51780,"zlib_hex":"..."}
```

`zlib_hex` is the lowercase hexadecimal encoding of one complete zlib stream.
The expanded document is the ordinary typed advertisement JSON, including every
resource, capability, semantic contract, configuration default, profile and
canonical byte. There is no dictionary lookup, abbreviated offer, digest-only
substitution, second advertisement truth, or change to invitation signatures.
Decoding reconstructs the ordinary typed advertisement before existing admission
and current-Host validation. It grants no membership, Plan or Play.

The profile has explicit, separate limits: expanded JSON is nonempty and at most
65,536 bytes; the compressed stream is nonempty and at most 24,576 bytes. These
are profile limits, not global semantic limits on `HostAdvertisement`. The
complete carrier additionally retains its own byte limit. Native serial joins
remain at 49,152 bytes; native binary WebSocket messages remain at 49,138 bytes.
A compressed stream at its profile maximum need not fit a complete carrier;
carrier admission must still check the complete envelope atomically.

The receiver bounds its input carrier before JSON decoding. It validates the
encoding tag, declared extent and hex extent before allocating the expanded
buffer. The inflater must finish one stream, consume every compressed byte and
write exactly the declared extent. Invalid hex, truncated streams, trailing or
concatenated streams, forged lengths and expansion beyond the declared extent
refuse. The resulting bytes must be valid UTF-8/ordinary typed JSON, including
existing canonical structured-value validation. Envelope string allocation is
bounded by the upstream carrier; the profile cap bounds subsequent expansion.

## Carrier versions and compatibility

- `conduit.conduitos/serial-spawn-observation@2`, protocol 2, uses this profile in
  `advertisement`. The legacy serial schema @1/protocol 1 uses the ordinary raw
  advertisement. New consumers accept both and reject a mismatched profile.
- `conduit.body/routed-admission-request@2` uses this profile in the nested
  request's `host_advertisement`. Its nested portable request and original
  signature transcript remain unchanged. Legacy routed request @1 keeps the
  existing raw advertisement JSON.
- `conduit.body/routed-admission-response@2` uses this profile in an admitted
  receipt's `host_advertisement`. The legacy response @1 remains raw. The owner
  replies with the version selected by the request so legacy clients receive
  their existing representation. New consumers accept both explicit versions.
- Standalone `PortableSpawnAdmissionRequest@1` and
  `PortableAdmissionReceipt@1` retain their existing ordinary JSON. Core public
  advertisement serialization and Application private transport are unchanged.

The existing generic `ROUTED_ADMISSION_REQUEST_SCHEMA` and
`ROUTED_ADMISSION_RESPONSE_SCHEMA` constants remain version 1. General hosted and
StdHost routes retain their existing raw advertisements and carrier limits,
including valid advertisements larger than this compact profile's expanded cap.
Only native join preparation explicitly selects
`COMPACT_ROUTED_ADMISSION_REQUEST_SCHEMA`. A sender must deliberately opt into
this finite profile; adding the native carrier never narrows generic admission.

A new version 2 sender requires a version 2 receiver. There is no implicit
re-signing, invitation retry, authority change or fallback after a refused
exchange. Compatibility means explicit legacy decoding and legacy responses to
legacy requests, not that an old receiver can interpret a new compressed field.

## Browser boundary

Browser serial ingestion reconstructs the entire advertisement before passing
it to the existing join/admission path. Streaming decompression stops and
cancels on overflow before accumulating an oversized output. Because ordinary
JavaScript numeric parsing can round valid Rust u64 values, this browser
carrier accepts only lexically plain, safe integer JSON numeric tokens. It
refuses unsafe integers, fractions, exponents and negative zero before parsing;
it never silently converts a number to a string or rounds a generation. This
is a narrower browser admission boundary, distinct from Rust's complete u64
roundtrip. The current full native product advertisement at generation 1 has no
unsafe numeric tokens and is covered by an actual Rust-generated fixture.

Observation rechecks cancellation and retained loader-receipt identity after
the asynchronous decoder returns. A cancelled, aborted or replaced loader
observation cannot publish a stale join.

The focused proof includes actual complete native advertisements at 64- and
192-byte Host/Boot identities, generation 1 and u64::MAX, original invitation
signature verification through `AdmissionManager`, both routed frame bounds,
and full NativeOwner receipt equality. This is host CPU protocol proof; live
TLS, protected-service guest bounds, QEMU and physical acceptance remain
separate checks.
