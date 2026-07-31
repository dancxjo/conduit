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

Understanding these values does not claim that a host offers media operations.
No descriptor triggers discovery, conversion, download, device access, or
allocation.

## Deterministic profile

The first proof uses fixed PCM and image bytes with integer arithmetic only.
Descriptor hashing is SHA-256 over the documented canonical UTF-8 descriptor.
Tests reject zero dimensions, missing timestamps, invalid plane/stride layouts,
unsupported formats, channel-layout drift, packet-extradata drift, metadata or
byte overflow, pressure overflow, cancellation, and unsupported hosts.
