# Authenticated, non-transitive federation profile

Status: loopback network and deterministic adversarial proof for #3076.
Entrance: `cargo xtask check federation-security`.

The first proved profile carries signed operation frames over loopback TCP. Its
session challenge is signed by both peers and binds exact initiator and
responder Host/Boot/offer generation, Line and session identity, descriptive
Line security, fresh nonce, expiry, and finite frame count. The proof uses a
`plaintext-network` Line deliberately: signed peer attribution and effect
authorization remain distinct from transport confidentiality. It makes no
public-Internet or encrypted-transport claim.

The receiver validates four independent layers in order:

1. exact mutually signed current session and peer identity;
2. exact current admitted Body membership observation;
3. monotonic finite frame sequence and current credential state;
4. the receiver-local opaque #3072 handle against exact Base/Plan/Play/
   operation/subject/resource generation/envelope scope.

The sender receives no Base-registry access and cannot select an arbitrary
operation. All descriptive IDs and wire claims are copyable, but capability
possession stays in the receiver's private table. A successful signature does
not create membership or authority. Revoking the session credential or local
capability immediately refuses later frames. Host/Boot/offer, session, Line,
receiver, signature, operation, subject, and resource mismatches remain
distinct refusals and do not consume the expected sequence.

The A-B-C fixture gives B separate authenticated relationships with A and C.
It proves an A-signed frame cannot redirect into the B-C session and B cannot
substitute a resource outside C's locally issued capability. There is no proxy,
delegation, remote shell, automatic peer discovery, or failover-to-anyone
operation. A replacement peer requires a fresh mutually signed challenge and
fresh receiver-local capability.

Existing `CandidateInventory` remains the bounded staged-disclosure surface:
discovery proof is not a membership credential, candidate observation is not
admission, and full current offers are validated within existing finite byte
and item bounds. Existing Body admission remains the sole membership-changing
path and requires its own administrative proof.

Patchbay-safe `FederationInspection` reports session, Line, descriptive
transport class, exact authenticated peer Host/Boot/offer generation, current
membership state, credential status, and finite sequence position. It contains
no private key, signature, nonce, or Base bearer material.

This profile proves signed integrity/attribution and receiving authority on
local loopback TCP. Deployments over hostile networks still require a reviewed
confidential authenticated Line (for example an explicitly configured TLS or
Noise realization) carrying the same session frames; TLS alone never replaces
the receiver checks above.
