# Tongues brownfield proof

This crate adapts exactly one pre-existing Tongues starter into ordinary Conduit boundaries. The
specimen is `StarterGraph::TextToSpeech` from
`crates/tongues-pipeline/src/starter.rs` at Tongues revision
`5748f20ee4fd133be6a9332b01d96dc0649b26a3`. The crate calls that pinned upstream API and checks
the original graph remains `text_source.out -> tts.in -> audio_output.in`.

The authored Conduit Form retains only text-to-speech-to-audio meaning. Planning separately seals
the exact implementation, artifact, Host, Boot, output Base resource pool, authority grant, host
operations, and capacity-one/32,768-byte Cords. Play uses the production `conduit-kernel`
scheduler and its admitted host-operation table. There is no Tongues dispatcher in the execution
path.

Two output conditions are deliberately different:

- primary playback proves submission to an admitted audio-output operation; it does not claim a
  human heard the result;
- degraded output proves production of a bounded WAV artifact and explicitly does not claim
  playback or persistence beyond the admitted artifact operation.

The deterministic PCM fixture makes the boundary repeatable; it is not a production voice model.
Receipts expose a PCM digest and bounded kernel Sign digest, not source text, PCM samples, model
contents, or device details. Format mismatch, pressure, cancellation, underrun, unavailable
implementation, Base denial, Base loss, and output failure remain distinct outcomes.

Run the focused repository proof with:

```text
cargo xtask demo tongues --json
```

If the repository-wide `xtask` binary is blocked by an unrelated workspace dependency, the owned
proof remains directly testable with `cargo test -p conduit-tongues`; that narrower Cargo command
is diagnostic, not the documented repository entrance.

The stop line remains one starter: no broader Tongues migration, voice marketplace, cloning, or
studio/DAW surface belongs here.
