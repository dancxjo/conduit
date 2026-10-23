# Native Pages checkpoint publication repair

Issue #6261. The publisher for green Integration `39864702975` failed in run `39873151863` at checkpoint 18. Its tested source was `113ab8ee0f81f5a3c3b038c038010a6730797f92`; the staging helper is byte-identical on base `f0dbc3484878e7325382f956da92e942ee039785`.

The normal collector explicitly captures `patchbay-face` with `expected_change: false`, while the documentary composer required mandatory change. The retained real fb7 QEMU packet reproduces that refusal. Changing only the synthetic fixture to match the collector also fails against the unchanged composer; both negatives remain here.

The fix changes only that expected-change declaration. The composition fixture now follows the collector, and an inverted declaration is explicitly rejected. Source, image, Boot, Body, hash, dimensions, paired evidence, pointer, Look and text-edit validation remain enforced. All 305 pipeline tests pass, including 29 documentary composition controls. The corrected staging command completed against the retained real QEMU packet, and all 62 original files still match their sealed hashes.

`manifest.json` records the exact two source files, runtime, command and raw log hashes. A subsequent evidence-only commit preserves both measured source files. This is local documentary composition against retained emulator evidence, not a fresh QEMU run, hosted publisher success or public Pages acceptance. Candidate, normal merge, relevant Integration and verified release/Pages identity remain required before closure.
