# DHT11 observation realization

This `no_std` device package realizes the shared
[DHT11 protocol Source](../../../plots/device-protocols/dht11.conduit). Checked
Forms convert decoder coordinates into first-class Temperature and Ratio
quantities. The public Kind remains `device/dht11-observation`; checksum,
length, range, sign and decimal interpretation remain authored in Conduitese.

Preparation owns bounded canonical conversion storage. The optional `kernel-step`
feature supplies the existing pure conversion Back; it uses the ordinary kernel
and creates no sampling loop. The `source-capture` feature derives request
geometry from the checked capture Plot and validates it against a caller-supplied
pulse envelope. It does not supply a GPIO, provider, capability or capture grant.

The opt-in `recording` feature owns the unchanged bounded immutable JSON codec,
ordered recovery and [recording disposition Form](recording.conduit). The existing
Body runtime paths reexport this one implementation. Snapshot Host Calls, commit
observation, authority and application recording policy remain in the Body runtime;
this feature adds no storage effects or Source realization. Default features remain
empty.

The separate opt-in `quantity-history` feature converts validated recordings into
the shared reported-quantity contract. Preparation retains one recovery workspace;
the caller explicitly selects a tail of at most sixteen samples from its latest
chunk. Sequence, producer Boot time, gaps and unavailable outcomes remain intact.
Each encoded batch must also fit its existing 4 KiB profile; sixteen samples are
a cardinality ceiling, not a guarantee that every such encoding fits. Oversize
refuses without automatically reducing the requested window.
This is allocating hosted preparation, not a Step or another history store. Its
backing model excludes caller-owned catalog construction, input/file buffers,
inline storage and allocator overhead. Neither this feature nor its returned
observations supply publication, membership, storage or retirement authority.
Default and recording-only consumers do not enable the Data dependency.

The result schema has one shared immutable backing exposed as `RESULT_TYPE_BYTES`.
Publication consumers can borrow those exact bytes rather than generate and link
their own copy. The Body chooses cadence, publication, recording and Face/Look
policy; those responsibilities are not part of this device implementation.

The package's hosted tests check Forms, captured-frame conversion, typed failure
and recovery, admitted capture geometry and allocation-free recurring conversion.
They use CPU fixtures, not a physical sensor. `cargo xtask prove environment source`
also checks the separate application Face/Mask integration; the recording proof
includes the shared package with `recording` enabled and the existing Body
chunk, recovery and publication tests. Normal workspace tests include the device
package, and the Foundation shard enables `kernel-step` and `source-capture`.
`cargo xtask prove environment recording-quantity` checks the opt-in adapter,
strict device lint and Thumb/WASM compilation. A tail view does not replace the
legacy sixty-four-sample reading or qualify a browser/native Mask.

Moving the Rust owner does not change the authored protocol or Form identities.
Firmware and Source adoption still require their exact ordinary image, resource
and authority checks; a successful decoder test is not an installed Play.
