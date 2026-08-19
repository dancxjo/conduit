# Netherwick Brainstem migration ledger

Issue [#1521](https://github.com/dancxjo/conduit/issues/1521) owns this ledger. The inventory is pinned to Netherwick commit
`f43ff13846b47b05e133d0321bdbaafffd1bcdbe`, the revision consumed by
`conduit-netherwick`. It is historical input, not a second source of current
Conduit truth.

The migration rule is:

```text
portable meaning -> selected realization -> device protocol -> physical Base
```

Create Open Interface (OI) over UART is a device protocol over a UART Base. It
is not a Conduit LINE. A LINE connects Conduit Hosts or Parts.

The Pico W attachment additionally requires an admitted, enabled logic-level
translation resource between the Create's 5 V transmit signal and the Pico's
3.3 V receive GPIO. A 5 V signal must never be connected directly to Pico W or
ESP32 GPIO. Whether 3.3 V is sufficient for the Create receive input is an exact
attachment fact to prove, not an ambient assumption. Portable Forms contain
none of these electrical facts.

## Disposition vocabulary

- **keep**: portable meaning or a bounded realization responsibility remains.
- **compose**: the behavior is an ordinary Form over smaller portable kinds.
- **service**: retain only as a bounded, explicitly authorized maintenance
  action outside the ordinary Gear palette.
- **replace**: ordinary Conduit machinery owns the responsibility.
- **delete**: remove the Brainstem-specific API/runtime surface once its named
  replacement evidence exists.
- **fixture**: retain exact historical bytes or behavior only for conformance.

No row authorizes physical use. Deterministic conformance, std physical proof,
and Pico W HIL remain distinct evidence classes.

## Public command and RPC surface

| Brainstem command | Disposition | Highest honest Conduit seam | Replacement evidence required before retirement |
|---|---|---|---|
| `ping` | delete | Host/Boot observation and LINE liveness | Host fencing distinguishes live Host, lost LINE, and lost device |
| `status` | delete | Presentation and Observatory projection | Current Plan/Play/resource/safety truth is inspectable without private state |
| `get_capabilities` | delete | `HostAdvertisement` and exact offers | Both Hosts advertise exact current resources and implementations |
| `get_events` | delete | bounded Signs and provenance | Required lifecycle, refusal, safety, and terminal Signs are retained |
| `arm`, `disarm` | replace | authority/session grant and revocation | Motion requires an exact grant and revocation reaches safe disposition |
| `stop` | keep | cancellation plus mandatory actuator safe disposition | Stop is bounded, observable, and cannot be rejected by ordinary backpressure |
| `estop`, `clear_estop` | keep/service | mandatory local safety; authorized conditional clear | E-stop is local and non-bypassable; clear proves the same current safe condition |
| `clear_safety_latch` | service | hazard-generation-scoped authorized safety action | Wrong/stale hazard generation is refused distinctly |
| `careful_mode` | service | narrow attended authority/policy profile | Finite TTL and the invariants that remain absolute are enumerated and tested |
| `escape_motion` | keep | hazard-generation-scoped motion admission | Exact hazard, direction, bounds, TTL, and terminal safe disposition are proven |
| `heartbeat_stop` | replace | authority/control-liveness fence | Control loss independently stops motion without depending on LINE recovery |
| `clear_motion_queue` | replace | cancellation of admitted finite work | Cancellation identifies affected work and preserves terminal Signs |
| `cmd_vel` | keep | body-forward linear/angular velocity intent | One portable velocity contract lowers through each selected drive realization |
| `drive_direct`, `drive_arc` | fixture/compose | Create-native lowering or reusable adapter Form | Neither wire shape becomes a separate architecture-defining Gear kind |
| `request_sensors`, `stream_sensors` | delete | typed Current/Flow outputs with finite pressure | Sampling, freshness, capacity, closure, and pressure are explicit |
| `song_define`, `song_play` | keep | standard music/sound semantics | Existing portable `music/play` Form lowers through bounded Create song slots |
| `define_chirp`, `play_feedback` | compose | ordinary finite sound Form | Feedback names are Presentation/policy choices, not device primitives |
| `set_silent` | replace | sound policy/authority or absence of a sound Plan | No global private Brainstem audio state remains |
| `set_lights` | keep | portable indicator/Presentation output | Bounded light meaning lowers to the Create LED profile |
| `dock` | keep | portable docking request | Exact authority, refusal, opcode realization, and terminal Sign are proven |
| `power_state`, `create_power_on`, `create_power_off` | service | power observation and bounded device service action | Unknown/off/on remain distinct; toggle/start/baud actions are authorized |
| `restart_create` | service | Create device lifecycle operation | Stop-first behavior and fresh device re-observation are proven |
| `reset_odometry` | service | bounded odometry-frame reset | New frame/provenance generation is explicit |
| `zero_imu_orientation`, `clear_imu_orientation` | service | bounded calibration operation | Calibration identity, frame, freshness, and invalidation are explicit |
| `calibrate_turn`, `orientation_probe` | service | attended calibration Form/action | Motion authority, duration, safe stop, result, and failure are retained |
| `set_mode` | service/realization | Create OI device-mode operation | Mode is realization truth; unsupported/unsafe transitions are refused |
| `bootsel` | service | Pico W firmware-maintenance action | Physical Host identity, authority, safe disposition, and terminal loss are explicit |
| `reset_motherbrain` | service or delete | exact installed GPIO service attachment | Unavailable unless installed and observed; never implied by the common Form |
| `Unsupported` retired verbs | delete/fixture | no production seam | Exact compatibility rejection fixtures remain only while useful |

The old navigation-shaped verbs (`face_bearing`, `track_bearing`,
`hold_heading`, `turn_to_heading`, `turn_by`, `drive_for`, `arc_for`,
`creep_until`, `scan_arc`, `dock_align`, `wall_follow`, `wiggle_align`,
`bump_escape`, and `unstick`) remain deleted from Brainstem. Navigation,
alignment, and recovery are outside this epic; no compatibility Gear is added.

## Observations and portable values

Every observation carries an exact producing Host/Boot/implementation,
observation Sign, clock/domain, frame where applicable, observation time or
bounded age, calibration identity where applicable, and availability state.
Stale, missing, malformed, and unavailable are not values fabricated as fresh.

| Brainstem sensor/state | Disposition | Portable contract | Realization source |
|---|---|---|---|
| left/right bump | keep | structured contact observation; `robotics/observe-bump` remains the aggregate convenience | Create OI packet 7 flags |
| cliff left/front-left/front-right/right | keep | structured cliff hazard with exact location and optional signal | Create OI packet flags/signals |
| wheel drop | keep | structured wheel-drop hazard observation | Create OI packet 7 |
| wall | keep | proximity/contact-adjacent observation, not fabricated metric range | Create OI wall flag/signal where supported |
| virtual wall | keep | virtual-wall/beacon observation | Create OI packet flag |
| IR byte | keep | bounded IR/beacon code observation | Create OI IR packet |
| buttons | keep | ordinary structured button/input observation | Create OI buttons packet |
| battery charge/capacity/voltage/current/temperature | keep | richer power/battery observation | Create OI power packets |
| charging state/source and board charging indicator | keep | structured charging observation with provenance per source | Create OI plus optional GPIO input; disagreement remains visible |
| distance/angle delta | keep | `robotics/observe-odometry` with start-local frame | Create OI distance/angle integration |
| OI mode | keep as realization truth | Create device observation, not portable robot identity | Create OI mode packet |
| overcurrent | keep | actuator/device hazard observation | Create OI packet 7 |
| IMU orientation | keep | `robotics/observe-imu` orientation with body frame and calibration identity | MPU-6050 over I2C |
| acceleration, tilt, impact | keep | structured inertial observation and mandatory local hazard inputs | MPU-6050 over I2C |
| Create responsiveness/power | keep | device observation with freshness | finite OI transaction and optional power GPIO |

`robotics/observe-range` is used only when a mechanism honestly yields bounded
metric range. Boolean wall/cliff/virtual-wall bits are not silently promoted to
distance.

## Outputs and realization responsibilities

| Brainstem output | Disposition | Portable seam | Required lower seam |
|---|---|---|---|
| differential motion | keep | `robotics/drive-differential` consuming velocity intent | mandatory safety realization -> Create OI codec -> UART Base |
| zero/stop | keep | actuator safe disposition | priority finite OI stop write; local fallback on provider failure |
| speaker/song | keep | standard music/sound kinds | finite Create song slots/opcodes -> UART Base |
| feedback chirps | compose | bounded sound Forms selected by Presentation/policy | same speaker realization |
| dock | keep | portable docking action | Create dock opcode -> UART Base |
| Create LEDs | keep | indicator/Presentation output | finite LED opcode -> UART Base |
| Create power toggle | service | power service action | exact GPIO output and timing Base; embedded only unless separately offered |
| translator enable | keep below semantics | attachment safety/resource prerequisite | exact GPIO output; embedded physical configuration only |
| local status LED | keep | Presenter/Host status manifestation | exact GPIO/PWM output |
| SSD1306 OLED | keep | bounded Presenter projection | SSD1306 attachment -> I2C Base; failure is non-fatal to safety |

## Mandatory local safety invariants

Physical differential-drive offers are invalid unless their selected
realization includes one admitted local safety envelope. A Form cannot route
around it. The envelope owns finite stop work and safe output disposition.

| Invariant | Required trigger/effect | Required evidence |
|---|---|---|
| motion TTL/deadline | expiry commands zero output | expiry and zero-output Sign at the local clock |
| E-stop | local asserted input/state inhibits motion | asserted, latched, refused, conditional-clear Signs |
| wheel drop | any applicable drop inhibits motion | sensor generation and latch generation retained |
| cliff | applicable cliff inhibits motion | exact detector/location and latch generation retained |
| bump/contact withdrawal | stop, then only bounded direction-safe withdrawal if retained | contact generation scopes the reflex; no general escape authority |
| authority/control loss | revoke and stop | authority identity and stop terminal Sign |
| Create UART/device loss | stop and mark actuator unavailable | UART-provider loss differs from device no-response |
| watchdog starvation | hardware reset/safe output | watchdog capability and last terminal evidence are not fabricated afterward |
| tilt/impact | thresholded local inhibit | calibrated IMU identity, threshold profile, and latch generation |
| charging | inhibit physical motion while applicable | source, state, interlock, and disagreement truth |
| all failures | zero/safe disposition | failure cannot be converted into retry or success |

Pico W HIL must prove the independent watchdog and every installed physical
input. The std Host must not advertise an independent hardware watchdog or
auxiliary GPIO/I2C safety input it does not possess. Both must still preserve
the common non-bypassable safety contract appropriate to their admitted
physical configuration. Missing mandatory inputs make the full-safety motion
profile unavailable. A std Host may instead offer the visibly distinct
`no-independent-watchdog` reduced-safety profile only through an exact
authority that records either a current wheels-off-floor attestation or a
Plan- and attachment-bound operator acknowledgement for floor motion. An
absent watchdog or auxiliary input is never encoded as healthy or clear.

## Bases, device protocol, and Host differences

| Responsibility | Pico W realization | std realization | Portable Form impact |
|---|---|---|---|
| Create transport | hardware UART, TX/RX pins, 57,600 8N1 | admitted serial/UART provider, exact 57,600 8N1 | none |
| electrical compatibility | observed/enabled level translator; 5 V Create TX never reaches MCU GPIO directly | exact adapter electrical contract belongs to the physical attachment | none |
| Create OI session | finite codec, allow-listed packets/opcodes, bounded partial-frame state | same codec/session contract | none |
| power toggle/translator OE | exact GPIO outputs and timing | unavailable unless separately installed | common Form must not require them |
| charging indicator/E-stop | exact GPIO inputs when installed | unavailable unless separately installed | observations/offers differ honestly |
| MPU-6050 | attachment over admitted I2C controller/pins | unavailable or separately realized | IMU-dependent Forms may not place there |
| SSD1306 | optional Presenter over admitted I2C | absent or separately realized | no effect on robot semantics/safety |
| deadlines | monotonic embedded timer | monotonic std provider | same finite TTL meaning |
| watchdog | independent hardware watchdog | only an honestly offered external watchdog; otherwise unavailable | physical motion offer reflects the difference |
| upstream connectivity | CYW43 Wi-Fi LINE/provider where used | ordinary std LINE/provider | Create UART remains independent |

The selected Plans must expose different Host, Boot, Base, provider,
implementation, and resource identities while the canonical Form remains byte
for byte unchanged.

## Events, status, and transport retirement

| Brainstem responsibility | Disposition | Conduit replacement |
|---|---|---|
| boot/tick events | replace | Host/Boot identity and admitted clock/deadline work; ticks are not product events |
| power/mode requests | replace | service action lifecycle Signs |
| raw packet received/decoded | fixture/replace | bounded protocol diagnostics plus observation provenance |
| drive requested/stopped | replace | action admission, Play/cancellation, and terminal Signs |
| `CreateNoResponse`, UART framing, timeout, invalid packet | keep | distinct device/Base/protocol failure classes |
| command accepted/rejected/completed history | replace | typed action/refusal/terminal Signs |
| private status snapshot | delete | Presentation/Observatory projection from authoritative state |
| private capability arrays | delete | exact Host advertisements/offers/resources |
| private event ring/RPC cursor | delete | bounded Sign retention using ordinary policy |
| UART compact command protocol | delete/fixture | Conduit actions/Ports over a selected LINE; exact rejection bytes may remain fixtures |
| HTTP JSON command API | delete | no canonical robot RPC; optional HTTP is a Presenter only |
| UDP command/discovery API | delete | ordinary LINE discovery/connectivity and Host identity |
| AP/DHCP/mDNS/ICMP/TCP/HTTP plumbing | replace or delete | bounded platform/network Bases and LINE realization only where required |
| build identity response | replace | exact artifact/implementation/Host/Boot identities |

The exact advertised event inventory has these dispositions:

| Brainstem event names | Disposition and replacement |
|---|---|
| `boot` | Host/Boot identity Sign |
| `command_accepted`, `command_rejected`, `command_started`, `command_completed`, `command_interrupted`, `command_timed_out`, `command_renewed` | action admission, refusal, progress, cancellation, deadline, and terminal Signs |
| `body_power_requested`, `body_power_changed`, `body_mode_requested`, `body_mode_changed` | authorized service-action and fresh device-observation Signs |
| `telemetry_received`, `sensor_frame_decoded` | protocol diagnostics and exact observation provenance, not product events |
| `motion_requested`, `motion_stopped`, `motion_inconsistency_detected` | actuator admission, safe-disposition, and device/actuator-fault Signs |
| `safety_tripped`, `safety_cleared` | mandatory safety-envelope transition Signs with exact generation |
| `bump_changed`, `cliff_changed`, `wheel_drop_latched`, `wheel_drop_cleared`, `wall_changed`, `virtual_wall_changed` | typed contact/hazard/proximity observations and safety Signs |
| `battery_low`, `charging_state_changed` | typed power observations and charging-interlock Signs |
| `buttons_changed`, `ir_changed` | typed input and IR/beacon observations |
| `heartbeat_expired`, `estop_latched`, `estop_cleared` | authority/control-liveness and E-stop safety Signs |
| `imu_frame_received`, `imu_fault`, `tilt_changed`, `imu_calibration_changed`, `impact_detected` | typed inertial observations, calibration identity changes, and safety Signs |
| `contact_withdrawal_started`, `contact_withdrawal_completed` | hazard-generation-scoped reflex lifecycle Signs |
| `audio_state_changed` | sound policy/Presentation observation; delete private global state |
| `error` | delete generic bucket; retain the narrow typed Base/device/protocol/safety/action failure |

Transport reachability never grants motion authority. LINE loss, Host loss,
UART-provider loss, Create no-response, and safety inhibition remain separate
states and produce separate evidence.

## Completion gates

A row moves from historical input to retired only after all applicable gates
are recorded:

1. portable contract conformance, including negative, pressure, cancellation,
   stale, and terminal cases;
2. finite Create OI codec/device conformance over a generic UART Base;
3. std physical Create observation, bounded authorized motion, Stop, and failure
   injection;
4. Pico W HIL for exact UART/GPIO/I2C/timer/watchdog resources and the same
   portable behavior;
5. one byte-identical canonical Form produces two exact Plans and Plays through
   the production kernel;
6. obsolete Brainstem production RPC/runtime code and product vocabulary are
   removed, leaving only explicitly named historical fixtures.

Until those gates pass, `pete-brainstem` is quarantined design quarry and
describe-only evidence. It is not an accepted production realization.
