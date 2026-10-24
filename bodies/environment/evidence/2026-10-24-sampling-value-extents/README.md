# Sampling payload Type ceilings

The checked protocol Types now generate an immutable payload extent table during
build preparation. `ApplianceSource::maximum_value_bytes` checks expansion
provenance and all normal gear ports against those linked Types, plus the native
Tick and bare Empty codec lengths. Unknown and abnormal Types refuse. This
retains first-class quantities and grants no provider or execution authority.

All current authored ports fit 2,274 bytes, the canonical Dht11Result ceiling.
The existing two large slots are reduced from 4,096 to 2,274 bytes; seven 1,024
byte slots remain. A generated compile-time check prevents a changed reading
Type from silently exceeding the reviewed large-slot profile. This is a
narrowed existing profile, not a general graph lifetime allocator. Payload
backing is 11,716 rather than 15,360 bytes, excluding unchanged slot metadata.

The retained CPU checks pass 44 tests: ten appliance/kernel tests, one Source
construction test, two extent tests, fourteen native assignment tests and
seventeen Root-control tests. They exercise 960 samples through the production
kernel with valid data, checksum refusal, timeout, provider failure, geometry
refusal, malformed reading and short capture. The recurring allocation probe
remains active. Strict library/extent-test Clippy also passes.

These tests establish the current finite authored profile. They do not prove
arbitrary fan-out changes, a combined sampler/Mask/output allocation, physical
overnight sampling, stack high water, or concurrent Wi-Fi transitions. The
previous physical sampler capture used the earlier 15,360-byte payload profile;
its retained and peak figures must not be relabelled as current measurements.
The native Face ESP32 release image links successfully (13 existing warnings).
The device was not flashed for this change.
