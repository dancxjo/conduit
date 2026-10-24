# Native station HTTP custody and attended AP regression

The selected ESP32 switch now awaits actual station HTTP listener readiness after
association and DHCP. The service owner retains distinct AP/station stacks and
reuses its two response buffers. Station mode serves no DHCP. Its acknowledgement
is distinct from AP readiness and remains inside the original attempt deadline;
cancellation/failure still requires bounded AP restoration. This changes no Body
membership, network authority, retry policy or native observation meaning.

`cargo xtask prove environment owner-face-http-run-esp32` now owns the bounded
HTTP observer and one generated nonexistent-network fixture. The observer retires
on success or failure and verifies actual native Face identities/revisions, absent
acquisition coordinates, freshness, same-Boot control and deferred transition.
It never owns UART or prints/retains the generated password. The plain
`owner-face-run-esp32` remains a separate sampler/Face lifecycle proof.

The attended named proof captured three physical DHT observations and correlated
all three native Face revisions across 59 HTTP 200 responses. Before publication,
HTTP returned 503. The setup page returned 200 and one POST returned 202. During
all three captures, setup remained `awaiting-native-retirement`. After attributed
quiescence, Body Lull and provider retirement, one credential call and one join
call ran. Association was rejected, no attachment receipt was issued, and AP
services were restored in the same Boot. No real network credentials were used.

The keyed image retains 36,924 linked stack bytes: 64 fewer than the preceding
image, and 60 above the unchanged 36 KiB floor. Root heap capacity remains
118,768 bytes; sampling retained/peak usage remains 79,692 bytes, retirement
releases it to 13,988, and post-network use is 19,368. The observed SDK peak is
40,040 of 73,728 reserved bytes. These are unsigned diagnostics, not stack
high-water or complete concurrent admission. The sampling/network workspace guard
remains. Successful station HTTP, mDNS, concurrent transition and continuing
native display remain unproved.

HTTP tests pass 34 cases, including distinct station readiness and cancellation
across restoration; strict HTTP lint passes. CPU replay accepts the retained
physical HTTP capture and rejects six corrupted identity/time/freshness/Boot/order
cases. The prior Source-only run and pre-flash dispatcher refusal are retained
separately rather than counted as HTTP proof.

Full logs, ELF/map and captures are retained outside Cargo output under
`outputs/environment-native-face-probe/1792875914`. The protected operational image
was restored and verified with three fresh source sequences and twelve acknowledged
physical display writes.
