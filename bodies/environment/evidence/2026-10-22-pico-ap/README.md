# Pico W AP/HTTP physical bench evidence

These receipts capture the task worktree before its rebase onto current `dev`.
The binary is identified by the retained ELF/UF2 hashes, rather than representing
this as proof of a later merge candidate or an accepted release. Source and
portable integration checks are rerun separately after rebasing.

The Pico W owns radio PIO0/SM0/DMA0/1 and DHT11 GP2 on PIO1/SM0/DMA2. USB shows
fresh Plan binding, preparation within a 196 KiB arena, allocation sealing, and
two observations with the AP ready. The host then associated with the open
`Conduit-Climate` AP, received 192.168.4.2 through DHCP, and issued eight
consecutive page/JSON requests. All returned HTTP 200; JSON reported fresh
66.2°F and 52% relative humidity. Ethernet retained the host's internet route.

These are short physical sensor and network observations. They do not prove
17-hour endurance, overnight history, production Host/Body installation,
browser recorder membership, AP/station handoff, or successful TM1637 wiring.
Firmware binaries and detailed build logs remain in the task's retained outputs;
regenerate through `cargo xtask prove dht11 build-http` for the current source.
