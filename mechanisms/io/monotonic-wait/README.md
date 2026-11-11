# Monotonic wait ownership

`MonotonicWaitHostCall` retains an initialized native timer provider under the
existing `time/tick` semantic contract and `conduit.host/wait@1` Host Call. Input
is the existing eight-byte monotonic-duration representation; successful waiting
returns no value. Authored cadence remains in Source.

The native Root supplies actual provider ownership, a finite maximum duration,
resource backing, an independently issued capability and the exact selected
Plan/Play. Configured Tick entries must match Root's independently checked Source
facts. Binding does not copy configuration from incoming Plan bytes or issue
authority. This cooperative boundary does not confine hostile native code.

The provider performs no Play allocation and must quiesce pending work when its
future is dropped. Wrong node/call, stale requests, malformed or overlong
durations and revoked possession refuse before provider work. Zero duration is
valid. Provider pressure remains distinct from success. Dropping a pending wait
first drops provider work, then revokes the unfinished capability. There is no
implicit retry or result buffer.

```sh
cargo xtask check monotonic-wait
```

This entrance checks configured zero-result native wait binding, exact
sequencing, duration limits, cancellation, retirement and allocation-free
invocation. Timer behavior, Host identities and issuance in
these tests are explicit CPU fixtures. `ReadyMonotonicWaitBase` retains actual
provider identity and resource generation before projecting an offer through the
existing Base registry. Projection starts no timer work. Binding checks native
identity and all selected/configured facts before taking Root's retained slot;
refusal preserves that slot, and successful transfer removes its availability.
Configured duration and count must fit the native attachment and issued budget.

The ESP32 Root initializes its concrete provider only after RTOS monotonic-clock
initialization and retains one pending-wait slot. Complete clock/task/kernel
memory admission, assigned installation and actual climate Body start remain
separate. Readiness alone starts no wait or running appliance.

`bind_assigned_retained` applies the same finite Source and local capability
checks to the constrained native assignment. Configuration keys, types, values
and order must match Root's independently retained checked Source. A stale Boot
or changed configuration leaves the timer slot available. Only complete
preflight transfers it. CPU tests encode an ordinary assignment, refuse stale
and missing expectations, then transfer and invoke without Play allocation.
This does not claim physical assigned timer installation.

The selected assigned binder accepts either contiguous bytes or borrowed
canonical records through the same Core validation and custody checks. Both
forms run the timer transfer/refusal proof. Refs #6346.

The dev candidate also passed the environmental runtime's four `capture_kernel`
CPU integration tests after the assigned-input API update. They traverse the
production kernel, reject substituted local grants/attachments and replacement
providers, and check fresh Boot identity. These checks do not establish physical
timer installation, overnight operation or a completed recorder host.

The optional `clock-at` feature supplies the shared `clock_at` operation for
`machine/clock/at` and `conduit.host/monotonic-clock-at@1`. Its selected owner
retains exact Plan/Play possession, one pending lease, prepared canonical codecs,
and the provider. `start` validates the request before effects; `poll` performs
one bounded observation, up to the admitted 4096 polls; `revoke` quiesces the
provider and prevents late completion. Results preserve actual observed time,
clock refusal classes, and monotonic checks. This supplies no hardware clock or
authority. Root must separately admit all retained storage and polling work.

Clock-only MCU consumers select `default-features = false, features =
["clock-at"]`; the existing duration-wait API remains the default `wait` feature.
Reviewed clock request/result schemas are compiled at build time. Runtime
clock-at construction decodes those exact finite bytes and needs no parser.
`clock-at-catalog` additionally enables preparation-time authoring catalogs for
consumers such as ConduitOS; it is unnecessary for MCU execution. ConduitOS
retains its platform installation and composite dispatch around this same
shared operation owner. This does not establish Green integration, physical
BME280 sampling, HT16K33 output, or a fitted complete MCU workset.

`clock_at::retained::ReadyClockAtBase` retains the actual calibrated provider and
independently initialized implementation/profile facts. Its constrained
`check_assigned[_fore]` and `prepare_assigned[_fore]_retained` entrances use the
same Core assignment validation and opaque possession as ordinary selected
binding. They accept contiguous bytes or borrowed canonical records without
reconstructing a full Plan. Stale Host/Boot/generation, substituted native
profile, capability, authority, artifact or local Fore refuse before transfer.
The complete 512-byte result envelope and 4096-poll grant remain mandatory.

A prepared ticket exclusively borrows the original ready slot and prepares its
canonical codecs. Dropping it leaves that provider in place. Binding moves the
same provider exactly once into the shared operation owner; it grants no new
capability, retries, budget or time basis. Bind/start/poll use no allocation after
preparation. Root must still charge actual codec, identity, provider and opaque
table storage before Play; these APIs do not assert complete workset fit.

`observe_monotonic` is a separate owned descriptive observation. Its original
basis formatting and Host/Boot String clones allocate; Core's owned instant
constructor itself adds no allocation. This remains explicit and separately
funded, outside the allocation-free MCU deadline start/poll path. No unchecked
clock identity, borrowed substitute instant or cached descriptive copy is added.
