# Recorder construction

This describes the prepared software contracts. The real browser recorder,
its persistent reference catalog, Line and Body installation remain incomplete.
The public conformance entrance is `cargo xtask prove environment recording`.

## Meaning and readiness

`install_recovery_kind` and `install_generation_source_kind` register meaning
with the ordinary Source checker and expansion catalogs. They initialize no
provider and create no Host, Boot, storage authority or generation.

`ReadyRecordingRecovery` owns an initialized recovery Back for one matching
gear in the checked workload. Root supplies the recorder's actual current
Host/Boot separately from the authenticated historical producer context.
`ReadyRecordingGenerations` owns a finite ordered list of already selected
portable references, with all encoded output reserved before Play. It creates
no reference and performs no storage lookup. An empty list closes normally.

Both use the same `ReadyRecordingSoftware` custody: advertise only while the
actual initialized Back is retained, check exact Source/Host/Boot/generation
and realization selection, then transfer that object once. A rejected selection
keeps the object. Root must replace historical advertisements after transfer;
an old descriptor is not continuing availability. These transfers do not start
Body/Play or authorize storage.

## Exact storage access

The generation source emits `resource/ref` values, matching the existing
snapshot-read Fore. Its cursor advances only after the kernel commits an
emission. Output pressure, insufficient fuel or an uncommitted Step cannot
skip a generation; cancellation stops emission. Preparation rejects duplicate
identity/version pairs, incompatible profiles and an exceeded finite ceiling.

A portable reference is data. Snapshot reads still need independently admitted
current residence, exact resource bindings and authority. The existing browser
snapshot Back binds one exact generation; a list cannot silently broaden that
binding. A complete batch needs the corresponding admitted read routing.
Root must also obtain and retain its ordered reference catalog through admitted
state. This source does not discover a catalog, reserve manifest slots or prove
that referenced content has been published.

## Batches, recovery and Face

`PreparedRecordingBatch` initializes every encoder, publication Back and commit
observer for the complete finite workload. The default 960 samples occupy 15
full 64-sample chunks. Root supplies each immutable generation and its exact
content extent. Partial flushes require separate admission. The constructor
makes no store live and owns no implicit retry, next generation or eviction.

A `RecordingSnapshotBack` advances its observer only through an actual committed
Host Call completion. `RecordingRecoveryBack` similarly exposes history only
after committed consumption of valid content in the expected producer context.
Bad content, cancellation and capacity refusal preserve previous recovery.

`PreparedRecordingFace` projects committed recovery counts, sequence range,
known gaps and refusals into the ordinary Face grammar. The HTTP
`recording_response` Mask formats that sealed Face and retains its identity and
revision. It keeps no second recorder state. Recovered history is distinct from
a new durable write acknowledgement and from fresh sensor observation; typed
Temperature and humidity remain in the observation/history contracts.

CPU conformance covers Source/Plan selection of initialized software, exact
ownership transfer, transactional progress, recovery and Face publication.
It does not establish IndexedDB persistence, a live Line, host membership,
physical overnight recording or complete MCU memory admission.
