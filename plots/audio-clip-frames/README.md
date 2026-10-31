# PCM clip frames

`unpack` drains one validated finite PCM clip into its original PCM frames. It
preserves byte contents, clock, sample offsets and a short final block. Pressure
waits without consuming the clip; malformed tails refuse before the first frame.
There is no padding, resampling, device selection or capture authority here.

This prepares finite recorded or captured audio for waveform/spectrum processing.
It does not make whole-clip capture a live source. Downstream analysis must admit
the actual frame geometry, including the final block. Std kernel conformance is
covered by `targets/std/tests/pcm_clip_frames.rs`; physical capture and shared
Body delivery retain their separate admission requirements.
