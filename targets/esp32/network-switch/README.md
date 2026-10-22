# ESP32 AP/station execution machinery

This SDK adapter implements the existing portable `StationOperations` and
`AccessPointSwitchOperations` traits. Root supplies the actual retained radio
controller, **distinct** AP and station stacks/runners, prepared SDK configs,
AP address and service owner. Constructing it does not mint a Host offer,
authority, attachment or Body/Plan/Play.

With the pinned ESP32 SDK activated:

```sh
cargo xtask prove environment network-esp32
```

This is a cross-target compile proof against the real SDK and portable execution
traits. It is not a physical network transition or an installed Host Call. The
currently deployed climate bench continues to serve its AP unchanged.

Withdrawal awaits actual AP-service retirement, clears old AP and station IPv4
state, selects station mode and enables DHCP. Association invokes one SDK connect
operation using the supplied finite retry configuration. DHCP readiness requires
both a current station link and station address. The AP's old static address can
never be returned as a station success. Root chooses and admits all configuration
and deadlines; this adapter chooses no SSID, credential, retry or attachment.

`Esp32Deadline` uses one absolute instant across the complete attempt. An expired
instant refuses before polling work; a late completion cannot become success.
A separate Root-admitted instant bounds restoration. These cooperative deadlines
do not preempt synchronous SDK code: its mandatory work and memory must also be
admitted. SDK buffers/credentials remain SDK-owned; no new blanket heap or secret
erasure guarantee is made by this adapter.

The SDK connect future only drops its event subscriber on cancellation. The
adapter additionally attempts a synchronous return to AP radio mode when that
future is cancelled or fails. `cleanup_required()` stays true through station
operation and cancellation until complete AP restoration succeeds. Radio fallback
alone is never reported as AP readiness. Root must revoke the old attachment,
clear station addressing, restore AP addressing, observe AP link-up and await
actual DHCP/HTTP service readiness. A failed cleanup remains unavailable; there
is no implicit retry, new authority or fabricated success.

The attended AP bench now supplies a concrete `AccessPointServices` owner backed
by its actual socket task. Withdrawal awaits socket/request retirement; restoration
awaits DHCP and both HTTP listeners. The unique control handle and finite command
generations survive cancellation; obsolete acknowledgements cannot satisfy a new
request. This bridge is compiled into the AP bench, whose startup uses restoration.
Physical station withdrawal/restoration is still unproved.

A real installation still needs the private page mailbox connected through admitted `network/credentials` and `network/join`
Host Calls, current Body lifecycle, actual station-service ownership, and physical
success/failure/link-loss proof. No placeholder service implementation is shipped.
