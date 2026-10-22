# Native canonical Patchbay inventory — local implementation checkpoint

Owning interaction gap: [#6201](https://github.com/dancxjo/conduit/issues/6201),
within [#5994](https://github.com/dancxjo/conduit/issues/5994).

Native preparation now moves the admitted target graphs into one existing
portable Patchbay actor per slot. A fixed array maps compact canonical action
indices to sparse native target slots. Selection uses the existing checked
`SelectPlot` command and one monotonic actor revision. It does not replace the
Body Plan or grant Edit/Mask authority. Root independently binds the returned
target to the acknowledged interaction, exact semantic action target, retained
graph inventory and successor revision before updating its selected target.

Preparation records an explicit graph limit separately from allocator capacity.
Spare capacity grants no extra target. FinishInit still moves prepared custody;
it does not rebuild graphs. Inspector selection supplies no Body-focus evidence.

## Local checks

- Application-core library: 45 passed.
- New canonical selection contracts: 3 passed, including sparse switch/back,
  stale/forged/replayed input refusal, exact Body Plan, finite preparation and
  failed later preparation without partial initialization.
- Portable resident commands: 5 passed.
- Root inventory-index conformance: 1 passed, including sparse targets and
  signed, padded, overflowing, empty and unknown indices.
- Production bitmap allocator fixture: passed with the unchanged 2,072,576-byte
  private grant. Init peak was 1,863,712 bytes, canonical protocol peak
  1,916,576 bytes, and Graph/ProjectGraph scenario peak 2,042,048 bytes. Reported
  headroom is 30,528 bytes across these CPU scenarios, not a universal fit claim.

Failed attempts remain in local logs. Corrections retain full validation:
distinct checked Sources replace a duplicate synthetic target fixture; the
private-heap caller joins every admitted resident Plot subject; and the actual
typed encoder mechanically refreshes startup.catz for eight newly serialized
`source_origin: null` fields. Exact wire equality remains asserted. The original
startup fixture and negative logs remain retained locally.

## Remaining gates

Protected Root freestanding compilation, hostile substituted service-return
integration, real QEMU cross-Plot selection and fresh-editor handoff, combined
head CI and merge remain required. This checkpoint earns no native-stack,
physical, hosted-acceptance or stable-publication claim. It closes no issue.
