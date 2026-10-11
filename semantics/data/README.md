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
