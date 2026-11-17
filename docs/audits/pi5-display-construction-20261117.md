# Checked Pi display construction

Current dev omitted Pi 5 from the linear-framebuffer package's target, implementation and presenter closure. It also lacked the checked explicit scanout/native-Mask initialization types already used in the deployed N cohort. This cut extracts only that construction code from b341d1c619, adds the exact existing Pi 5 target and exports the checked types. It initializes no hardware and grants no display, memory, input or playback authority.

The constructor requires explicit KMSDRM geometry, bounds the raster, preserves the authored surface/Show/runtime-memory/input capacities, and refuses insufficient or ambiguous pools. It does not resize them from incoming Face content.

Validation: workspace Make owning library 9 passed/0 failed, including three new construction tests. Strict no-dependency Clippy with lib/tests and -D warnings passed for conduit-workspace-make and conduit-linear-framebuffer-make. Rustfmt and diff hygiene passed. Tests use public checking and actual package/catalog composition, not a fabricated checked configuration.

Threebee's actual installed Host source rechecked with unchanged configuration ID sha256:d0ffc42a6b6d0e64bff302e8a4dc0f0955dff6c39cbd011132b69347e67df095, 1024x600 and 2457600 surface bytes. Motherbrain's complete proposed display-plus-speaker source progressed past the display refusal but remains refused because its pinned std/hosted-whisper-clip-speech@2 is absent from current dev's catalog. That microphone declaration was not removed or weakened. The synthetic Pi 5 display/speaker composition passes; the complete Motherbrain profile is NOT validated for deployment.

The first isolated build failed on temporary disk quota before tests. Released compiler metadata and failed incremental state were reclaimed; the same test scope resumed with incremental compilation disabled. No compiler was restarted merely because an observation timed out. Source, proof executables, generated inputs, libraries and logs were preserved.

No Root, service, Pi, MCU, sound or display effect was performed by this source cut. This is not #6886 completion, runtime IMAGE funding, Started authority, Ambient acquisition, graphics resource delivery or audible proof. Queue retains the first-next #6886 slot; no competing PR/Candidate was opened.
