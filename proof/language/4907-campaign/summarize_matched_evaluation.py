#!/usr/bin/env python3
"""Read-only complete-cohort report; refuses partial evaluation aggregation."""
from pathlib import Path
import hashlib
import json
import sys
root = Path(sys.argv[1]).resolve()
manifest_path = Path(__file__).parent / 'causal-candidate-evaluation-prep/pair-manifest-v5.json'
assert hashlib.sha256(manifest_path.read_bytes()).hexdigest() == '25a4f7e8b41febbe1067fbdd10ae515a634a81c9fcb4840a66f77b68dcca26a5'
manifest = json.loads(manifest_path.read_text())
cases_bytes = Path(manifest['cases']['path']).read_bytes()
assert hashlib.sha256(cases_bytes).hexdigest() == manifest['cases']['sha256']
expected_ids = [case['id'] for case in json.loads(cases_bytes)['cases']]
assert len(expected_ids) == 17 and len(set(expected_ids)) == 17
expected = {f'{arm}-{n:02}-report.json' for arm in ('causal', 'full') for n in range(17)}
actual = {p.name for p in root.glob('*-report.json')}
if actual != expected:
    raise SystemExit(f'Incomplete report set: {len(actual)}/34; missing={sorted(expected-actual)} extra={sorted(actual-expected)}')
summary = {'scope': 'Frozen matched nonblind diagnostic cohort only; no default/model-quality/installed-Current acceptance', 'reports': {}, 'arms': {}}
for arm in ('causal', 'full'):
    rows = []
    for n in range(17):
        path = root / f'{arm}-{n:02}-report.json'
        report = json.loads(path.read_text())
        assert report['arm'] == arm and report['case_ordinal'] == n
        assert report['status'] == 'CASE_COMPLETED'
        assert report['identity']['sha256'] == manifest['arms'][arm]['sha256']
        case = report['case']
        assert case['id'] == expected_ids[n]
        row = {k: case[k] for k in ('id', 'tokens', 'uas_correct', 'las_correct', 'vocative_tp', 'vocative_fp', 'vocative_fn', 'nonfinal_commit', 'first_commit_step')}
        row['changes_after_commit'] = sum(s['changes_after_commit'] for s in case['stability'])
        row['winner_edge_revisions'] = sum(s['winner_edge_revisions'] for s in case['stability'])
        row['model_invocations'] = report['model_invocations']
        row['accepted_commits'] = sum(bool(o['commit']['accepted']) for o in case['observations'])
        row['prefix_count'] = len(case['observations'])
        row['observed_prefix_elapsed_ns'] = sum(o['elapsed_ns'] for o in case['observations'])
        row['max_observed_prefix_elapsed_ns'] = max(o['elapsed_ns'] for o in case['observations'])
        rows.append(row)
        summary['reports'][path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
    total = {k: sum(r[k] for r in rows) for k in ('tokens','uas_correct','las_correct','vocative_tp','vocative_fp','vocative_fn','changes_after_commit','winner_edge_revisions','accepted_commits','prefix_count','observed_prefix_elapsed_ns')}
    total['nonfinal_committing_cases'] = sum(r['nonfinal_commit'] for r in rows)
    total['cases'] = rows
    summary['arms'][arm] = total
assert [r['id'] for r in summary['arms']['causal']['cases']] == [r['id'] for r in summary['arms']['full']['cases']]
print(json.dumps(summary, indent=2))
