# Finite checked USB Audio candidate

Supported candidate: USB Audio Class1.0 at USB2 full speed, configuration1,
AudioControl interface0, AudioStreaming interface1 alternate0 idle/alternate1,
TypeI PCM signed16 little endian, stereo left/right,48000 samples/second.
Synchronous ISO OUT01 has maxpacket192 and bInterval1 (one millisecond), no
feedback endpoint. Each packet holds48 stereo sample frames (192 bytes).
The exact100-byte configuration is recognized in checked Source. SET_CONFIGURATION,
SET_INTERFACE, endpoint SamplingFrequency SET_CUR48000 and GET_CUR exact48000
are explicit class controls; unsupported descriptors/rate/channel/layout refuse.

PCM meaning reuses semantics/audio/types.conduit PcmFrameHeader, including clock,
start-frame, discontinuity and exact payload extent. USB packet cadence reuses
usb-pcm-packet-bytes/usb-pcm-next-phase in protocol.conduit. SignalCadence regular
(samples per Quantity) remains the generic sample-clock meaning; USB controller
frame IDs are a separate domain, explicitly supplied per packet. No host assigns
sample clock, converts formats or retries class negotiation.

Native currently retains one original possession for at most8 TDs and completes
that HostCall only after every TD completes or acknowledged all-stop. Therefore
successfully repeating8-packet batches does not establish continuous service:
the ring empties between batches, and completion14 must remain underrun. First
actual QEMU execution must retain this truth. If continuous rolling refill is
needed, a separately reviewed larger finite explicit-frame request must keep the
same original claim while refilling free native slots. Source owns bytes and
clock conversion; native must not hide cadence, repair or retry.

`audio_device.py` is an external emulated device, adapting the existing bounded
usbredir framing fixture. It uses only one freshly owned Unix socket, a finite
transcript and at most128 packets. ISO OUT usbredir has no per-packet reply: its
transcript proves received PCM bytes, while native QEMU event evidence proves
controller completion separately. Neither proves audible output. It is an
internal proof harness pending integration through cargo xtask.

Authorities: USB-IF Audio1.0 https://www.usb.org/sites/default/files/audio10.pdf,
Formats1.0 https://www.usb.org/sites/default/files/frmts10.pdf; SPICE usbredir0.15.0
https://www.spice-space.org/download/usbredir/usbredir-0.15.0.tar.xz
(usbredirproto.h and docs/usb-redirection-protocol.md).

Source-only candidate. No native ISO execution, continuous streaming, acoustic,
physical-device or issue-closure acceptance claim yet.

`check_audio_wire.py` requires at least72 actual192-byte packets, exceeding the
64TRB native ring, and independently decoded native completed batches with
contiguous explicit frame IDs. Its receipt must retain original PCM/clock/start,
selected topology, exact negotiated format, actual native batch outcomes and
original acknowledged stop/retired possessions. Underrun or a scheduling gap
refuses the continuous-reuse acceptance check; this does not erase its evidence.
No receipt is supplied by this candidate, since no guest Audio run occurred.

The factored batch policy is `audio-batch-back.conduit`:24 small checked truths
cover8 exact PCM blocks,8 byte/frame correspondences,7 adjacent clock/sample/
USB-frame continuities and the transport shape.23 finite Boolean zip/and gears
compose them. `audio-negotiation-back.conduit` is the ordinary setup-once control
route for the exact descriptor, configuration, alternate, SET_CUR and GET_CUR.
Both composition candidates still need exact generic pair/catalog preparation
and ordinary kernel tests; passing the small pure policies does not prove these
routes installed. The singlepacket clock projection is checked. Extending it
through a sequence requires the separate conservative byte-contract proof fix.

The current integration adds `audio-playback-policy.conduit` and
`audio-playback-back.conduit`: a typed state loop retains one original PCM block,
observes a real controller snapshot, submits eight exact 192-byte projections,
correlates every completion deadline and extent, and requests drain. Its native
catalog preparation and the 15-test Source suite passed at commit `0687b401a`.
These checks did not execute USB transfers.

`audio-admission-back.conduit` connects setup acceptance and the block laws to
that loop using exact typed finite pairs. This full entry is `usb-audio-playback`;
its serialized-entry preparation test passes with one control and one ISO
placement. Public packaging also passes (80,383 bytes, 45 expanded gears). Generate its Source and generic
specialization inputs with:

```sh
python3 targets/conduitos/plots/usb/proof/audio/build_source.py --output /new/audio-input
cargo +stable xtask make conduitos protocol-source \
  --source /new/audio-input/playback.conduit \
  --specializations /new/audio-input/specializations.json \
  --entry usb-audio-playback --output-dir /new/audio-package
```

The assembler performs no checking or physical effects. The current loop covers
one eight-packet block. Continuous refill across at least 72 actual packets,
64-TRB reuse, fault/cancellation coverage, cross-architecture execution and
physical output remain required. Keep the existing continuous wire checker:
one successful block cannot satisfy it.
