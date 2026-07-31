# Bounded media codec and container operations

Status: current pre-release contract.

## Boundary

Media values do not imply codecs. Probe, demux, mux, decode, and encode are
separate node contracts with independently observable providers. A host may
know `conduit.media/audio-frame`, `conduit.media/container-chunk`, and
`conduit.media/packet` while reporting every operation provider unavailable.

The deterministic first profile is exactly RIFF/WAVE, PCM signed 16-bit
little-endian, stereo LR, 48 kHz, 192 frames, and empty codec extradata. The
profile, extradata, and checked fixture are identified by SHA-256 values in
the exact node configuration. No format guessing or ambient plugin lookup is
permitted.

## Operations

- `conduit.media/container/probe` validates the complete container and emits a
  bounded normalized summary.
- `conduit.media/container/demux` converts one exact finite WAVE value into one
  exact packet.
- `conduit.media/container/mux` converts that packet back to the exact finite
  WAVE value.
- `conduit.media/audio/decode` converts the exact PCM packet to the existing
  audio-frame value.
- `conduit.media/audio/encode` performs the inverse operation.
- `conduit.media/wave/literal` is only the checked content-addressed fixture
  source used by the first proof.

Every operation binds container, codec, profile, extradata identity, profile
identity, input/output bytes, track and packet counts, reorder depth, retained
bytes, metadata count, work, and terminal flush behavior. These facts remain
distinct from cord capacity, queue bytes, pressure, and runtime evidence.

## Framing and terminal behavior

Incoming byte chunks are accumulated only up to the exact retained and input
ceilings. All divisions of the same WAVE bytes normalize to the same track,
packet, timestamp, duration, and PCM payload. Truncation, trailing or malformed
bytes, unsupported bindings, timestamp reorder, and any finite bound overflow
fail before output is committed.

The first profile has one track, one packet, zero reorder depth, zero metadata
entries, and exact terminal flush. Scheduler cancellation produces the normal
cancelled terminal evidence and commits no fabricated flush output.

## Evidence and non-goals

The production executor records the exact contract, provider implementation,
artifact, typed ports, semantic configuration, cord bounds, work, cancellation,
and terminal cause. The checked conformance matrix is
`conformance/c4/media-codecs.json`.

This contract does not publish a universal codec, format guessing, FFmpeg data
structures, plugin installation, hidden buffering, device I/O, or support for
any profile beyond the one named above.
