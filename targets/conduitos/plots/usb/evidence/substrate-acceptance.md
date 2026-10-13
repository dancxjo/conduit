# Product USB substrate acceptance ledger (#5253)

This ledger joins the reviewed contracts, deterministic mechanism tests and
ordinary product guest receipts. It does not certify integrated `dev`, physical
hardware, class acceptance, or synchronized execution of every artifact below.
Each receipt retains its own collected Source/package and kernel identity.

| Requirement | Evidence and scope |
| --- | --- |
| Normal product selection and native dispatch | The CDC sustained guest uses a `product-host` kernel, independently approved Root, five plotted setup controls and installed bulk offers. [Receipt](native-cdc-sustained-bfa318cf/usb-cdc-current-sustained-zero-allocation.json). Product bootstrap and subordinate Host Call paths implement the same ordinary offers; discovery grants no authority. |
| Checked control and endpoint-read Plots | [control.conduit](../control.conduit) and [endpoint-read.conduit](../endpoint-read.conduit) retain the shared typed Fores. `control_factory/tests/execution.rs` exercises checked control through the production Kernel; `endpoint_read_factory/tests/execution.rs` exercises packed endpoint results through it. The CDC guest separately executes the installed native offers. These are distinct evidence scopes, not one generic-Plot guest transcript. |
| Cancellation cannot publish late success | `control_owner/tests.rs` and `endpoint_read_factory/tests/execution.rs` fence revoked calls. `usb_native_host/paired_lease_tests.rs` submits original endpoint leases, acknowledges the original stop, then rejects old TRB events and both original Kernel tokens. A device response after Stop in a usbredir transcript is not relabeled as native late completion delivery. |
| DMA retained until acknowledged quiescence | `usb_native_host/stop_ack_tests.rs` checks original slot/pointer identity and failed-stop quarantine. Paired lease tests retain both original reservations and Kernel obligations after failed stop. [EP0 regression](ep0-timeout-retirement-3149cab5/validation.json) adds a genuinely submitted control timeout: failed stop retains uncertain DMA; exact acknowledged stop retires the cursor and lease. |
| Wrong Boot/Plan/Play/provider/attachment and forged possession refuse | `control_owner/tests.rs` mutates 13 operation-scope fields before submission. `endpoint_read_owner/tests/possession.rs` refuses foreign handles and issuers despite copied descriptive identities. `usb_control_installation_tests.rs` and DMA/continuity tests preserve provider/resource generation and original attachment ownership. |
| Sustained finite reuse without Play growth | Actual CDC guest: 16 TX/16 RX, exact ordered octets, zero allocator/reallocator entries and acknowledged Stop. Actual [Audio cross-architecture receipt](native-audio-public-cross-architecture/usb-audio-public-cross-architecture.json): 72 exact packets, eight reused DMA slots and zero Play allocator entries on both targets. [BOT proof](native-bot-zero-allocation-d2ab8e8a/validation.json): 16 actual tagged reads and exact blocks, zero Play allocations. Preparation is a separate lifetime stage. |
| Exact evidence identity and terminal outcome | Native serial transcripts retain Source/Root/BASE/RESOURCE/Plan/Play and matching allocation/Stop identities; independent wire checks correlate typed ordinals/extents with actual device packets. Deterministic Kernel fixtures additionally check original admitted request/token identities. These complementary checks do not claim a single captured cross-tier trace. |
| Shared API, class policy in Source | Control/bulk/ISO owners implement transport and possession. CDC, ECM, BOT and UAC checked Plots author descriptor truth, setup, wire framing, timing and class outcomes. External device fixtures implement device behavior, never Conduit providers. [Offer inventory and target limits](../README.md). |

## Failure coverage

- Full/pressure: pending control possession, endpoint ownership and eight-slot ISO
  pressure tests; no second submission or mutation of retained DMA.
- Short/overrun: exact native completion/residual and endpoint read envelope tests;
  checked CDC/ECM metadata and byte-length guards retain admitted partial truth.
- STALL/disconnect: paired genuine lease tests plus actual class guest transcripts.
  Current corrected-package ECM [STALL](native-ecm-envelope-d2ab8e8a/stalled/validation.json)
  and [disconnect](native-ecm-envelope-d2ab8e8a/disconnect/validation.json) preserve
  distinct outcomes, no subsequent effects, zero allocations and acknowledged
  Stop. Incomplete network service remains refused.
- Pending/no event: held CDC and BOT transfers exercise finite Root-budget
  cancellation. These are not mislabeled EP0 timeout evidence.
- EP0 timeout: [red/green regression and stop suites](ep0-timeout-retirement-3149cab5/validation.json)
  submit a selected native control TD into an empty event ring, reach bounded
  `ControlTimeout`, retain uncertainty through a failed original stop, and retire
  only on the next exact acknowledged stop. Old success cannot reopen the call.
- Wrong generation/late event: original attachment, DMA and Kernel token tests
  refuse stale replacements and late publication.

## Admission still required

The earlier retained run passes 98 native and 94 offer/identity tests. After the
EP0 correction, 99 native tests, eight original-stop tests and five ring tests
pass. Current-head Candidate and exact merged-tree Integration remain required;
this ledger does not close #5253 by itself.

Full-slot Stop permanently seals its current Host and revokes possession.
Endpoint-only quiescence/rearm and interrupted BOT reset reentry are not
implemented by that terminal path. Isochronous IN and unsupported target/board
providers remain explicit unsupported dispositions. Physical/HIL evidence and
multi-architecture class acceptance are separate scopes, not extra substrate
closure gates invented by this ledger.

## Standalone entry audit

The exact standalone control entry now executes in the product guest: see
[retained proof](standalone-control-21e596e3/README.md). Its typed18-byte descriptor
matches the external fixture, with zero Play allocations and retirement.

The exact standalone endpoint-read entry instead refuses preparation with
`Possession(WrongSelection)`: [retained failure](standalone-endpoint-read-refusal-21e596e3/README.md).
The current combined admission path unconditionally issues control possession,
but this checked Plan has no control placement. The combined guest demonstration
does not discharge this missing standalone acceptance item.

The endpoint-only admission correction73f3abc97 binds only the exact independently
selected endpoint scope and refuses control invocation when control is absent.
[Two production-path regressions and31 native lifecycle tests](endpoint-only-admission-73f3abc9/validation.json)
pass. This removes the demonstrated preparation defect; a corrected guest rerun
is still required before claiming the standalone endpoint acceptance item.
