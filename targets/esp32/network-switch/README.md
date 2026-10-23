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

`Esp32NetworkJoinProvider` now connects the actual controller/stack/service
adapter to the portable selected Host Call owner. Root prepares the SDK station
configuration before Play, including its heap-backed password String. The runtime
credential request must match that private configuration before radio effects;
the wrapper neither creates a new String nor logs credentials during execution.
It returns the reserved attachment record only after association and a current
station address succeed. SDK internal memory/work still require separate finite
Root admission; cross-compilation does not establish that budget or physical
network success.

Root prepares a nonzero attempt duration before Play. Its absolute deadline is
established at the first authorized invocation, so planning or delivery latency
cannot consume the radio budget before execution. The attempt is marked spent
before any radio work; cancellation cannot restart it. The entire withdrawal,
association and DHCP attempt shares that one absolute deadline.
A guard checks time before each work poll and after it, rejecting work started
expired and synchronous completion after expiry. The timer still supplies expiry
wakes, and synchronous native work remains non-preemptible. Deterministic guard
regressions run through `cargo xtask prove environment network`.

Cleanup has a separately admitted duration. Its absolute deadline is established
on first cleanup, which can follow a long-lived successful station attachment;
cancellation or another cleanup call cannot reset it. All AP addressing, link and
service acknowledgements must complete within it. No retry or new credential
selection is hidden in the provider. Actual offer/capability issuance, the private
page-to-control-Play handoff, station HTTP/mDNS ownership and physical
join/fallback remain open.

`prepare_station_configuration` is the Root preparation callback for the
attended WPA2-personal profile: a UTF-8 SSID of 1–32 bytes and a UTF-8 passphrase
of 8–63 bytes, with embedded NUL refused. It creates the SDK password String
before Play and explicitly disables the SDK's default connection retry. The
selected provider refuses configurations outside that profile or with retries
enabled, including binary SSIDs whose SDK string view loses bytes. This is SDK
representation preparation, not submission consumption or permission to join.
The Root preparation image calls this after an acknowledged submission when
the operator requests its offer. The selected control Play remains uninstalled;
configuration preparation makes no station-ready claim.

The adapter owns its initialized controller, private SDK configurations and
service-control handle. Its interface handles still refer to Root-retained
network resources. This permits the selected provider to live in Root storage
through a long-lived attachment without borrowing sibling Root fields or adding
pinning, cloning or heap allocation for the ownership transfer.

`Esp32NetworkJoinProvider::prepare` borrows an optional owner slot. Configuration,
duration and attachment validation finish before it takes the owner. A refusal
leaves the slot intact, preserving AP ownership; success empties the slot and
returns the sole provider owner. An empty slot receives `OwnerUnavailable`.
The constructor's actual-resource and finite SDK admission requirements remain
unchanged. Root still needs to admit those resources and install the selected
control Play; compilation and strict linting establish no physical join proof.
