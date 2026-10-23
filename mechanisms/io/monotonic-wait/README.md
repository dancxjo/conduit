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
cargo xtask prove environment wait
```

This entrance checks zero-result capability issuance, existing subject binding,
configured native wait binding, exact sequencing, duration limits, cancellation
and allocation-free invocation. Timer behavior, Host identities and issuance in
these tests are explicit CPU fixtures. A retained ready advertisement, an ESP32
provider and actual climate Body installation still need to be connected; this
crate alone installs neither a clock nor a running appliance.
