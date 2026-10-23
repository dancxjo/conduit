# Bounded Source-reference assignment transfer

The supported native-construction proof and physical software-ready capture now
encode and decode a compact transfer before ordinary canonical validation and
fixed-table compilation. The actual captured appliance assignment occupies 7,111
wire bytes instead of 46,411 canonical bytes, under an explicit 8 KiB wire ceiling.
Seven independently compiled Source computation records supply 39,552 bytes of
canonical payload without copying it into the wire buffer.

The versioned representation has magic CDAPREF1 (8 bytes), complete wire length
(u16 LE), record count (u16 LE), the full 32-byte resident Source digest, then the
unchanged 125-byte canonical assignment header. Each record is either inline
(tag 0, canonical tag u8, payload length u16 LE, payload) or a resident reference
(tag 1, slot u8). At most 256 total records and 32 resident slots are admitted.
All resident slots must appear exactly once. Unknown tags, wrong Source, missing,
duplicate or out-of-range references, overflow, truncation and extra bytes refuse.
The decoder writes caller-provided record slots and borrows wire/ROM payloads.
The encoder and decoder do not allocate; partial output on refusal is discarded.

Framing is deliberately distinct from admission. Decoded header and records run
through the same Core canonical digest, schema, scope and inventory validator,
then Runtime's independently pinned layout/roles/cadence checks. A corrupted
canonical digest inside an otherwise well-framed transfer is explicitly refused.
This creates no capability, native lease, reservation, Body or Play.

All three dedicated framing controls and the existing 25 Make construction
controls pass, along with seven Core controls, five legacy assignment goldens,
and compilation of both generated resident modules. The actual appliance CPU
round trip proves zero allocation for transfer encoding/decoding and admission,
including digest/layout refusals. Strict focused Clippy and the ESP32 optimized
release build pass (same five unused material/issuance warnings).

The retained physical capture observes the previously flashed Root policy image,
then performs transfer and table compilation on the planning host. No packet has
been delivered to the MCU, and the rebuilt ELF was not flashed. Independent
firmware layout provisioning, device ingress, native binding, full memory/work
admission, Body/Play installation and sensor/Face/Mask execution remain required.
Positive station/mDNS, recorder custody/transport and a physical overnight run
also remain unfinished. No dev or stable acceptance is claimed here.
