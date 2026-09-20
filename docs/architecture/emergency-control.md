# Emergency authority reduction

Emergency control is a small authority-reduction boundary beneath ordinary
Forms, Plans, Plays, presentation, speech-to-text, and model inference. It is
not a second lifecycle system. A Host admits an exact `EmergencyPolicy`; local
physical, local acoustic, keyboard-rescue, and authenticated remote adapters
may then submit boot-scoped `EmergencyRequest` values with their distinct
provenance.

Acceptance at this boundary requests effects. It does not claim they completed.
Graceful Lull, local execution revocation, carrier isolation, propagation,
machine halt, and machine reset remain separate outcomes. A target-specific
machine base owns any terminal halt or reset mechanism and must report absence
rather than substitute a generic implementation.

ConduitOS Ctrl+Alt+Delete deliberately remains a dedicated boot-scoped reboot
rescue rather than entering this Body-scoped seam. It is available before a
Body exists, carries authority only from the validated local HID path, and
requests exactly the architecture reboot base. Routing it through
`EmergencyControl` would either invent a Body for the no-Form boot case or make
Body membership a prerequisite for recovering the machine. Its receipt names
the `conduitos/dedicated-boot-reboot@1` route and `boot` authority scope so the
special case cannot be mistaken for general emergency-policy admission.

The distinction is about admission scope, not priority: the chord remains
below ordinary Form/Plan/Play input and therefore works both before any
ordinary plan and while a play is active. Other keyboard rescue mechanisms may
use `EmergencyTriggerClass::LocalKeyboardRescue` when they genuinely operate
on an admitted Body policy; they must not acquire ConduitOS's local physical
reboot authority merely by producing equivalent semantic key values.

The acoustic key is three distinct entries from one versioned finite vocabulary.
Birth may retain it only with the exact detector and admitted entropy-provider
identity. There is no clock, Body identity, or deterministic fallback when a
profile claims random selection. The phrase is recovery information, not a
cryptographic secret; knowledge of it grants only a local authority-reduction
request and never Wake, authentication, policy change, or ordinary privileges.

The fixed sequence matcher consumes only bounded vocabulary detections. Wrong
order, unrelated speech, invalid confidence, overflow, stale microphone
generation, loss, and timeout reset progress or make the path unavailable. A
successful phrase fires once and remains suppressed until recovery through a
separate, stronger authority path. Raw audio is not part of the durable Body
configuration or emergency outcome.

Keyword spotting and finite sequence matching are also ordinary reusable gear
semantics with typed audio, observation, and trigger ports. Their ordinary Form
outputs are inert observations: they neither admit emergency authority nor
perform an effect. A safety Host may install the same bounded implementations
beneath Play for the out-of-band path. Emergency admission and terminal machine
halt/reset deliberately are not callable Form gears, because ordinary work must
neither acquire shutdown authority nor become able to delay or veto that path.

The std Host composes that out-of-band path through one
`AcousticEmergencyAdapter`. It pins the durable birth key, exact offline detector
version, canonical Base-registry microphone provider identity and generation,
sequence matcher, and Body-scoped `EmergencyControl` before accepting PCM. A
detector match is still
inert until the complete configured phrase is observed. The complete phrase can
only request the reductions admitted by `EmergencyPolicy`; it cannot Wake,
authenticate, grant authority, resume work, or consult a Form, Plan, Play, model,
or transcript. Physical mute, provider loss, replacement generation, sequence
failure, and input overflow make the adapter unavailable and clear partial
progress. Recovery requires a fresh adapter admitted against current provider
truth; it is never an implicit retry.

Remote emergency admission consumes only frames opened by an exact current
protected Line session. The local adapter additionally pins one current Body
membership credential, peer Host/Boot, and session binding; encrypted reachability
alone is not authority. Each bounded request carries the exact Body and credential
identity plus a monotonic session freshness value. The emergency phrase is absent
from this protocol and knowledge of it grants no remote authority.

Local reduction is decided before optional onward propagation and does not wait
for remote peers. Propagation has a finite target count, no implicit retry, and
retains each delivered, unreachable, unauthorized, or refused outcome so partial
reachability cannot be reported as global success.

The current source-level contract and deterministic tests do not establish a
microphone implementation, false-positive threshold, architecture halt/reset
support beyond the existing x86_64 rescue reset, physical/HIL behavior,
distributed propagation, or stable-release acceptance. Those proof classes
remain explicit work under issue #3682.
