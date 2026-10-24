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
