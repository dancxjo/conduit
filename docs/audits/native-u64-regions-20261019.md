# Native U64 chain execution

The Std Host can prepare an optional pure U64 chain profile that keeps interior
values as native words. The authored Gears, logical Cords, per-Gear transactions,
and Source failure attribution remain in the existing execution kernel. This
extends the canonical chain profile described in [the fusion audit](pure-chain-fusion.md).

## Selected profile and bounds

`std/native-u64-chain@1` supports 2 through 32 local unary pure expressions with
exact U64 inputs and outputs, using Value ports or closing Flow ports. Its typed
expression evaluator supports U64 arithmetic and Bool conditions, with at most
128 prepared expression nodes. Checked and proven arithmetic, shift overflow,
lazy conditionals and arithmetic refusals follow the ordinary prepared evaluator.

The containing fusion Offer retains the exact installed computational Offers.
The Planner selects the physical alternative; installation validates the sealed
fragment, topology, execution region and available reservations before Play.
Required observation and authority boundaries prevent selection. Host Calls,
state, independent Fore ports, fan-out, remote Cords and isolation boundaries are
outside this profile. Canonical byte inspection prevents native binding; attaching
a debugger later returns an unsupported-profile refusal and requires preparation
of an inspectable realization.

The hybrid hosted store prepares a fixed native sidecar. Canonical backing stays
allocated, and native words consume the same logical item and byte quota as their
eight-byte canonical values. Disjoint regions share one reservation calculation,
including a staged result. Ownership generations, retired cells, release and
clear behavior remain checked. Native values cannot be read through the canonical
byte accessor or escape an interior chain Cord.

## Measured Std results

The measured source is
[`5feaf16031ed`](https://github.com/dancxjo/conduit/tree/5feaf16031ed29368f9f348b5cbf13c41b5651d2).
Each trial sends 64 values through an ordinary checked Source, Planner, sealed
Plan and Std runner, performing 32 additions per output. Three fresh processes
pinned to CPU 4 each ran three paired repetitions with rotated profile order.
The following elapsed-time ratios are medians of the three process medians;
a ratio below one means the native profile took less time than canonical fusion.

| Gears | Play ratio | Process median range | Full Std call ratio | Source and Plan phases plus Std call ratio |
| --- | --- | --- | --- | --- |
| 8 | 0.8404 | 0.8227–0.8441 | 0.9125 | 0.9874 |
| 32 | 0.9105 | 0.8877–0.9380 | 0.9268 | 0.9802 |

Play includes Std input and output adaptation. The full Std call also includes
Back preparation and report translation. The last column sums the recorded
Source/Plan phases and Std call; common fixture input construction, object
teardown, harness work and artifact writing are excluded. These are finite-batch
descriptive results, with nine paired observations per Gear count, rather than
confidence intervals or a universal profitability rule. The profile combines
native expression evaluation with native Cord transport, so the experiment does
not isolate their individual contributions.

Value-store backing increases from 136 to 408 bytes for 8 Gears and from 520 to
1,560 bytes for 32 Gears on this x86_64 ABI. Other Back storage and total process
memory are separate. Planner selection in this fixture uses disclosed frozen
cost assumptions; the timings do not install a production profitability policy.

The 32-Gear kernel differential fixture reduces canonical stores from 132 to 8
for four values while retaining ordered outputs and events. The Std fixture's
1,984 avoided interior canonical publications per 64-value batch are derived
from its selected profile and fixed graph, rather than a hardware memory-traffic
counter. Native expressions and the kernel storage/transport loop have
allocation-free conformance checks. The full Std Play interval includes delivery
ownership and adapter allocation costs recorded separately in the raw rows.

## Retained evidence and refusals

All profiles preserve complete canonical outputs, logical invocation counts and
ordered remote, close and completion identities. Canonical and native fusion
also match every retained kernel event field, including sequence numbers. Std
reports retain bounded transient history; these comparisons do not establish a
full lifetime transient trace. Unfused dispatch has different transient ordering
and sequence positions, already visible in the earlier canonical fusion baseline.

The accepted independent group contains 72 profile rows and 18 native/canonical
pairs, comparing 6,660 retained fused records exactly. The supported public
command first passed all 28 optimized conformance tests. Tests cover each of the
32 original failure points, every logical cancellation checkpoint, queue pressure,
Sign exhaustion, malformed programs, unsupported types, debug fences, stale
references, and allocation-free native storage. The evaluator's differential
suite covers 2,467 inputs against the ordinary prepared evaluator.

[The evidence folder](evidence/native-u64-regions-20261019) retains the public
capture and three accepted timing captures, original manifests, verifiers and
sampling exclusions. Every archived member was checked before duplicate raw
files were reclaimed, and the accepted family passed semantic verification after
extraction to a new location. Fifteen mutations were refused after their artifact
hashes were refreshed. Earlier cleanup-overlapping trials and a filesystem-full
attempt are excluded from the timing summary; those exclusions were recorded
before inspecting ratios.

The supported entrance is:

```sh
cargo xtask prove runtime-performance --hosted-pipeline true --values 1000000 \
  --repetitions 3 --decisions 1000 --samples 1000 --output NEW_DIRECTORY
```

Extract the public archive beside an accepted final archive, then run this
command from the evidence folder, replacing `N` with 1, 2 or 3:

```sh
python3 verify-native-chain.py EXTRACTED/native-chain-final-N \
  --source 5feaf16031ed29368f9f348b5cbf13c41b5651d2
```

Archive manifests bind all original files. The verifier checks Source and Plan
identity, full Fore outputs, retained events, capacity charges, phase accounting
and completeness.

## Integration scope

The review branch contains the native storage, typed evaluator, kernel transport
and Std installation. Its measured ancestor also contains prepared-session and
FARGAN work held for separate review. The native-owned source bridge records the
small differences introduced by current-dev delivery consumers and removal of the
unused session module declaration. Measured-source timings remain scoped to that
reference until refreshed-source measurements exist. Validation used dev base
`d231024bb7cf41fad7f2cde8b18cca67a536c038`. The isolated kernel and Plot suites passed 1,071 tests with zero failures and seven manual
tests ignored. Workspace-wide Clippy passed with warnings denied across all
targets, including the hosted performance feature. The refreshed hosted runtime
suite passed all 28 tests, and kernel Clippy without default features passed.
Later dev changes require canonical candidate integration against their merge preview.

This profile uses typed prepared operation trees and ordinary logical Steps.
A compact program across a real FARGAN numerical segment and ordinary Body
session adoption remain separate work. Availability proof here is Std; this
change installs no new browser or no_std Host Back. Stable acceptance requires
combined dev integration and the automated release train.
