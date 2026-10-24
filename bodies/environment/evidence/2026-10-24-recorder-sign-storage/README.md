# Bounded one-shot publication evidence storage

The Linux publication owner now uses the ordinary hosted sign store with the
existing 128 ordinary records and six remote lifecycle records. Preparation
additionally requires exactly one route with one target. The admitted owner
accepts one sealed input and delivers at most one reference: ingress admission
and close (two), egress offer, acceptance and delivery (three), and output close
(one). Failure is a prefix of this exchange; scheduler cancellation adds only
ordinary records. Retained mandatory evidence is never reset or evicted.

Hosted backing allocates only the six remote entries, rather than the fixed
store's 128. CPU64 sink retention falls from 976,105 to 932,905 bytes for fifteen
owners (43,200 bytes saved); preparation peak falls from 2,518,125 to 2,477,805.
Two-owner retention is 130,298 bytes and peak 1,675,198. Inline sink, reference
Vec and worker future remain 616, 24 and 3,648 bytes. The measurement scope and
probe limitations are the same as the preceding recorder-backing capture.

The focused joint Root test passes both assignment representations for 960
readings, two-reading cancellation and occupied-generation failure, retaining
the original durable prefix and catalog/Body State fresh-Boot recovery. Two
actual file publication tests cover success, collision, cancellation after file
completion and incomplete preflight. Sample/write execution has zero Rust heap
allocations. Strict Linux library lint, formatting and the memory ledger pass.

No complete Host resource admission, authenticated Line, continuing rotation,
combined MCU admission or hardware deployment is claimed. Earlier immutable
evidence remains available in the preceding directories.
