# Unqualified preparation preserved for recovery

This recovery branch preserves the held producer-context witness/test source
and test-fixture adjustment separately from the qualified integration lane.
They are not installed recorder behavior or proof of durable commitment.

The adjacent patch is an unfinished semantic-law layout experiment. Baseline
CPU layout measured KindSemanticLaw272bytes, FlowFold/FlowScan264bytes,
BoundedCollect224bytes and KeyedJoin176bytes. Changing only the four enum
payloads to boxes is incomplete: constructors and borrowed projections must
be updated and canonical wire/fingerprint/refusal parity must be tested.
The partial patch failed compilation and was removed from the delivery tree
before fleet builds. It is retained solely to avoid losing the investigation;
no memory reduction or deployment is claimed for it.
