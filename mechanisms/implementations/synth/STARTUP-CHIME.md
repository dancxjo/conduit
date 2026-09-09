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

The [Startup Chime](../../../forms/startup-chime/README.md) and
[First Wake Chime](../../../forms/first-wake-chime/README.md) Forms now use this
renderer through the admitted browser audio operation. The shared browser
workspace offers the ordinary cue by default when its audio API is available;
either Form can be installed or removed through the retained Body's Forms
chooser. See the Form guide for the runnable entrance and browser proof.

No first-wake flag lives in this renderer. The generic `body/first-wake` source
derives eligibility from retained Body lifecycle evidence. Render conformance
establishes the digital signal; browser execution, listening, and physical
playback remain different evidence. Native audio support is separate work.

The startup model is: a Body wakes its installed Forms. Some
Forms draw, some listen, some publish, and some may make a sound. The default cue
is eligible once per ordinary Wake when audio is admitted; absence or denial of
audio does not prevent the Body from waking. The separately reusable first-wake
source retains its Body lifetime scope across reload with a fresh Boot. It
does not repeat when a Form is added later or a Plan is replaced. Started
evidence is saved before effects dispatch; a crash can still prevent audibility,
so this is not an exactly-once physical-delivery guarantee.
