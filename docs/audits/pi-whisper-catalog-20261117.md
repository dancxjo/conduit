# Pi speech capability construction closure

The hosted Make package already registered the exact Whisper clip Base and its initialization contract, but built capability metadata only from `supported_nucleus_offers()`. That list intentionally does not contain the hosted Whisper clip provider. A Host pinning `speech/recognize-clip` to `std/hosted-whisper-clip-speech@2` therefore refused as unknown despite the matching Base being available.

Include that existing typed provider offer when constructing hosted Make metadata. This changes neither the nucleus runtime offer list nor any provider implementation, grant, authority, or resource bound. Its actual declared host-call and process-slot prerequisites are retained. A Pi5 display/Whisper regression verifies ordinary checked construction and refusal of a foreign revision.

Qualification on parent ad8142bf790e1007adbede5d22150e0fb00a1bb2:

- Workspace Make library: 10 passed, zero failed/ignored.
- Strict Clippy: workspace Make and hosted Make, library/tests, no dependencies, warnings denied: passed.
- rustfmt and diff checks passed.
- Rechecked the complete private Motherbrain source with only the previously proposed HDMI playback Base added; original microphone/model/path/hash declarations and all limits retained. Checked configuration: sha256:3e454e3cbb0f0428ddfee47d3836c5032d001a5e4ea04cd52aacc1a4efecbcfd. Scanout 1024x600, surface 2457600 bytes. No private paths or identities copied into this public artifact.
- Rechecked the actual Threebee source: unchanged sha256:d0ffc42a6b6d0e64bff302e8a4dc0f0955dff6c39cbd011132b69347e67df095.

These are construction checks, not provider liveness, firmware fabrication, Ready, admission, Started, or playback proof. No Root/service/device/audio effects occurred. #6886 retains the only publication/review slot.

The first local test invocation used a different debug profile and exhausted the local tmp quota during generated dependency output; the terminal failure was inspected. Reclaimed 5,081,863,654 bytes of inactive regenerable archives from the primary's released preparation-refusal cache, preserving its proof executables, generated inputs, and logs. Reran under the original low-debug profile and obtained the successful results above.
