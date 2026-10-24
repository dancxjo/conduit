# Native sensing with queued network setup: physical evidence

The supported `cargo xtask prove environment owner-run-esp32` run completed on
the clean image in `owner-root-image.json`. During the live run, the retained
separate internal HTTP fixture submitted only generated credentials for an
absent network. `/network` returned 202. No real network password was used.

All three signed, one-minute DHT observations arrived with Root reporting
`setup_status=awaiting-native-retirement`, reservation offer generation 1 and
next Play sequence 2. The private request stayed queued: no network-control Play
had yet been reserved. The ordinary retained owner projected three Body Faces,
verified quiescence, recorded ordinary Lull and verified signed retirement.

Only after retirement released construction storage did Root admit the queued
request's first attempt. The same Boot reached `access-point-restored`, exactly
one credential read and one join (`[1,1,0]`), association rejection and no station
success receipt. Final offer generation was 3. The restored `/network` page was
reachable with HTTP 200. Final reported Root heap use was 15,444 bytes, with a
105,592-byte Boot peak inside the 114,688-byte reservation. No guard was relaxed.

This verifies workspace ordering and preservation of acknowledged input through
actual sensing and retirement. It does not claim simultaneous join and sensing,
a physical offer change before cleanup, successful station joining, repeatable
configuration, a second native Play, browser Body membership or overnight
recording. Retained-owner authentication across changed current offers is
separately covered by the production CPU verifier/reservation checks. The MCU
is retired and its AP restored after this finite proof.

Each `.metadata.json` preserves the exact reply fields except the duplicate
`publication` byte array, whose exact bytes are retained in the named `.bin`.
The original reply file SHA-256 is recorded; Boot-signed receipt fields are
unchanged. Canonical payload lengths/digests, sequences and Body Face correlation
were checked. Face JSON is compacted without changing semantic content. Full
original captures remain in the local proof output. Related issues: #6340 and
#6341. The earlier startup and live-memory failures remain separate evidence.
