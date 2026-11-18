# Checked Source to admitted CPU Play

This is a host-only qualification of the same checked three-fragment factory used by MCU firmware. It uses fake electrical/recording providers gated by actual authenticated Start; it does not install a Host, change a Body, flash a device or prove MCU stack/heap containment.

From the repository root, with an isolated caller-owned cache and a fresh output path:

    python3 targets/mcu/source-build/tests/cpu_factory/make.py --configuration targets/esp32/profiles/blue-wroom32.host.conduit --image /path/to/checked-image.json --workset targets/mcu/source-build/tests/fixtures/cpu-environment.conduit --source-revision da2f2a0b2e86cf3fe1a56639ff4730653e5e1eb0 --entry yellow-sampling --entry yellow-mask --entry yellow-output --cache target/cpu-factory-cache --output target/cpu-factory-result

Add --offline only when dependencies are already available. Compiler process memory limits belong to the runner; they do not change the checked Host's 118784-byte arena.

The recipe builds the explicit source-emission Cargo example, calls the SAME Make emitter as firmware, generates immutable calibrated inputs, reads that completed Cargo graph and compiles the actual CPU factory. It then runs six-sample local sensing/Face/Mask/output with transport loss and the exact observed resource shortage case. It needs no previously generated Rust, copied libraries, hand-made fingerprint wrapper, firmware source include, HAL, flash utility or connected device. Cargo's internal fingerprint representation is confined to this test harness.

The authored Source is committed; the public checked IMAGE is a caller-supplied Make output and is not imported into Git. The revision above is the original Make identity input whose ordered Source digest must match that golden IMAGE. The generator independently verifies configuration, artifact and Source correspondence before calibration. Replaying a golden CPU test is not fabrication or admission of a current physical Host: new deployment must use the actual current Source revision, newly checked Make IMAGE and ordinary Body authority.

The raw generator is also available as:

    cargo run --locked -p conduit-host-esp32-make --features source-emission --example emit-source -- OUTPUT_DIRECTORY

That lower-level entry requires the existing CONDUIT_HOST_CONFIGURATION / HOST_IMAGE_ID / SOURCE_IDENTITY / HOST_IMAGE_DESCRIPTION + SHA256 / WORKSET_SOURCE + SHA256 / WORKSET_ENTRIES environment inputs. make.py prepares and checks them explicitly, and rejects an IMAGE that does not match the ordered revision/Source/entries.

Generated resident adapter code is shared with the ESP32 caller through Source composition. CPU diagnostics use the shared preparation_trace codec. No firmware-directory file is imported by the CPU proof or Make generator.

Receipts retain exact input hashes, generated non-Source hashes, dependency features, fixture/template hashes, positive and shortage outcomes. Whole-Play aggregate retirement, power-cycle/history continuity, physical MCU fit and installed Body Return funding remain separate obligations.

## Stop checkpoint, 17 November 2026

The final source-only recipe passed at code8fb6d62638, reproducing all eleven calibrated inputs and the full positive/shortage CPU cases. CHECKPOINT.json pins scope and log hashes. Owning Make16/default12, runtime136 withoneexistingignore and native-sync21 passed on the earlier ab994 cohort; later strict results retain their own cohorts. This is not a full final-tree/current-dev or physical qualification.

The compiler-cache selector now identifies the successful emitter binary by exact bytes; it does not select a Clippy-only fingerprint merely because feature names match. Initial ambiguous-cache refusal and lint diagnostics remain retained. The captured public IMAGE stays outside Git as an explicit checked Make input; no generated Rust, rlibs, secrets or device image is committed.

User requested source/issue handoff and stop. Further implementation/deployment is paused. Complete Return/cold funding, actual Root paired Source initializer and physical MCU lifecycle proof remain open; no live Root/Pi/MCU changes were made by this work.
