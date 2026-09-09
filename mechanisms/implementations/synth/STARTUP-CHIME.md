# Conduit startup cue

The original cue is a 1.2-second gesture of softly overlapping 294, 441, and
588 Hz filtered triangle tones. The complete score, strength, envelope, and reference DSP profile
are explicit in [startup_chime.rs](src/startup_chime.rs). It uses the existing
integer synthesizer and contains no recorded audio, codec, or network input.

To create a file for listening from a repository checkout:

```sh
cargo xtask audio render-startup-cue --output /tmp/conduit-startup-cue.wav
```

Choose a new output path; the command refuses to overwrite an existing file.
It writes 57,600 mono signed-16 frames at 48 kHz, without opening an audio device.
`--dry-run` describes the render without writing a file. The renderer uses fixed
storage, accepts blocks of at most 256 frames, and stops immediately on cancel.
Callers must admit and retain each output block before requesting the next.
Different Host block sizes produce identical sample bytes.

This is the sound-design portion of [#3152](https://github.com/dancxjo/conduit/issues/3152).
It is **not yet an installed Form or a Body startup hook**. Audio playback,
exact Form admission, Wake delivery, and durable generic first-wake State remain
to be connected through the normal planner and kernel. No first-wake flag lives
in this renderer. Render conformance establishes the digital signal; listening
and physical playback remain different evidence.

The intended startup model remains: a Body wakes its installed Forms. Some
Forms draw, some listen, some publish, and some may make a sound. The default cue
is eligible once per ordinary Wake when audio is admitted; absence or denial of
audio must not prevent the Body from waking. The separately reusable first-wake
mechanism must retain its explicit identity scope across a surviving Body's
Host restart. This renderer makes no claim to provide that persistence.
