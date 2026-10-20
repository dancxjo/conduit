# Pinned FARGAN model provenance — preparation

This record prepares #5711 on Conduit base
`3ddae3bb9d41a466e446a55464348dc043eb6ed4`. It contains metadata and hashes,
**no model parameters**. It does not resolve redistribution rights, admit a
product model, rerun numerical proof or close #5711/#4898/#5726.
[Machine-readable evidence](model-provenance.json) distinguishes verified bytes,
upstream documentation, existing implementation profiles and unknowns.

## Official origin and exact artifacts

Upstream is [xiph/opus at 503d81b138d76621aae4b12786e90de48aa8db3a](https://github.com/xiph/opus/tree/503d81b138d76621aae4b12786e90de48aa8db3a).
Its pinned [autogen script](https://github.com/xiph/opus/blob/503d81b138d76621aae4b12786e90de48aa8db3a/autogen.sh)
passes archive digest `a5177ec6fb7d15058e99e57029746100121f68e4890b1467d4094aa336b6013e`
to the [model downloader](https://github.com/xiph/opus/blob/503d81b138d76621aae4b12786e90de48aa8db3a/dnn/download_model.sh).
That script names the official checksum-addressed archive, verifies its checksum
when a SHA-256 tool is present, and extracts it. This preparation downloaded
nothing and executed neither downloader nor model.

The existing local archive was read and independently hashed: **134,674,421
bytes**, matching that exact digest. Its `dnn/fargan_data.c` matches the generated
source pin `68ba6e8731007f7184be862d7d1afe4cef096a7c9bcbfa115962340f725e004c`.
Its checkpoint is `dnn/models/fargan_sq1Ab_adv_50.pth`, **13,775,086 bytes**,
SHA-256 `2eb56f36f71f246174d769658b5712fa81214650bd425cf050c740c8d6b4464c`.
The archive member and existing local checkpoint are byte-identical; no PyTorch
checkpoint loading or pickle execution was performed.

The [official FARGAN README](https://github.com/xiph/opus/blob/503d81b138d76621aae4b12786e90de48aa8db3a/dnn/torch/fargan/README.md)
identifies the pretrained C data and corresponding checkpoint distribution.
Do not replace that selected checkpoint with another similarly named model.

## Export and authored topology

The existing standard-library-only development exporter verifies the generated
C digest and explicit array lengths before serializing little-endian profiles.
It validates every resulting profile digest before writing parameter data.
The inspected manifest has 82 array entries across its declared profiles.
The selected F32 bundle is **3,272,868 bytes** (818,217 F32 elements), SHA-256
`d35f0510da476716183a33caafe45cc095e48d496f06de2db7655e780a385e47`.
An existing independent reproduction copy has the same hash. The exporter was
not rerun here. The compact profile is distinct; its existence does not prove
compact whole-network parity.

Network order belongs to checked `fargan_conditioning`, `fargan_signal`,
`fargan_pitch_history`, `fargan_subframe` and `fargan_frame` Source. Their exact
hashes, exporter and oracle hashes are retained in the JSON. The finite profile
uses 20 feature values, 128 F32 conditioning-history values, 837 F32 active
signal-state values, pitch periods 32–255 samples, four 40-sample subframes and
160 samples per 16-kHz frame. Selected precision is
`number/ieee754-f32-libm-reference@1`, numerical contract
`conduit.numeric/fixed-f32-libm@3`. These are explicit hosted full-float graph
contracts, not universal acoustic semantics or a persistent speech stream.

## License evidence and unresolved scope

The pinned [COPYING](https://github.com/xiph/opus/blob/503d81b138d76621aae4b12786e90de48aa8db3a/COPYING)
and [contributor statement](https://github.com/xiph/opus/blob/503d81b138d76621aae4b12786e90de48aa8db3a/LICENSE_PLEASE_READ.txt)
provide software/contribution licensing evidence. The official
[implementation license page](https://opus-codec.org/license/) describes the
three-clause BSD implementation terms. Source and binary redistribution retain
required attribution, conditions and disclaimer; contributor names cannot imply
endorsement. Patent statements remain separate.

The inspected model archive has **no LICENSE/COPYING/README members**. Its
generated FARGAN C file has no explicit license header. No separate checkpoint
license or exclusion was located in the inspected primary sources. Official
upstream distribution of these pretrained implementation files is positive
provenance evidence; absence of an archive-specific statement is **not evidence
that redistribution is forbidden**. Nor does this record assert that a separate
checkpoint/tensor-bundle grant has been conclusively documented.

Before publicly redistributing the selected parameters, record the reviewed
scope of the upstream terms for that checkpoint/generated data/derived bundle,
with required notices. If documentation does not settle it, obtain clarification
from the upstream owner rather than invent a license or silently select another
model. The existing manifest's private-development restriction is preserved;
this preparation neither strengthens it into a prohibition nor removes it.
Exact training-corpus composition, rights and speaker provenance are not proven
by the README's example training-data link. No new model training is required
for the first runtime acceptance, but provenance claims must remain bounded.

## Oracle prerequisites and exact development command

The known local fixture is in the prior #4907 preparation workspace, under
`2026-10-06/goal-please-finish-and-close-4907/work/fargan`. Its `oracle/` source
archive and `upstream/dnn/` generated C/header match **all twelve** pinned
source/model checks in `build_full_frame_oracle.sh`, including duplicate
source/model checks. It has no `.git` metadata; file hashes, not an asserted
checkout HEAD, establish the script's source prerequisites.

After the owner schedules a build slot, the development-only operations are:

```sh
sh proof/fargan/development-reference/build_full_frame_oracle.sh \
  "$FARGAN_FIXTURE/oracle" "$FARGAN_FIXTURE/upstream/dnn" \
  "$FARGAN_FIXTURE/full-frame-oracle"
"$FARGAN_FIXTURE/full-frame-oracle" \
  "$FARGAN_FIXTURE/conditions.f32le" \
  "$FARGAN_FIXTURE/full-frame-oracle.bin"
```

`FARGAN_FIXTURE` denotes that existing local directory; it is not a credential or
an automatic download. The script requires a C99 compiler, standard headers,
libm, realpath and sha256sum. It selects `-O2` full float with contraction, SIMD
and tree vectorization disabled; do not add `DISABLE_DEBUG_FLOAT`. The oracle
is independent development C and must never be linked into product Play.

For the existing 24-frame synthetic feature fixture, the expected output is
**113,880 bytes**, SHA-256
`9c676f6c76aec71281ea097fe0c477c5a3b77f5022e639a0d82dbb4a5437fcc3`.
That output was absent from the inspected fixture. Compiler wall time, CPU,
peak memory and oracle execution cost are **unmeasured here**; no numerical
estimate is substituted. The generated C input alone is 21,978,624 bytes.
Existing conditioning/subframe traces are present, but are not substitutes for
this full-frame oracle. Its synthetic warm-state trajectory is numerical proof,
not language conditioning, intelligibility or voice quality.

Repository performance validation uses the supported `cargo xtask prove
runtime-performance --fargan-fixture ...` entrance described in the existing
README. This preparation runs neither that entrance nor the development tools.
Positive same-plan conditioning, persistent streaming PCM, ConduitOS execution,
resource evidence and attended comparison remain the parent obligations.
