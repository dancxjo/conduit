# Resident preparation slots on finite heap backing

Resident software keeps its two fixed arrays on preparation heap backing.
Transfer moves their contents into the same assigned appliance arrays without
allocation. Validation and initial wait storage refusal still precede allocation.
The 32-bit proof passes all 19 tests, including all seven 960-sample execution
scenarios and refusal/transfer allocation checks. Native construction passes
22 tests and compilation of both generated recipes. Runtime library Clippy
passes; the attempted all-target Clippy also found an unrelated unused helper
in tests/support/pulse.rs (bind_assigned_budget), which this change does not edit.

The Xtensa Source constructor frame fell from 11,200 to 3,120 bytes. Preparation
peak increased from 60,116 to 62,876 bytes; retained software payload increased
from 51,423 to 54,183 bytes. No execution allocation was added.

A physical intermediate image got past program decoding but failed later in
recursive evaluator preparation (equality::binary / prepare::prepare_node),
still with a stack-guard panic. This is not successful software readiness.
The intermediate image preceded the final allocation-order correction; its
ELF is retained separately under outputs/native-source-slot-backing. The final
allocation-order build is outputs/native-source-slot-backing-final and has not
been flashed. The known-good fixed-memory network image was restored and its
Root replied with an uninstalled network control. The retained logs distinguish
that recovery from the failed intermediate image.

A separate equality frame extraction is under validation; this receipt makes
no claim about its physical outcome or about Body installation or sampling.
