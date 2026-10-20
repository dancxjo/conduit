# Held experiment: exact normalized score-margin retention

This unbuilt standalone Source/test preparation is NOT imported by Language
build.rs, catalogs, lib.rs, decoder or xtask. Accepted v2 and frozen v3 bytes,
profiles and prior negative evidence remain unchanged. There is NO calibrated
production margin, selected survivor policy or success claim.

For each active hypothesis i, q_i=s_i/c_i, where scored decision count c_i is1..8
and s_i is in[-1000000,1000000]. An explicit profile supplies rational M=m/d,
0<=m<=2000000 and1<=d<=64. Retain active i exactly when every active j satisfies

    (s_j*c_i - s_i*c_j)*d <= m*c_i*c_j.

This compares q_best-q_i to M without division or floats. It preserves every tie
and at least one active maximizer whenever input admission succeeds. It has no
forced top1, grammar/POS/name rule, confidence-as-truth claim or graph rewrite.
The mask changes only experimental survival; later actual all-survivor graph
agreement/frontier/commit law would remain unchanged, if this were accepted.

Each |s*c|<=8000000; each difference<=16000000; its product by d is bounded by
1024000000. Right side m*c_i*c_j<=128000000. All fit signed64 exactly. Native
admission rejects out-of-profile score/count/margin arithmetic before expression
evaluation. Active count0, all inactive, absent/unknown profile identity, absent
calibration or confirmation identity return acceptedfalse/allfalse. Zero-count
inactive slots do not participate. The Source input accepts bounded identity
strings solely as named experimental provenance, NOT authentication. Any future
production wrapper must admit a pinned calibrated descriptor/resource matching
these strings and the margin, or refuse. Arbitrary nonempty caller strings must
never become installed authority.

Decision counts remain a required future custody seam: bind every counted model
invocation and accepted scored transition to actual plan receipts, prior
hypothesis identity, exact current text revision and model/profile identity.
Reset count and score at checked fresh-revision rebase. Count cannot be supplied
as an unverified confidence denominator, and no caller-current flag is accepted.
The current experiment only tests math on synthetic declared tuples, including
different scored derivation lengths; it does not validate their execution.

`tests/experimental_score_retention.rs` prepares this standalone checked
expression once per synthetic test, directly evaluates it, and admits Native
input contracts. Cases cover equal scores, distinct lengths with equal rational
means, inactive largest score, inclusive margins, signed extreme valid bounds,
empty beam, active missing count, foreign/missing profile/calibration, overflow
score, bad count and zero denominator. It neither builds a full ProtocolSource
package nor runs a model. Tests are prepared but NOT compiled/run. Root may
coordinate a future focused test after deciding this held experiment's scope.

## Predeclared independent calibration/confirmation protocol

No dataset output or threshold has been selected. Before inference, allocate
fresh eligible rows and pin exact manifests with corpus/license/version/source
hashes, row IDs, document/template family IDs and partition hashes. Exclude all
existing actual model training rows/teaching expansions, all previously observed
DEV/TEST rows, ALL motivating/reserved families (including Hello/Travis and name/
punctuation/template variants), and any semantic near-duplicates. The existing
TRAIN pool may have insufficient untouched supported rows; if so stop and collect
independent licensed data rather than reuse trained examples as calibration.

Group by document+authored-template family; use SHA256 of pinned corpus digest
and group identity with protocol salt `normalized-retention-calibration@1`.
Modulo5 bucket0 is calibration, bucket1 is confirmation, buckets2..4 remain
unobserved reserve. Publish/pin membership before seeing model outputs. Rows
must exercise punctuation, names, syntactic alternatives, non-vocatives,
lexical ambiguity and adversarial prefixes within the four-token profile.

Freeze model/feature ABI, Source legality/rank/consensus, fresh-revision scoring,
count custody and normalization formula before evaluation. A predeclared finite
candidate margin grid enumerates reduced rational m/d within the representable
bounds on an explicitly documented grid; the exact grid and precision/coverage
acceptance criteria must be pinned in that manifest before any calibration
output is observed. This document deliberately supplies NO selected number or
numeric default. Selection on calibration must optimize declared false-commit
risk/coverage/delay criteria, with denominators and uncertainty; independent
confirmation then assesses the selected profile once. Insufficient coverage,
false commits or inconsistent correction behavior retains a failed profile.
Do not tune on confirmation or the now-observed motivating/reserved cases.

Record full beams, counts, normalization fractions, original scores, masks,
actual committed edges, source/model/profile identities and correction events.
Any later known-example evaluation is explicitly observed-case regression,
never fresh calibration. Adoption would require a separately reviewed Source
profile+custody wrapper and stable contract checkpoint; this preparation grants
none of that and erases no frozen v3 negative result.
