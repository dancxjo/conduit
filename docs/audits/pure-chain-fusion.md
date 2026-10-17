# Pure-chain physical dispatch development proof

For #5397, the installed std Host can execute one admitted chain of 2–32 pure
U64 Gears through the ordinary selected Plan, lowering, Back preparation and
production kernel. The unchanged checked logical graph remains authoritative.
This is a development implementation and measurement record, not a stable
release acceptance claim.

The existing 1/8/32-Gear fixture performs 32 checked additions per input over 64
ordered inputs. The original hosted Host Call baseline remains in every capture.
The comparison isolating fusion uses the same installed local computational Back
with and without the selected physical region. Changing the hosted Back to the
local Back is not counted as a fusion saving. The singleton selects no region.

## Measured result

| Capture | Full call ms, unfused / fused | Process CPU ms, unfused / fused | Completion outputs/s, unfused / fused |
| --- | ---: | ---: | ---: |
| Local 02 | 7.063 / 6.858 | 7.065 / 6.860 | 11285 / 12498 |
| Local 03 | 7.043 / 6.858 | 7.045 / 6.860 | 11360 / 12560 |
| Local 04 | 7.035 / 6.873 | 7.036 / 6.867 | 11241 / 12601 |
| CI | 5.701 / 5.394 | 5.705 / 5.397 | 14543 / 16734 |

| Capture | Measured cold + call ms, unfused / fused | First output from call start ms, unfused / fused |
| --- | ---: | ---: |
| Local 02 | 17.335 / 18.468 | 1.272 / 1.625 |
| Local 03 | 17.242 / 18.388 | 1.270 / 1.658 |
| Local 04 | 17.303 / 18.367 | 1.233 / 1.645 |
| CI | 12.871 / 13.471 | 1.142 / 1.402 |

Across the three local process medians, full-call wall time is 2.3–2.9% lower
and completion-window throughput is 10.6–12.1% higher at depth 32. Measured cold
phases plus the call are 6.2–6.6% slower; first output from call start is also
slower. The dispatch saving does not establish a total cold-program speedup.

Each row is the median of three matched trials in one process, alternating
unfused/fused trial order. The three local captures are independent processes on
the same shared Linux host, externally pinned to CPU 19. Frequency and background
load are uncontrolled. The CI capture is a separate Linux runner and has its own
source identity and CPU metadata. These are descriptive measurements, not a
confidence interval or a calibrated regression threshold.

The deterministic victory is fewer physical ready selections. At depth 32,
4,160 decisions become 194 (95.34% fewer), while actual logical invocations remain
4,160. Those invocations include empty polls and terminal work; useful expression
evaluations remain 2,048. At depth eight, decisions fall from 1,040 to 130 while
logical invocations remain 1,040. The singleton remains 130/130 with no region.

Cold preparation and first output are retained, not amortized away. The fused
selection and preparation cost can exceed the dispatch saving in this short
64-value program. The table includes the sum of measured cold phases and the
full call; it excludes harness artifact serialization between those phases.
Completion-window throughput covers the 63 intervals between the first and last
acknowledged outputs, including ordinary Host and collector work.

All 1,984 intermediate canonical values and 15,872 intermediate bytes at depth
32 remain materialized. These are fixed-fixture work counts, not hardware copy
counters; the internal-copy counter is explicitly unavailable. The value arena
remains 65 slots / 520 bytes. Full-call allocations rise from 9,646 to 13,804
and reallocations from 419 to 562 because preparation revalidates the admitted
region. Post-first-output Host/collector observations remain 378 allocations,
zero reallocations and 2,898 requested bytes. The separate admitted kernel Play
probe observes zero allocations/reallocations; the entire Host call is not
claimed to allocate nothing.

## Exact semantics and admission

Both selected Plans retain all 32 Gear identities, their checked semantic
contracts, configuration, typed input/output ports, authored source spans and all
31 typed Cords with their temporal, capacity and pressure settings. Observatory
retains the physical fusion separately from the logical rows. The actual frozen
32-Gear source is projected through Observatory and Patchbay within the existing
finite topology bound; it remains inspectable and serializable.

The kernel binds a disjoint finite successor table before Play. A region advances
at most 32 logical Steps per physical dispatch. It uses each original prepared
driver, input context, fuel, reservation, atomic commit, bounded Cord queue and
Sign path. Output pressure, Yield, failure or an unavailable successor stops
advancement. Cancellation and debugger suspension remain checkpoints before each
logical Gear. All required per-Gear and output Signs remain attributable.

The std Offer rederives checked purity and numeric eligibility for each exact
current-Boot installed capability. Preparation repeats validation rather than
trusting a profile label. The reviewed envelope is U64-to-U64 Value or closing
Flow, with no independent Fore inside the chain. Host Calls, state, effects,
authority, clock/observation, independent asynchronous boundaries and unsupported
numeric representations cannot be smuggled into this profile. Existing Planner
eligibility and boundary checks remain authoritative; isolated/preempted regions
and nonlocal Cords refuse at preparation. Fan-out and merge chains are refused.
Browser/WASM explicitly refuses this physical profile; its build passing is not
browser execution parity.

The immutable Planner alternatives use disclosed fixture work assumptions:
32 useful additions plus one assumed dispatch per Gear versus one per region,
with zero transport savings. These are work units, not calibrated provider
nanoseconds. Raw selection evidence retains both candidates. Expensive fusion
and required observation boundaries retain the unfused alternative; there is no
universal automatic-fusion profitability claim or weakening of precision.

The bounded kernel tests cover zero/one/many inputs, pressure with delayed output
consumption, per-Gear value/completion evidence, every one of 32 failure points,
every cancellation and debug checkpoint, and invalid/overlapping/late binding.
The ordinary std Plan/Play tests compare native, hosted, local and fused ordered
outputs and mandatory Signs, reject malformed and overflow input, and force an
arithmetic failure at every original Gear. Fused errors name the exact Gear and
source span from retained failure evidence. Existing unfused refusal messages
remain unchanged, including JSON operations and Scalar tape parity.

## Reproduction and custody

Use a clean checkout of the recorded source and a new directory for every run:

```sh
taskset -c 19 cargo xtask prove runtime-performance \
  --output /tmp/conduit-fusion-01 --values 1000000 --repetitions 3 \
  --decisions 1000 --samples 1000 --hosted-pipeline true
python3 proof/performance/verify-pure-chain-fusion.py /tmp/conduit-fusion-01
```

The supported entrance runs all 16 optimized conformance tests before measuring,
checks clean source custody before and after, and hashes the original retained
artifacts. The read-only verifier checks matched source/checked identities,
logical Plan equality, output digest, required output Signs, fixed value capacity,
work accounting and the actual reduction in decisions. The retained archives
contain the complete original raw records, Host/Plan/Play receipts, metadata,
conformance and status manifests. Sidecar comparisons do not replace originals.

Raw captures: [local 02](evidence/pure-chain-fusion-02-raw.tar.gz),
[local 03](evidence/pure-chain-fusion-03-raw.tar.gz),
[local 04](evidence/pure-chain-fusion-04-raw.tar.gz),
[CI](evidence/pure-chain-fusion-ci-raw.tar.gz).

The compact [receipt](evidence/pure-chain-fusion.json) includes archive SHA-256,
source identities, all three local captures, the independent CI capture and
additional cold/first-output/allocation costs. All original artifact hashes were
verified before archiving. Local captures use corrected source
`f280ac16754222505e040d77bbc2906ce2f3afc1`. The earlier CI merge preview is
`b506d3571de2ae3de1841a22b360beca68ee12ea`; the later correction preserves existing
unfused error messages and does not change successful execution. An interrupted
earlier local worker is retained outside these successful captures and is not
performance evidence.

## Validation boundary

Refreshed kernel/planner/Observatory/runtime regressions passed 573 tests across
107 targets, with five ignored, and the separate 32-Gear Patchbay test passed.
After the refusal-surface correction the complete default std Host library
passed 799 tests with 17 ignored, and the runtime suite passed all 16 tests.
Strict workspace Clippy and the ordinary integration entrance, including WASM,
passed during implementation. Final candidate admission and full dev integration
remain the normal merge/publication gates; exact accepted-main evidence is
required before closing the owning issue. See [PR #5947](https://github.com/dancxjo/conduit/pull/5947)
for those separately identified integration results.

## Freestanding integration footprint

Exhaustive dev integration exposed an additional target constraint: compiling the
unused region traversal into the independently linked no-alloc x86_64 domain
pushed its immutable image 232 bytes beyond the unchanged 128 KiB window. The
reviewed physical profile is installed std only. No-alloc kernels now explicitly
refuse binding and compile their ordinary single-Step dispatch path; std keeps
the existing region path and its allocation-free Play behavior.

Under the exact production private-domain compiler flags, code shrinks from
57,919 to 56,959 bytes, while constants remain 69,864 bytes. The constants move
one page earlier, taking the mapped span from 135,168 to 131,072 bytes. The actual
unchanged `DomainImage::parse` admits the corrected ELF within its original
131,072-byte limit. No mapping, memory, or executable-image allowance is widened.
The [footprint receipt](evidence/pure-chain-fusion-private-image.json) records the
profile and exact kernel/validator hashes. A no-alloc regression checks explicit
refusal and ordinary per-Gear completion; the five allocated region regressions
continue to cover pressure, failure, cancellation and zero allocation during Play.
