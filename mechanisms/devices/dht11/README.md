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

The result schema has one shared immutable backing exposed as `RESULT_TYPE_BYTES`.
Publication consumers can borrow those exact bytes rather than generate and link
their own copy. The Body chooses cadence, publication, recording and Face/Look
policy; those responsibilities are not part of this device implementation.

The package's hosted tests check Forms, captured-frame conversion, typed failure
and recovery, admitted capture geometry and allocation-free recurring conversion.
They use CPU fixtures, not a physical sensor. `cargo xtask prove environment source`
also checks the separate application Face/Mask integration; the recording proof
includes the shared Forms tests. Normal workspace tests include the device
package, and the Foundation shard enables its two optional features.

Moving the Rust owner does not change the authored protocol or Form identities.
Firmware and Source adoption still require their exact ordinary image, resource
and authority checks; a successful decoder test is not an installed Play.
