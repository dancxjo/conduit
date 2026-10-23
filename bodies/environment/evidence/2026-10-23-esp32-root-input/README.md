# ESP32 Root private-input physical evidence

The replacement ESP32-WROOM revision 3.1/4 MB ran the Root preparation image
identified in [receipt.json](receipt.json). The host associated with its
`Conduit-Climate` AP and received 192.168.4.2. `/network` returned HTTP 200 with
preparation-only wording; `/reading` returned 503 because no observation Face
was installed. A synthetic private credential submission returned 202 without
echoing the input or claiming a join had started.

Three subsequent UART offer requests reported `network_configuration: prepared`
under the same observed Host/Boot. Body and native Plan installation remained
false. This executes the actual mailbox-to-SDK preparation path, retaining the
configuration across offers. The selected credential Host Call has not run.

Opening USB serial resets this board. An initial observation after submitting
therefore cleared its volatile input; that trace is retained separately. The
accepted run held serial open throughout submission and all three offers.

The existing temperature/RH image was restored afterward. Six serial display
events acknowledged successfully, with current observations, AP ready, 69.8°F
and 52% RH in the last sample. This retires the diagnostic Boot; its offers
cannot be used to bind a current Plan.

This proves short physical preparation and restoration, not selected network
execution, station joining, fallback, mDNS, installed Host/Body construction,
browser recording or overnight endurance. The file hash identifies the retained
ELF; runtime image attestation remains null. Full firmware and transport logs
remain in the task's `environment-root-preparation-service` outputs, with hashes
in the receipt. Build through `cargo xtask prove environment root-esp32`.
