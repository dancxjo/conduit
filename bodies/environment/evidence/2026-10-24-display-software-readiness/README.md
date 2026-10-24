# Actual ESP32 output software construction

The supported `cargo xtask prove environment display-output esp32-readiness
--flash` completed on the attached ESP32 WROOM, MAC `b0:a7:32:28:61:78`.
The clean firmware source is `e134d947a06acdf41e9277f5582d635748f76a6d`.
`image.json` pins the saved ELF SHA; the ELF stays in the attended local artifact
store. This is a host-controlled flash/capture record, not device attestation.

Root actually constructed and retained the three portable output computations
from trusted Make material. The startup control reply reports output software
ready, sampling software absent and no installed Body or native Plan. Root's
reported heap usage is 63,192 bytes and peak 74,432 of 114,688 bytes. The build
passed the unchanged 28 KiB CPU stack floor. The first diagnostic link failed
that floor after a status-buffer expansion; restoring its previous capacity
fixed the build. Neither the stack guard nor heap totals were lowered.

The image remains in its separate readiness profile after this finite proof.
No output Plan was installed, no Mask Play started and no packet was sent to
the physical display. Sampling and output construction were not combined.
Authenticated output installation, shared memory/work admission, recurring
Mask/Look pages and physical display verification remain unfinished. The logs
prove preparation on this MCU, not an overnight appliance or recorder.
