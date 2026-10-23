# Native AP/client failure and fallback: physical evidence

The existing supported `cargo xtask prove environment network-fallback` entrance
passed on the same Boot and image as the adjacent owner-retirement evidence.
The MCU remained powered after sampling, Lull and worker retirement. The Linux
host joined the MCU AP over Wi-Fi while retaining Ethernet connectivity.

A private HTTP POST to `/network` returned 202. It carried only generated fixture
credentials for an absent network; no real network password was used or retained.
The proof then made no UART requests for thirty seconds. The ordinary installed
Root setup kernel dispatched one credential read and one join, association was
rejected, and native socket-task acknowledgement established AP restoration.
Two subsequent observations retained the same Boot, seven kernel steps and
exact dispatch counts `[1, 1, 0]`; no station-success receipt was fabricated.
The complete bounded check finished in approximately 34 seconds.

A separate post-run HTTP readback of `/network` returned 200 and the setup form;
its bytes and digest are retained alongside the original proof output. The
appliance root route `/` returned 503: this Root preparation service does not
yet serve an admitted appliance Face/Mask. The readback is explicitly separate
from the original proof script's acceptance logic. Host offer generation
advanced from 1 to 3 in the same Boot. Final Root heap usage was 15,436 bytes;
the earlier native run's 105,164-byte peak remained the Boot peak.

This proves the native negative join/fallback path and reachable restored setup
page after worker retirement. It does not prove successful station joining,
mDNS on a joined network, link-loss fallback, a second setup attempt, concurrent
resident sensing, browser Body membership, or recording. Setup remains one-shot
in this image, and the climate worker remains retired. No new firmware was
flashed for this network check. Related implementation gaps remain open.
