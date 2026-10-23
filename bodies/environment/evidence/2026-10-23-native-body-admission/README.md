# Native ambient admission responder

The actual ESP32 Root composition now creates a Boot-local Ed25519 responder
from initialized hardware TRNG and serves existing borrowed Body challenge/proof
documents over bounded UART. There is no secret/key fallback, membership flag,
Body install, capability issue or start command in this exchange.

Eight CPU Root controls pass, including the production signer through ordinary
AdmissionManager completion. Wrong Host/Boot/generation/protocol/nonce/expiry
shape refuse without allocation. A valid signature also allocates nothing;
tampering preserves membership, and replay refuses after successful admission.
The deterministic key and Boot in CPU controls are explicit fixtures.
Strict Root-control Clippy and optimized ESP32 compilation pass. Four unused
issuance-method warnings remain. No new image was flashed in this slice.

A read-only observation of the previous running image measured a 6,352-byte
native advertisement, requiring an independently bounded 8 KiB UART offer;
the Pico USB carrier's 4 KiB limit remains unchanged. Challenge input is 1 KiB,
with a five-second receive deadline and two-second replies. Physical signature,
authenticated membership, owner-to-Root installation and execution remain unproved.
