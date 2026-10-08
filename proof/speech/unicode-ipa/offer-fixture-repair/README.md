# Exact native-speech composition fixture repair

CI run 37858234436, hosts-std job 113588485338, found the existing native-speech fixture still expected only the utterance offer after this change added two IPA constructor offers. The exact added-offer assertion now requires all three complete canonical offers in order. Minimal composition rejects all three kinds; reference composition must contain all three complete offers, including their checked type profiles. No implementation changed.

The retained original failure log is verbatim. Final scoped tests pass: native_speech 3 tests and ipa_authoring_journey 7 tests. Scoped test Clippy passes with warnings denied. All 36 original IPA source hashes remain unchanged; the receipt records the prior base, changed test before/after hashes and test commit. This subsequent packet is evidence only. Shared Cargo target used four jobs, debug info disabled, incremental disabled; 33 GiB disk headroom checked before execution.
