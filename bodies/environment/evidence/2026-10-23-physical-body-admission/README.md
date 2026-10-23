# Physical ESP32 Body admission

The recorded firmware was flashed successfully to the attended HW-463 ESP32
with MAC b0:a7:32:28:61:78. `cargo xtask prove environment body-admission-esp32`
passes against that actual Boot. Root refuses a stale-Boot challenge, preserves
its current key and advertisement, then signs the ordinary Body challenge.
The host's ordinary AdmissionManager refuses a tampered signature, admits the
valid Host proof and refuses replay. Valid Body biography evidence is retained.

The public key was authorized through the attended UART of the expected board.
This is not image attestation or an unattended device trust bootstrap. The born
Body and membership evidence reside on the preparation host; nothing claims the
MCU installed that Body, acquired effect authority or started an appliance Play.
The final Root offer confirms unchanged provider facts and installation false.

The first run verified the signature but failed to append biography because its
membership record sequences preceded the fresh Body's randomized birth sequence.
The host producer now advances from that actual birth sequence. The failed
capture remains under target/environment-body-admission/1792782485; final proof
includes the physical stale-Boot refusal. No firmware retry or reflash was needed.
