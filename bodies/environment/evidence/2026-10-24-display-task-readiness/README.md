# Linked display-task image: physical readiness and recovery

Ran `cargo xtask prove environment display-output esp32-readiness --flash` on
ESP32 `b0:a7:32:28:61:78` from the clean source pinned in `image.json`. The image
includes the newly registered-task construction path and links with the existing
28 KiB CPU stack floor. The readiness inspection itself installs no Plan or Body,
issues no native possession and performs no display transaction.

The board reported three retained portable output software offers. Native
availability remained unchanged across inspection. Root steady usage was 64,024
bytes and its peak was 96,984 bytes within the unchanged 114,688-byte region.
The separate SDK region retained its own accounting. Host planning against those
observed offers produced the exact four-node, five-cord output graph with two
local Fore ports and three Host Calls; it remains uninstalled.

A scripted final step restored the integrity-checked working image. An eighteen
second UART capture then recorded five live display updates, advancing source
sequences 3 and 4. Every captured observation was current/fresh and every display
write acknowledged; three distinct register frames were observed. This is driver
acknowledgement, separate from human visual confirmation or an HTTP check.

The small committed receipts summarize the retained raw capture and image in
ignored `target/environment-display-readiness/1792827656` and recovery logs in
`target/environment-display-task-probe/1792827652`. Hashes pin those artifacts;
ambient readings and the large offer inventory are not copied into Git.

This proves physical bootstrap/readiness with the new image footprint and
successful recovery of the live bench. It does not prove native kernel memory
admission, task registration, signed lifecycle commands, Frame delivery, a live
Mask client, concurrent sensing/network transition, recording or overnight
endurance. The working image was restored; no readiness-only image was left as
the appliance.
