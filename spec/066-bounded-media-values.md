# Bounded media values

Status: current pre-release specification.

## Ownership

`conduit.media/*` owns host-language-neutral audio frames, image/video frames,
encoded packets, stream descriptors, and rational media time. Generic
provenance, sensitivity, envelopes, clock correlation, pressure, cancellation,
and evidence remain platform contracts. Codecs, containers, devices, and
FFmpeg/SoX data structures are outside this value foundation.

Every descriptor states finite dimensions, frames, planes, strides, metadata,
and bytes. Time uses a non-zero rational basis, an explicit timestamp and
duration, a discontinuity marker, and conversion uncertainty. Exact descriptor
identity is compatible; any format, layout, time-base, or packet-extradata
change requires a separately named adapter.

The current descriptors are:

- `conduit.media/stream`: a nonzero SHA-256 stream identity, rational time
  base, maximum frames and bytes per value, maximum metadata entries, and
  maximum buffered values;
- `conduit.media/audio-frame`: sample representation, rate, named channel
  layout/order, channel and frame counts, finite planes/strides, and byte
  ceiling;
- `conduit.media/video-frame`: width, height, pixel format, color space,
  range, transfer, orientation, alpha, finite planes/strides, and byte ceiling;
- `conduit.media/packet`: codec, profile, extradata identity, key and
  discontinuity flags, rational timestamp/duration, and byte ceiling;
- `conduit.media/metadata`: at most 64 key/value entries and 64 KiB total,
  with exact provenance identity and preserved sensitivity; and
- `conduit.media/time`: rational time base, one timestamp, nonzero duration,
  discontinuity, conversion uncertainty, and optional media-timestamp to
  host-tick correlation evidence.

Timestamp sequences are monotonically ordered and reject duplicates. Exact
packet compatibility includes codec, profile, extradata, time base, and byte
ceiling. Exact raw-frame compatibility includes the complete format/layout
descriptor. Per-value flags and correlation observations remain evidence; they
do not silently rewrite descriptor compatibility.

Understanding these values does not claim that a host offers media operations.
No descriptor triggers discovery, conversion, download, device access, or
allocation.

## Deterministic profile

The first proof uses fixed PCM and image bytes with integer arithmetic only.
Descriptor hashing is SHA-256 over the documented canonical UTF-8 descriptor.
Tests reject zero dimensions, missing timestamps, invalid plane/stride layouts,
unsupported formats, channel-layout drift, packet-extradata drift, metadata or
byte overflow, pressure overflow, cancellation, and unsupported hosts. The
standalone and composed panels use the production exact-plan executor in both
the CLI and browser registry; cancellation is injected before the first node
step and undersized cord bounds produce an explicit pressure rejection.
