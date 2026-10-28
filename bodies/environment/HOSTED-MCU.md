# Linux-owned Body with a native MCU member

This attended profile moves ordinary planning and the Body lifecycle onto Linux.
The ESP32 owns its initialized providers and checked resident software. Linux
admits that actual Host through challenge proof, plans against its current
offers, and transfers a bounded assignment. Root verifies its independently
linked Source, issues native possession, and starts held. Linux records the
ordinary Body start before requesting signed release. The MCU then samples
through the shared execution kernel; bench readings are not substituted.

## Construct and run

With the pinned ESP32 toolchain installed, create a private owner directory once:

```sh
cargo xtask prove environment owner-key /absolute/private/environment-owner
cargo xtask prove environment member-image /absolute/private/environment-owner/owner-public.bin
```

The member ELF is `target/environment-esp32/xtensa-esp32-none-elf/release/conduit-esp32-tm1637-probe`.
Flash that image using the existing ESP32 hardware workflow. The public key is
provisioning data; `owner-secret.bin` stays on Linux. The build checks an explicit
Conduit runtime package allowlist, retains the local indexed task planner, and
refuses linked Source parsing/checking/expansion. Source authoring remains on
the build host. This member image embeds
checked software, not a future Host/Boot, selected Plan or Body authority.

After a fresh MCU Boot, with UART permission, run:

```sh
cargo xtask prove environment host-owned /dev/ttyUSB0 /absolute/private/environment-owner
```

This births a Body with its existing Crèche-generated name, installs Linux as
first host, admits the MCU and verifies one scheduled physical publication.
It then requests signed cancellation/retirement and lulls the Body. An optional
final count, `1` through `960`, runs that many publications; the linked Source
samples once a minute, at most 960 times. SIGINT/SIGTERM requests the same
bounded cleanup. A carrier failure can leave cleanup unresolved; retained
evidence must not be rewritten to claim quiescence.

Each process has a private `runs/<boot-uuid>/` directory. It retains admission
proof, Body biography transitions, signed Root control/publication evidence and
the semantic environmental Face. Re-entry into an already controlled MCU Boot
refuses before birthing another Body. Restart recovery and unattended service
installation are not implemented by this attended profile.

The quantities remain first-class Temperature and Ratio values. Linux applies
the existing Fahrenheit Look and temperature/humidity Masks to that Face,
reporting four segment patterns and annunciators as previews. **These previews
do not write the physical display.** This native run currently admits only the
sampler workset. Continuous display execution, ordinary joining invitations,
external Fore recording and durable reboot continuation remain delivery work.

## Combined display preparation measurement

Build the attended memory checkpoint with the same public provisioning key:

```sh
cargo xtask prove environment member-image /absolute/private/environment-owner/owner-public.bin --display-fit
```

After flashing and a fresh Boot, use the same `host-owned` command. The image
retains the display computations and Fahrenheit/RH Look beside the running
native sampler. Authenticated control replies include `display_fit` telemetry
with explicit `physical_write: false` and `mask_play_installed: false`.
This is a fit checkpoint, not an enabled physical display. After selector and
Source compaction, measured free Root memory exceeds the existing
output-kernel preparation gate. The separate bounded physical check below
exercises actual output; continuous Mask execution remains open. See [the target measurements](make/TARGET_CHECKPOINT.md#compiled-scalar-and-observation-lowering).

The focused CPU construction/output proof, including exact offer equivalence,
stale-scope refusal, packet equivalence and allocation-free execution, enters
through `cargo xtask prove environment display-output`.

## Bounded native display run

Build the combined native diagnostic and, after flashing and a fresh Boot, run:

```sh
cargo xtask prove environment member-image /absolute/private/environment-owner/owner-public.bin --display-check
cargo xtask prove environment host-owned /dev/ttyUSB0 /absolute/private/environment-owner 2 --display-check
```

The Body workset contains the sampler and ordinary output Plot. Both are planned
against actual current offers, held under separate exact reservations, and
released after the same Body start. The output's exclusive local Fore borrows
the actual MCU publication Face; no register commands or invented measurements
arrive over UART. It alternates Fahrenheit and RH through the existing pure
Mask/Look helper, with a three-second wait before the next page. The shared
kernel retains the checked computations and routes across individual Frames;
request identities and the original finite write ceiling survive reuse.

The physical profile admits at most 19,200 Frames. The owner remains attached:
one complete invocation's evidence occupies a 64-record log and a bounded
2,064-byte transport page. Linux verifies the device signature and exact page,
persists it with its signed provenance, syncs both files and directory entries,
then signs an acceptance bound to the Body, Boot, Play, invocation and digest.
Only that acceptance allows buffer reuse. A missing acceptance pressures display
output while the independent sampler continues. Wrong/stale acknowledgements
cannot erase custody or replay writes. Owner control protocol 3 requires the
matching owner executable and firmware; there is no implicit legacy fallback.

The owner records publication commit and observation clocks in the MCU Boot
domain. The Face's existing two-minute age policy determines freshness; a signed
reading older than five seconds no longer terminates a minute-period appliance.
The publication count bounds the attended run. On completion, the owner cancels
output, archives terminal evidence, retires both owners and records Body lull.
Increasing that count does not increase the finite display work ceiling.

This remains an attended physical checkpoint, **without an admitted ordinary
Mask Plot/route**. Final display blanking and a complete semantic withdrawal path
remain open. Confirmed native completions and signed evidence are instrumented
physical execution; LED appearance still requires human observation. The
[measurement ledger](make/TARGET_CHECKPOINT.md#reusable-physical-output-and-evidence-handoff)
records target RAM, flash, stack and the physical run.

`cargo xtask prove environment display-output` covers complete checked-graph
reuse, conditional refusal outputs, evidence encoding, custody, signed
acknowledgements and host age policy. The 960-valid-frame and separate
960-alternating-success/refusal CPU runs use zero execution allocations or
reallocations and have no evidence gaps. They use fixture native completions;
the largest observed complete invocation log is 36 records, not a worst-case
admission claim.

## Pico comparative gate

Use a retained construction birth Sign from the existing environment-image
workflow:

```sh
cargo xtask prove environment pico-maximal /absolute/image-output/birth-sign.json
```

This produces a UF2 in `target/environment-rp2040/thumbv6m-none-eabi/release/`.
It is a memory checkpoint, not a full Host deployment. Its actual PIO GP2
capture provider, initialized monotonic timer, installed Body, resident sampler
software and active Pico W AP remain retained during owned-offer and indexed
Source preparation. The maximal diagnostic uses a fixed 212 KiB heap; the
stack floor remains 36 KiB. The checkpoint also retains an unchanged-196-KiB
comparison so heap growth is separate from compiler savings.
It starts no Play. DHT/display wiring can remain on the ESP32.

The gate is a lower bound: it omits display and the complete admission/control
Root. A failure already here disproves fit for this preparation strategy;
success would still require those owners, actual execution and complete
admission. An allocation fault enters ROM BOOTSEL and retains its bounded fault
receipt; loading the identical image reports that receipt instead of retrying.
See [the measured checkpoint](make/TARGET_CHECKPOINT.md).

## Wireless carrier candidate

`cargo xtask prove environment member-image /absolute/private/environment-owner/owner-public.bin --wireless-display-check` builds the same authenticated Root protocol on one TCP carrier (port 4242), listening on the initialized AP and station interfaces. The existing `/network` page and acknowledged network setup Plot own AP-to-station switching; this profile does not add another credential or radio path. The current mailbox is volatile. Automatic captive-portal opening and durable credential Keep realization are not implemented.

The owner accepts `tcp://192.168.4.1:4242` wherever the serial port was supplied. A retained, signed, lulled biography can be continued using a final `--resume=/absolute/path/body-lulled.json` after `--display-check`. The previous owner must have exited with verified retirement; the new owner uses ordinary continuity under a fresh local incarnation and keeps the Body identity. This is preparation for wireless operation, not evidence of network HIL or unattended recovery.

Do not deploy this candidate as an unattended long-distance appliance yet. The running-job TCP inspection loop can now reopen its explicitly selected peer after a carrier loss, using its retained request counter and a fresh signed inspection of the same Body/Boot/Play. It never retries Prepare/Start/Release blindly. Process recovery, station-to-fallback-AP discovery, and new-Boot continuation still need their separate ownership and admission paths. Saved network configuration requires a finite Body-lifetime Keep backed by actual durable private storage; the existing volatile mailbox cannot advertise that retention horizon.

The experimental `member-image OWNER_PUBLIC_KEY --local-evidence-check` profile now binds the journal to an actual dedicated ESP32 partition. Its required layout is `targets/esp32/proof/tm1637/evidence-partitions.csv`: the application ends before `0x290000`, and the remaining space is split into a `0x160000`-byte display-evidence region and a separate 64 KiB observation-outbox region. The adapter checks the actual table, bounds every read/program operation, and exposes no erase or general application storage entrance. Pre-Play preparation owns and verifies erased backing; a local commit and media readback release the kernel page. Remote acceptance advances only the export cursor. A prior nonempty journal is retained and makes this writer unavailable pending explicit recovery; it is never silently erased.

One physical run retained the same Body over Wi-Fi and stopped the Linux owner application for 90 seconds at a verified idle boundary. The MCU advanced from 42 to 72 display writes, committed 30 pages locally without remote acceptance, and advanced its sensor sample. The original owner resumed and accepted the backlog. A concurrent log-reader partial-line error was corrected by validating the completed records from that same pause, not by repeating the outage. The candidate links at about 2.58 MB within its 2.62 MB application partition; measured Root use is 102,576 of 118,768 bytes. These are this run's measurements, not a general bound for arbitrary workloads.

This still uses the explicitly fenced physical-check Face Fore rather than an installed ordinary timed Mask Plot. Automatic connection recovery, credential retention, general Body-lifetime Keep backing, and new-Boot continuation remain incomplete. These broader appliance behaviors are not implied by local evidence custody.

The repeatable attended proof entrance is `cargo xtask prove environment local-evidence-outage PID RUN_DIRECTORY http://MCU_ADDRESS/reading`, against a running 960-publication owner in this profile. It verifies the actual owner and its open run file, pauses only at an idle boundary, resumes through a pidfd in `finally`, reads only complete log records, and checks signed-controller acceptance plus retained page files. Its 90-second pause proves unavailable-owner progress, not cold Boot or network reconnection.


## Finite observation replay

The same experimental `--local-evidence-check` image also retains an outgoing
observation projection independently of the Linux owner. It uses the existing
temperature, humidity and recording-disposition Forms: 21 bytes per observation,
with sequence and **producer-observed Boot milliseconds**, not an invented
sensor acquisition timestamp. The immutable journal descriptor binds the checked
observation Type, all three Forms and that clock meaning. Each append commits
and reads back before being counted; no remote acknowledgement gates sampling
or display. Retention admits the existing 960-observation lifetime, not an
unbounded recording session. A CPU recovery test retains and replays all 960
records in 64 KiB with 536 bytes of journal workspace.

`GET /history?after=N` exports at most four records from that committed prefix.
The actual admitted MCU key signs a versioned transcript binding the
Body/Boot/active-Play scope, representation descriptor digest, requested cursor,
committed count and exact record bytes. This read-only route grants neither
membership nor storage authority. Bootstrap routes cannot export it. The Linux
owner on the selected TCP peer verifies those facts, exact sequence continuity,
and the observation clock, then syncs each batch and directory before advancing
its local collection cursor. Failure leaves the MCU's retained records intact.
This projection does not replace the full signed canonical publication evidence.

The outbox is Root publication custody, **not** a general application store or
Body-lifetime Keep implementation. It never erases or overwrites a previous
Boot's data; automatic previous-Boot recovery and autonomous Body wake remain
unfinished. Do not assume the appliance can restart without another host yet.

The attended proof entrance is
`cargo xtask prove environment observation-history-outage PID RUN_DIRECTORY http://MCU_ADDRESS/reading`.
It pauses the exact current owner for 150 seconds, verifies continued sensor and
display progress plus at least two retained observation sequences, resumes the
owner in `finally`, and checks that the verifying owner saves the missed history
and accepts the display-evidence backlog. This covers unavailable-owner progress
within one Play, not network reconnection, a power cycle or a new Boot.


On 26 October 2026, the corrected physical profile retained the same Body
through a 150-second stopped-owner check. During the pause its sensor sequence
advanced from 1 to 4, and display calls/local commitments advanced from 15 to
64 while remote acceptance stayed at 15. After resume, the owner verified and
synced missing observation sequences 2, 3 and 4, advanced its existing Face
without resetting the MCU, and accepted the original display backlog. Replayed
observations keep their original Boot observation times; replay arrival only
advances freshness evaluation. The linked image uses 2,594,800 of 2,621,440 app
bytes and preserves the 36 KiB stack floor; measured Root use/peak is
103,536 of 118,768 bytes in this workload.

The first deployment exposed a strict-sequence controller bug and the old
three-page cleanup assumption. That run was explicitly recovered, including
all 60 display pages and four signed observations, before the corrected image
was deployed. The proof harness now takes its durable log watermark only after
the owner is confirmed stopped, avoiding a concurrent-prefix race. These
corrections are part of the tested path rather than a claim that the failed
run succeeded.


## Management-carrier recovery

The attended owner distinguishes byte-carrier failures from filesystem errors,
protocol refusals, invalid signatures and recognized MCU fault diagnostics.
Only the running-job inspection loop retries transport loss. Its admitted job
deadline bounds retry lifetime and attempts, with two-to-thirty-second backoff.
Reopening discards partial framing and retains the same selected TCP endpoint;
it neither replans nor starts the MCU again. The local next request counter was
already consumed before transmission, so unsigned peer metadata cannot advance
it. A conflicting counter or another Host/Boot requires explicit reconciliation.
A valid signed publication under the admitted key and exact reservation is
required before declaring recovery.

A lost display acceptance reply is resolved by another signed inspection. If
acceptance was applied, export continues with the next page; otherwise the same
verified page remains offered. Local file custody precedes every acceptance.
This is not a blind retry of an uncertain operation.

For a management relay or an explicitly separate reading connection, the owner
accepts `--history-peer=IP:PORT`. It selects only a read-only HTTP destination;
the existing Body/Boot/Play/Form/key checks still apply. Ordinary TCP ownership
continues to use the selected peer's port 80 when this option is omitted.

The physical proof entrance, requiring a prepared fresh MCU Boot and a lulled
biography, is:

```sh
cargo xtask prove environment owner-carrier-outage tcp://MCU_ADDRESS:4242 /absolute/private/owner /absolute/prior-run/body-lulled.json /absolute/new-proof-output
```

This attended fixture requires Python's `cryptography` module. It verifies and
drops an actual `EvidenceAccepted` reply, blocks its transparent management
relay for 150 seconds, and checks recovery without lifecycle replay. Its
one-shot fault selector has a regression check. It retains the owner and relay
after the proof so the appliance can continue; explicitly stop the actual owner
when retiring that job. This is not an installed background service.

The first real connection-loss experiment found that the relay kept injecting
loss. The controller nevertheless reconnected twice under the same MCU Boot.
The faulty relay was replaced without restarting that owner or MCU, and the
retained run was independently checked through this read-only entrance:

```sh
cargo xtask prove environment verify-carrier-recovery PID RUN_DIRECTORY OWNER_LOG DROPPED_RECEIPT_JSON
```

That check verified the actually dropped receipt against its exact retained
page/command and admitted key, contiguous signed history through sample 17,
continued signed sampler publications, the original 129-page display prefix,
and no repeated Reserve/Prepare/Start/Release. The original automated relay
harness is retained as a failed fixture run; its corrected one-shot source has
CPU conformance, not another claim of clean automated HIL. The actual native
appliance and its owner remain running through the repaired transparent relay.
Graceful member departure policy, owner-process restart, Wi-Fi association
recovery, mDNS discovery/AP fallback and autonomous new-Boot operation remain
unfinished.

## Live closed-epoch custody acceptance

The joined recorder can request Root acceptance of an exact closed epoch:

```sh
conduit-environment-host --ack-observed-live PRIVATE_BINDING CONTINUITY_PRIVATE HOST:PORT HISTORY_DIRECTORY SLOT
```

`SLOT` is a hint in `0..5`, never authority. The live Root inspects the actual
frozen journal incrementally, verifies the latest retained member credential,
and signs a receipt bound to the exact epoch and request. The host authenticates
the offer, verifies and fsyncs the complete signed history, reopens those files,
and only then signs custody. Lost transport responses retry the same request
within a finite deadline. Invalid signatures, explicit refusals and mismatched
receipts cannot establish success.

The HTTP entrances are `GET /history/custody/offer?slot=N` and
`POST /history/custody/ack?slot=N`. JSON ACKs are capped at 768 bytes inside the
existing 1024-byte total request extent. One request may be pending; identical
retries share its result, and a different request retires the previous response
cache before preparation. Transport and cached receipts grant no erase authority.

Acceptance does not mean that a reclaim marker committed, flash was erased, or
storage is reusable. The Root now connects accepted custody to the permit-gated reclamation job.
Preparation checks the previously durable layout receipt, transfers the actual
frozen owner, and writes nothing. Its first Reclaiming commit, individual erase
and verification steps, and final Reclaimed publication run through the actual
sampling wait provider. Only the final verified proof restores an unbound Cold
slot; cancellation/refusal retains the same owner. The signed HTTP receipt still
confirms acceptance, not job completion. Continued sampling and display under
this connected workload still require physical verification. The first
physical acceptance trial succeeded cryptographically but later exposed a Mask
child workspace refusal; it is not uninterrupted display acceptance evidence.

The USB carrier exposes a loopback-only `/__carrier/environment` diagnostic.
It reads counters through the already owned serial session, never through a
second serial connection or a forwarded request to the selected peer. The
returned identity remains explicitly a carrier, not a Body member. This route
performs no Wi-Fi join or diagnostic retry after failure.
