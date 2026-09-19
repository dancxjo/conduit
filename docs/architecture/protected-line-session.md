# Protected Line session profile

Status: portable contract and deterministic conformance vectors for #3650. Target adapters and relay integration remain separate work.

The first protected-session profile is
`conduit.line/noise-nnpsk0-25519-chachapoly-sha256@1`, realized by the
established Noise revision-34 `NNpsk0` pattern with X25519,
ChaCha20-Poly1305, and SHA-256. Conduit supplies a fresh 32-byte ephemeral
private key at each endpoint and one exact 32-byte rendezvous secret as the
PSK. The implementation library owns the Noise state machine, key schedule,
nonces, authentication tags, and key zeroization; Conduit does not redefine
those operations.

Before the handshake, both endpoints encode the same bounded prologue. It
binds, in order:

- implementation/protocol revision;
- initiator Host and Boot;
- responder Host and Boot;
- rendezvous/negotiation identity;
- planned Line-session identity;
- relay candidate/routing binding;
- outer-transport binding;
- maximum payload bytes, frames per direction, and plaintext bytes per direction.

The initiator and responder roles are fixed by that binding. A stale or
substituted identity changes the handshake transcript and cannot authenticate.
The PSK is secret key material, not routing metadata. It never enters retained
evidence.

After the two-message handshake, each direction owns a separate Noise cipher
state. The small Conduit framing header contains only the format revision,
direction, exact sequence, and ciphertext length. That header is authenticated
as associated data. The relay may use a separate finite routing envelope, but
does not receive endpoint identities, plaintext, keys, or authority from this
frame format.

The session admits strict in-order delivery. Duplicate sequence is replay;
future sequence is reordering; wrong direction is a role error; modified
ciphertext or authenticated header is an authentication failure. These facts
are not collapsed into outer-carrier loss. Authentication, replay, reorder,
malformed-frame, wrong-direction, and counter/byte exhaustion are terminal and
retire both cipher directions. Explicit close and cancellation are distinct
terminal dispositions. Caller buffer shortage and an oversized local send
request refuse before consuming the session.

All retained state is finite. The binding is at most 2,048 encoded bytes;
payloads are at most 65,519 bytes; configured frame and plaintext-byte ceilings
are validated before handshake; one caller-owned input and output buffer can be
reused for the session lifetime. Evidence retains only the bound identities,
profile revision, role, limits, finite counters, and terminal disposition.

`architecture/protected-line/vectors/noise-nnpsk0-v1.json` is the immutable
cross-implementation vector. The Rust realization also proves 100,000 ordered
frames through fixed buffers. The crate's contract-only build disables crypto
and compiles for `x86_64-unknown-none`; a ConduitOS Host must use that boundary
to refuse this profile until an exact entropy source and target realization are
admitted. Browser and hosted adapters must consume the same vector and profile;
compilation alone is not interoperability evidence.
