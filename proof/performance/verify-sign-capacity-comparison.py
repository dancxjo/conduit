#!/usr/bin/env python3
"""Verify retained Sign reservation captures without requiring old build paths."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import statistics


def verify(directory):
    hashes = json.loads((directory / 'sha256.json').read_text())
    assert len(hashes) == 13
    assert set(hashes) == {p.name for p in directory.iterdir()
                           if p.is_file() and p.name != 'sha256.json'}
    for name, expected in hashes.items():
        assert hashlib.sha256((directory / name).read_bytes()).hexdigest() == expected, name
    metadata = json.loads((directory / 'metadata.json').read_text())
    observations = json.loads((directory / 'observations.json').read_text())
    rows = observations['rows']
    groups = [(sink, capacity, state) for sink, capacities in
              [('fixed', [1, 8, 128, 4096]), ('hosted', [1, 8, 128, 4096, 60000])]
              for capacity in capacities for state in
              ['Empty', 'Sparse', 'HalfTransient', 'FullTransient', 'FullSticky']]
    keys = {(process, sink, capacity, state, trial) for process in range(3)
            for sink, capacity, state in groups for trial in range(8)}
    assert len(rows) == 1080
    assert {(r['process'], r['sink'], r['capacity'], r['state'], r['trial']) for r in rows} == keys
    for process in range(3):
        raw = [{'process': process, **json.loads(line.split('SIGN_CAPACITY_COST_ROW ', 1)[1])}
               for line in (directory / f'process-{process}.stdout.log').read_text().splitlines()
               if 'SIGN_CAPACITY_COST_ROW ' in line]
        assert raw == [r for r in rows if r['process'] == process]
        assert 'Exit status: 0' in (directory / f'process-{process}.resource.log').read_text()
    for row in rows:
        occupied = {'Empty': 0, 'Sparse': 1, 'HalfTransient': row['capacity'] // 2,
                    'FullTransient': row['capacity'], 'FullSticky': row['capacity']}[row['state']]
        accepted = row['state'] != 'FullSticky' and not (row['state'] == 'Sparse' and row['capacity'] == 1)
        assert row['occupied'] == occupied and row['accepted'] == accepted and row['calls'] == 256
        assert isinstance(row['elapsed_ns'], int) and row['elapsed_ns'] >= 0
    summaries = observations['summaries']
    assert len(summaries) == len(groups)
    assert {(s['sink'], s['capacity'], s['state']) for s in summaries} == set(groups)
    for summary in summaries:
        means = [statistics.mean(r['elapsed_ns'] for r in rows
                                 if (r['sink'], r['capacity'], r['state'], r['process']) ==
                                 (summary['sink'], summary['capacity'], summary['state'], process))
                 for process in range(3)]
        assert means == summary['process_mean_batch_ns']
        assert math.isclose(statistics.mean(means), summary['mean_batch_ns'], rel_tol=1e-12)
        assert math.isclose(statistics.stdev(means), summary['process_mean_sample_sd_ns'], rel_tol=1e-12)
    profiles = metadata['profile']
    assert profiles['CARGO_PROFILE_TEST_OPT_LEVEL'] == '1'
    assert profiles['CARGO_PROFILE_TEST_DEBUG_ASSERTIONS'] == 'true'
    assert profiles['CARGO_PROFILE_TEST_OVERFLOW_CHECKS'] == 'true'
    artifacts = [json.loads(line) for line in (directory / 'build.jsonl').read_text().splitlines()
                 if line.startswith('{')]
    libraries = [a for a in artifacts if a.get('target', {}).get('name') == 'conduit_kernel'
                 and a.get('target', {}).get('kind') == ['lib']]
    assert libraries and all(not a['profile']['test'] for a in libraries)
    return rows, summaries, {'source': metadata['source'],
                            'executable_sha256': metadata['executable_sha256'],
                            'hashes': hashes, 'profile': profiles,
                            'raw_rows_verified': len(rows), 'summaries_recomputed': len(summaries)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before', required=True, type=Path)
    parser.add_argument('--after', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    before, before_summaries, before_proof = verify(args.before)
    after, after_summaries, after_proof = verify(args.after)
    assert [{k: v for k, v in r.items() if k != 'elapsed_ns'} for r in before] == [
        {k: v for k, v in r.items() if k != 'elapsed_ns'} for r in after]
    assert before_proof['source'] != after_proof['source']
    assert before_proof['executable_sha256'] != after_proof['executable_sha256']
    comparison = []
    for old, new in zip(before_summaries, after_summaries):
        assert (old['sink'], old['capacity'], old['state']) == (new['sink'], new['capacity'], new['state'])
        comparison.append({'sink': old['sink'], 'capacity': old['capacity'], 'state': old['state'],
                           'calls_per_batch': 256,
                           'before_mean_ns': old['mean_batch_ns'], 'after_mean_ns': new['mean_batch_ns'],
                           'before_process_mean_sd_ns': old['process_mean_sample_sd_ns'],
                           'after_process_mean_sd_ns': new['process_mean_sample_sd_ns']})
    proof = {'before': before_proof, 'after': after_proof,
             'exact_non_timing_rows_equal': True, 'comparison': comparison,
             'limits': ['Native production SignSink reservation batches; no scheduler CPU attribution.',
                        'Loop, black-box and result storage included; no timer subtraction.',
                        'Shared host; process variation retained; no automatic timing threshold.',
                        'No claim that every occupancy/capacity improves; inspect all comparisons.',
                        'Historical binary paths are not required; source/executable hashes are capture-time custody.']}
    with args.output.open('x') as output:
        json.dump(proof, output, indent=2)
        output.write('\n')
    print(args.output)


if __name__ == '__main__':
    main()
