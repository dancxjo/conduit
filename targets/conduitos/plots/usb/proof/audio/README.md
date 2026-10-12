# Checked USB Audio playback proof

The supported profile is UAC1 at USB2 full speed: configuration1,
AudioControl interface0, AudioStreaming interface1 alternate0 idle/alternate1,
TypeI signed16 little-endian stereo PCM at48000 samples/second. Synchronous
ISO OUT01 has maximum packet192 and interval1, with no feedback endpoint.
Each packet contains48 stereo sample frames. Checked Source recognizes the
exact100-byte configuration and performs configuration, alternate selection,
SET_CUR48000 and exact GET_CUR48000 controls before playback admission.

PCM uses the existing PcmFrameHeader with sample clock, start frame,
discontinuity and exact payload extent. Controller frame deadlines are a
separate domain. Source owns PCM validation, negotiation, deadlines,
completion correlation and drain. Native code assigns no sample clock,
performs no format conversion and retries no class negotiation.

The entry is `usb-audio-playback`. It retains the original PCM block while
setup completes, validates its laws, observes a real controller snapshot and
submits Source-authored packet deadlines. Generic typed feedback retains this
state: finite zip discards unmatched values when an input closes.
`build_source.py --repeat-nine` selects `audio-repeat-playback-policy.conduit`,
which explicitly repeats the eight-packet clip nine times in one finite
72-packet request. The default selects the original one-block policy.

Native retains one original possession for the entire request, reusing eight
256-byte DMA slots and the63 ordinary entries of a64-entry transfer ring.
Only actual completion releases a slot for the next retained packet. Source
bytes and frames are preserved; native never repairs a missed deadline.
Public `observed` output preserves the original typed ISO result, including
fault tag and completed prefix. The result schema bounds prepared output at
7864 bytes. Native protocol output storage has a separate32768-byte ceiling;
Root ingress remains4096 bytes. The public Fore matches its selected Back's
exact contract rather than adding an unmatched output ValueContract.

## Retained evidence

`../../evidence/native-audio-public-cross-architecture/` contains unchanged
Source package03de725b and byte-identical Root runs on x86_64 kernel d2ab8e8a
and AArch64 kernel71e043c8b. Each run verifies public snapshot, all72 completed
frame/requested/actual records and drained,72 exact wire packets containing
13824 original PCM bytes, terminal phase3 with PCM retained, and bound
acknowledged Stop with zero Play alloc/realloc attempts.

`../../evidence/native-audio-provider-loss-d2ab8e8a/` retains actual pending
ISO device loss: four packets and768 exact PCM bytes, typed provider-lost with
four matching completions, no packets after detach, terminal phase1 and
acknowledged Stop with zero Play allocation attempts.

`../../evidence/native-audio-setup-bfa318cf/` retains actual wrong-rate GET_CUR
and SET_CUR STALL responses, each refusing before any ISO packet. Native
wire and canonical Source results are separate checks. These are emulator
proofs, not audible or physical-device evidence. All19 Audio Source tests
pass, including all72 completion extents and216 corruptions; the synthetic
Source fault fixtures do not replace native fault delivery proof.

## Reproduction

The assembler checks nothing and performs no physical effects. Package its
ordinary Source through the public validation entrance:

```sh
python3 targets/conduitos/plots/usb/proof/audio/build_source.py \
  --repeat-nine --output /new/audio-input
cargo +stable xtask make conduitos protocol-source \
  --source /new/audio-input/playback.conduit \
  --specializations /new/audio-input/specializations.json \
  --entry usb-audio-playback --output-dir /new/audio-package
```

`audio_device.py` is one bounded external usbredir device, with one fresh
owned Unix socket and at most128 packets. OUT ISO has no per-packet usbredir
reply; do not manufacture one. `--mode wrong-rate` returns44100 Hz after the
48000 Hz SET_CUR, and `--mode stall-rate` returns an actual SET_CUR STALL.
Its five unit tests establish device wire behavior, not native acceptance.

`check_audio_wire.py` retains the strict minimum72-packet, exact PCM and
contiguous native frame checks. Retained archives also include their exact
canonical decoders, independent wire checkers and bound Play receipt checker.
One eight-packet historical run cannot satisfy the repeated-service gate.

## Remaining acceptance

Short transfer, missed service, underrun, pressure, cancellation, failed-stop
quarantine and premature drain still require the composed native fault proof.
Five-architecture acceptance remains open. Physical/acoustic evidence is a
separate proof tier, not an invented prerequisite for this emulator slice.
Keep exact Source, kernel, Root and original token identities in every claim.

Wire authorities: USB-IF Audio1.0
https://www.usb.org/sites/default/files/audio10.pdf and Formats1.0
https://www.usb.org/sites/default/files/frmts10.pdf; SPICE usbredir0.15.0
https://www.spice-space.org/download/usbredir/usbredir-0.15.0.tar.xz
(usbredirproto.h and docs/usb-redirection-protocol.md).
