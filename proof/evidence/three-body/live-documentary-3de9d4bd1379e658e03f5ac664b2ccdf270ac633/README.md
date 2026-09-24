# Retained live conversational documentary

Seven actual Ollama/Gemma 3 4B Presenter requests crossed ordinary Plan/Play at
the source commit named by this directory. The requests retain the resident
tutorial guidance alongside structured Body facts and exact action identities.
Multiple semantic moments sharing a captured state reuse that same recording;
they do not claim another inference or user interaction took place.

The `.txt` files contain exactly the outward `Speech` segments. Presented thought
remains in the original execution receipt, not in the voiced transcript. Piper
voices those retained words offline; MP3s and measured waveform PNGs are documentary
derivatives, **not** evidence of Conduit's runtime TTS chain or physical speakers.
Lossless WAV sources and tool/model digests are retained alongside them.

This is a historical live recording, not inference on each new release commit.
Publication checks that every retained request—including source Presentation,
policy, actions and bounds—equals the corresponding current fixture-run request.
A changed input refuses reuse. The routine release gate never starts a live model.

The model's limitations remain audible: some responses are terse or repetitive.
No words were rewritten to improve the result. Two pre-birth moments have no voice
because no Body exists yet. The original September 23 single-state conformance
recording remains separately preserved in the sibling `live-ollama-*` directory.

Repository entrances:

```sh
cargo xtask --json host prove-local-model --model gemma3:latest \
  --admitted-memory-mib 8192 --orifina-presenter --journey-documentary
cargo xtask evidence journey-audio \
  --track target/journeys/three-bodies/hosted-generative/track.json \
  --piper /absolute/path/to/piper --model /absolute/path/to/voice.onnx \
  --config /absolute/path/to/voice.onnx.json
```

The first command requires an already-local model and refuses to overwrite a
previous track. Exact model content identity is in each Presenter receipt; the
local alias above is not a pin. The second requires Piper and FFmpeg explicitly
installed for this one-time recording. They are not new release-run dependencies.
