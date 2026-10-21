"""Format an already complete replacement diagnostic; never run or tune a model."""
import json
import sys
from pathlib import Path

receipt = json.loads(Path(sys.argv[1]).read_text())
completion = json.loads(Path(sys.argv[2]).read_text())
assert completion['returncode'] == 0, 'worker did not complete successfully'
assert receipt['verification']['complete_case_arm_reports'] == 34
assert receipt['original29_reused_in_aggregate'] is False
assert receipt['manifest_sha256'] == completion['manifest_sha256']
assert receipt['classification'] == 'UNACCEPTED_NONBLIND_REPLACEMENT_DIAGNOSTIC_NOT_INSTALLED_ACCEPTANCE'
assert all(len(receipt['arms'][a]['cases']) == 17 for a in ['causal', 'full'])

lines = [
    '# Frozen parser comparison', '',
    'Complete, finite, nonblind replacement diagnostic: 17 frozen histories in each arm. '
    'The original 29-report run and failed 28-report replacement are excluded. '
    'Original source-pin equivalence remains unestablished. No model, default or installed Current acceptance is claimed.', '',
    '| Observation | Causal arm | Full arm |', '| --- | ---: | ---: |',
]
fields = [
    ('Tokens', 'tokens'), ('Correct unlabeled attachments', 'uas_correct'),
    ('Correct labeled attachments', 'las_correct'), ('Vocative true positives', 'vocative_tp'),
    ('Vocative false positives', 'vocative_fp'), ('Vocative false negatives', 'vocative_fn'),
    ('Cases with prefinal commitment', 'nonfinal_committing_cases'),
    ('Accepted commitment observations', 'accepted_commits'),
    ('Observed changes after commitment', 'changes_after_commit'),
    ('Winner edge revisions', 'winner_edge_revisions'),
    ('Numerical model invocations', 'model_invocations'),
    ('Illegal candidates refused', 'illegal_candidates_refused'),
]
for label, key in fields:
    lines.append(f"| {label} | {receipt['arms']['causal']['totals'][key]} | {receipt['arms']['full']['totals'][key]} |")
lines += ['', '## Paired history outcomes', '',
          '| History | Causal UAS/LAS | Full UAS/LAS | Causal vocative TP/FP/FN | Full vocative TP/FP/FN | Causal/full prefinal commitment |',
          '| --- | --- | --- | --- | --- | --- |']
for left, right in zip(receipt['arms']['causal']['cases'], receipt['arms']['full']['cases']):
    assert left['id'] == right['id'] and left['tokens'] == right['tokens']
    attachments = lambda r: f"{r['uas_correct']}/{r['tokens']}; {r['las_correct']}/{r['tokens']}"
    vocative = lambda r: '/'.join(str(r[k]) for k in ['vocative_tp', 'vocative_fp', 'vocative_fn'])
    lines.append(f"| {left['id']} | {attachments(left)} | {attachments(right)} | {vocative(left)} | {vocative(right)} | {left['nonfinal_commit']}/{right['nonfinal_commit']} |")
lines += ['', '## Time and resource boundary', '',
          f"The whole worker recorded {completion['wall_seconds']:.3f} seconds wall time, "
          f"{completion['child_user_seconds']:.3f} seconds child user CPU and "
          f"{completion['child_system_seconds']:.3f} seconds child system CPU. "
          f"Maximum child RSS was {completion['child_max_rss_kib']} KiB. "
          'These are whole-process observations on this machine, not per-arm portable resource guarantees.', '',
          'The retained canonical Source observations and preparation checks add diagnostic work. '
          'No real-time or broad accuracy claim follows from these timings or this previously observed finite case set.', '',
          '## Next consumer', '',
          '[#5755](https://github.com/dancxjo/conduit/issues/5755) remains the next selected consumer: '
          'compact realization from supplied committed vocative structure, reusing the accepted lexical/IPA '
          'projection and common prosody carrier. This record starts no consumer implementation and selects no learned default.', '',
          'The machine-readable comparison retains every paired case, report digest and Source trace inventory. '
          'Publication and dependent runtime acceptance remain separate.', '']
Path(sys.argv[3]).write_text('\n'.join(lines))
