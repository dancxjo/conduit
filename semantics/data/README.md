# Measurement quantities

Measurement samples and windows retain canonical `Quantity` values, including
fractional decimals and temperature points. Ordering and thresholds compare
points through the common unit conversion law. Point observations without
uncertainty can be retained and transported as points.

The current generic summary stores its range as a `Quantity`, and the sample's
optional uncertainty also has that type. An affine point range or uncertainty is a
**difference**, not a temperature point. Consequently, point summaries and
point samples with uncertainty return `PointDifferenceRequired`;
their wire decoders refuse those invalid representations as well. A dedicated
paired difference field is needed before these operations can accept them.

Other finite measurement summaries preserve exact decimal means when the mean
terminates in decimal. Nonterminating means return `InexactMean`; bounded
coefficient or exponent overflow returns `ArithmeticOverflow`.

## Reported quantity observations

`ResolvedQuantityObservationBatch` carries at most 16 reported observations for
one channel. Canonical prepared decoding checks identities, sequence and Boot-time
order, exact gaps, quantity family/role consistency and explicit unavailable
reasons. An empty batch is a valid finite view. Producer Host/Boot/Plan/Play IDs
are reported data, not evidence of membership, read authority, publication commit
or durable retention. Values retain their original quantities and sample times.

`ResolvedQuantityObservationSnapshot` contains one or two independently resolved
batches. Its prepared decoder checks source/channel/derived sample identity
collisions and requires each standalone batch to fit 4,096 canonical bytes. Its
aggregate ceiling derives from the checked schema and must fit the existing
16 KiB preparation profile. Owned decoding allocates during separately funded
preparation; these types create no recorder, provider, scheduler or Face.

Run `cargo xtask prove environment data-input` for focused conformance and strict
Data lint, `cargo xtask prove environment data-input all` for all Data regressions
and lint, and `cargo xtask prove environment data-target` for standalone WASM
compilation. These entrances do not depend on a Face or installed Source.
