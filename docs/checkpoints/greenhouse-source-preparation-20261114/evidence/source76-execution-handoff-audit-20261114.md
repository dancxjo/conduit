# Source76 execution handoff: verified blockers and next implementation

Read-only source inspection at shared parent25145af968. No build, device change, admission or Start. This supplements the successful CPU preparation receipts; it does not expand their claims.

## Why the entry never becomes Playing

`targets/rp2040/firmware/host/src/neutral_service.rs::serve` accepts original Root inspection, spawn, return and preparation. Its preparation branch invokes `preflight`, then records `prepared-dropped-structured-fore-mask-unavailable` on success. Once a command is retained, other commands receive `ROOT_REFUSAL@1 structured-fore-mask-unavailable not-playing`. `InstallContinuation` refuses. Inspection reports `phase=not-started fore=unsupported mask=unsupported`.

`neutral_preflight.rs` explicitly drops the prepared workset. The actual admission and Start transitions already exist in shared `execution_workset.rs::PreparedLocalWorkset::admit` and `execution_workset_start.rs::CommittedLocalWorkset::start_into`; the neutral entry does not call them. Simply removing the refusal string would not install the missing owners or confer authority.

`neutral_root.rs::run` independently charges original baseline allocation, then leases only the remaining configured heap for preparation. The CPU fixture's larger arena is not this budget. Preserve this accounting when implementing execution; CPU success is not target fit.

## What the current authored component actually exposes

The selected `shared-environment` Plot in `targets/mcu/source-build/tests/fixtures/environmental-exchange-selected.conduit` has:

- Inputs: `begin: BmeProtocolBegin`, `configuration: Ht16k33Configuration`, `frame: Ht16k33Frame`.
- Outputs: `captured`, `observation`, `sensor_refused`, `clock_failed`, `initialized`, `initialization_refused`, `written`, `write_refused`, `unrouted`.

The generated layout independently declares12 Fore ports: three inputs and nine outputs. This is a checked protocol component, not a closed autonomous application.

`plots/device-protocols/environmental-exchange.conduit` already arbitrates sensor, initialization and display-write requests through ONE correlated bus. It explicitly requires initialization acknowledgement before a frame is supplied. Do not create another overlapping physical I2C owner to display a frame; preserve this existing custody and acknowledgement ordering.

## Existing reusable pieces and limits

- `fore_bridge/structured.rs::StructuredIngressSpec` already validates exact linked structured ingress; it is not necessary to invent a second structured-value codec merely because this component is structured.
- `root_fore_carrier/actor.rs::PreparedForeSessions` currently contains exactly TWO routes and `prepare_bridge_sessions` takes one ingress plus one egress. This is production code, not just the one-node numeric test fixture.
- `fore_bridge.rs` explicitly describes a payload-only mailbox. Normal/abnormal terminal control is not implemented; structured ingress and egress reject abnormal ABIs. Increasing the route-array length alone would not complete finite-stream lifecycle support.
- These carrier pieces hold actual selected Lines and framed transport custody. They must not be used to invent network Lines for ordinary local equipment/Fore traffic.
- `execution_mask.rs` and `execution_mask_service.rs` already contain actual prepared renderer/Mask custody, with HT16K33 encoder tests. Branch-local existence is not integration of this protocol component with that service.

## Concrete implementation boundary

1. Finish the application's authored composition around this component: initialization, sample cadence, observation/failure handling, and display-frame flow. Keep physical configuration in checked Host/Base facts. Do not replace the missing Plots with a board-specific measurement/redraw loop.
2. Bind the exact local Fore/Mask owners and outputs with finite buffers and closure handling; preserve initialization-before-frame and physical write acknowledgement. Prefer closing application-internal ports in authored Source over extending a network carrier solely to carry local hardware traffic.
3. Connect typed BME observations to the standard publication/Face/history path with correct capture provenance. Do not label them as DHT readings or mistake delivery for durable recording commitment.
4. Prepare all required owners, attach actual services, call the existing whole-workset resource admission, and then the original authenticated Start exactly once. Refusal must leave no partial Playing state or consumed equipment owner.
5. Qualify the complete reachable application on target, including heap/startup overlap/stack and simultaneous Mask/observation/history custody. Then test owner disconnect, physical power cycles and buffered recovery before replacing working firmware. Check the installed finite operation/deadline/history ceilings against the intended unattended duration; do not silently enlarge grants.

The twelve-port component and preparation refusal are facts from current source. This document does not claim that target RAM fit, Source adoption across new Boots, local execution, durable commitment, membership recovery or the multi-host display are delivered. Existing6341/6457/6374/6378 and the continuing Body/history obligations remain the owners; no new memory epic or parallel transport is proposed.
