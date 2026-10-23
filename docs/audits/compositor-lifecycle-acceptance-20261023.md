# Protected compositor lifecycle acceptance, 2026-10-23

This index maps #6185 to actual, source-specific proof. Current normal-product
execution is `c6bf3e72cc828b125e3ff9532df0af5a8db1672f`; the separate semantic
appliance executed `a1610d7d95b42aac1ee0d4d1907f878a8a65e05b`. Documentation and
delivery commits are not additional executions.

## Normal retained product

The [current normal packet](https://github.com/dancxjo/conduit/tree/dce84cb8ba2d7240904b12d6a0b9c623dd31dd6a/docs/audits/evidence/compositor-normal-lifecycle-20261023/c6bf3e72c)
retains the actual ProductHost Root/ISO, manifests, raw serial/QMP, frames, strict
lifecycle and product receipts, command logs and independent reviews. Build and
boot verification passed. An initial journey was interrupted by the build
runner's disk reserve; its partial evidence remains retained. The same image
then passed the supported retained-image journey and final artifact verification
without another build. Runtime work grants and proof bounds did not change.

The strict collector accepted 23 closed lifecycle records for service instance 1:

| Checkpoint | Records | Acknowledged namespace / frame / guest sequence |
| --- | --- | --- |
| Prepared | 5 | No presentation acknowledgement required |
| Initial Hub | 6 | 1 / 1 / 1 |
| First Creche | 6 | 2 / 3 / 5 |
| Playing Hub | 6 | 16 / 2258 / 58 |

Installation/preparation, activation, staging and request costs have separate
observations. Every presented checkpoint joins its actual ACK, costs, Host/boot,
Make and guest identity. Observed service reservation is 19,349,504 bytes and
initialized staging is 8,388,608 bytes; these are observed preparation facts,
not invented manifest declarations. The independent review verified all 101
artifact entries, exact Root inclusion in the ISO, the loaded embedded guest and
27 decoded frame hashes. Existing CI now invokes this collector in its retained
product journey and copies the enriched product receipt, reusing its image and
boot. This wiring inspection does not itself claim a hosted CI pass.

## Stopped cancellation, zeroing, restart and retained work

The [semantic appliance packet](https://github.com/dancxjo/conduit/tree/7d0bb9348c207a3602f96539b6df74c37f176984/docs/audits/evidence/compositor-service-lifecycle-20261023/a1610d7d9)
retains the actual earlier appliance and its strict receipts. A stopped pending
upload is fenced; old protocol and semantic handles refuse; disposal releases
staging and the private slot before reuse. All twelve defined private regions
(19,349,504 bytes) and 2,097,152 initialized staging words are observed zero before
fresh preparation. A fresh service instance advances from 6 to 7, earns an
acknowledged Show and semantic progress, and survives stale-owner Drop. The same
retained grant is used; offer generation stays 1.

Three cold/retained pairs perform the same Face, Scene, raster and framebuffer
work with matching entry/switch/copy counts and finite admitted storage. Cold
means a fresh protected service, not flushed CPU caches. Raw TSC values and
spread remain in the packet; no causal speedup or calibrated latency is claimed.
Observer scans are included in service/owner disposal timings and excluded from
the separately named backend teardown interval. Pressure and stale-Show controls
retain their typed outcomes and real capture evidence.

The current and earlier appliances' relevant clearing, mapping, grant, guest,
proof-body and cfg objects match across 66 checked paths; both actual Root images
contain guest ELF digest `a394faa2fc4f8039fbf32e226dfb05d67efb5c169c0f7b5e6b3ed4eb06be9215`.
The normal packet contains the exact correspondence table and full scope audit.
This bridge does not relabel the earlier appliance as current execution.

## Protection and proof limits

The [actual ordinary protection packet](https://github.com/dancxjo/conduit/tree/1c5fe309708dec9db1dd81db5fcb81bdc8ca5ca4/docs/audits/evidence/privilege-boundary-native-20261023/72d6c3b8e)
retains 26 hostile entries, revocation/restart and Root/sibling sentinel controls,
plus malformed-envelope/shape, fault and non-yielding broker controls. These keep
their executed source identities. Fault quarantine and explicit cancellation
reach the same checked clear/Drop-before-Release implementation. The direct
full-byte witness is cancellation-only; it is not an independent scan at every
fault vector or a guest CPU fault after semantic Show.

Spare allocator capacity, DMA, physical scanout/remanence, asynchronous CPU
cancellation and broad kernel confinement remain outside these claims. No second
cache, larger work grant or unverified resource authority is introduced.

The scoped runtime surfaces are covered by these distinct executions and exact
source bridges. Reviewed delivery, Candidate and relevant post-merge Integration
remain required before closing #6185. Parent #6175 remains separate.
