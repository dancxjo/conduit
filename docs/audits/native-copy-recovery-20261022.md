# Native Mask/Show copy-recovery evidence

Refs #5608, #5994 and #6255. Source
`70ed87ef863c0f9aefd743dd065fbd35fcdc9755` built the existing supported
`cargo +stable xtask make conduitos compositor-semantic-recovery-proof`
appliance. The [original packet](evidence/native-copy-recovery-qemu-20261022/)
retains the full build/proof log, exact receipt, guest transcript and hashes.
The source and transcript hash were independently checked after completion.

This is the explicit `compositor-semantic-recovery-proof` fixture appliance,
not the ordinary uninstrumented graphical product. It uses the actual native
Make profile (256 MiB runtime arena/ceiling, unchanged), production Mask/proxy,
protected compositor session owner and canonical Birth draft. Its counting
scanout target deliberately returns `DisplayError::Lost`; it does not disconnect
a real display or exercise the ordinary Crèche owner-service route.

The actual guest receipt completes all checked stages: initially acknowledged
Show; failed Root copy retires old Show and keyboard input; old submission
refuses without a semantic revision change; independent fresh service instance
has no input authority before presentation; public atomic presentation completes
copy/acknowledgement before fresh Show/Play; held old input refuses; a new exact
interaction correlates and advances the canonical draft. The receipt validator
requires the unique exact outcome and enforces successful guest exit before
writing the retained receipt. The terminal log reports `PROVED`.

The outer tool session nevertheless returned status **143**. Its qualification
is retained separately. The valid completed guest receipt is evidence for the
fixture checks; this packet does not claim a clean zero-exit outer invocation.
No rerun was launched to hide the discrepancy. No source or production/proof
limits changed during the run. Previous ordinary product ELF/ISO and manifests
were preserved separately before building the appliance.

Physical scanout, human observation, guest CPU-fault recovery, malformed guest
reply recovery, ordinary-product provider disconnection and automatic user
recovery remain unproven here. Existing actual ordinary-product screenshots and
input packets have their own Source identities. Scene/resource-pressure and
current-dev combined-head acceptance remain separate #5608/#5994 requirements.
Read-only merge preflight against #6252 found seventeen overlapping files; no
branch merge or source mutation was applied. #6252 advanced to `f58ff6206` with
Candidate 39841239143 while this appliance built; prior-head results are not
acceptance for that head.
