#!/usr/bin/env python3
"""Verify retained completion-prefix observations from the production kernel."""
import argparse
import hashlib
import json
import statistics
from pathlib import Path


def read(root):
    status = json.loads((root / 'status.json').read_text())
    assert status['success'] is True
    expected_artifacts = {'build.jsonl', 'build.log', 'report.json'} | {
        f'process-{process}.{kind}.log' for process in range(3)
        for kind in ['stdout', 'stderr', 'resource']}
    assert set(status['artifacts_sha256']) == expected_artifacts, 'missing/foreign artifact'
    for name, expected in status['artifacts_sha256'].items():
        path = (root / name).resolve()
        assert path.parent == root.resolve(), 'foreign artifact path'
        assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, name
    report = json.loads((root / 'report.json').read_text())
    assert report['source'] == status['source']
    artifacts = [json.loads(line) for line in (root / 'build.jsonl').read_text().splitlines()
                 if line.startswith('{') and json.loads(line).get('executable')]
    assert len(artifacts) == 1 and artifacts[0]['fresh'] is False
    assert report['profile'] == artifacts[0]['profile'], 'build/profile mismatch'
    assert report['optimized'] is True and report['selector'] == 'packed'
    expected = {(process, nodes, trial) for process in range(3)
                for nodes in [1, 8, 32, 256] for trial in range(8)}
    rows = report['rows']
    assert len(rows) == len(expected)
    keyed = {(row['process'], row['nodes'], row['trial']): row for row in rows}
    assert set(keyed) == expected, 'missing/duplicate/foreign sample'
    for process in range(3):
        text = (root / f'process-{process}.stdout.log').read_text()
        assert 'test result: ok. 1 passed; 0 failed' in text
        observed = [json.loads(line.split('TERMINAL_COMPLETION_ROW ', 1)[1])
                    for line in text.splitlines() if 'TERMINAL_COMPLETION_ROW ' in line]
        assert len(observed) == 32
        assert [{'process': process, **row} for row in observed] == [
            row for row in rows if row['process'] == process], 'raw/report mismatch'
    return report, keyed, len(status['artifacts_sha256'])


def compare(before, after):
    old, left, left_hashes = read(before)
    new, right, right_hashes = read(after)
    assert old['source'] != new['source']
    assert old['executable_sha256'] != new['executable_sha256']
    for key in ['harness_sha256', 'profile', 'rustc', 'cpu_model', 'affinity', 'system']:
        assert old[key] == new[key], key
    assert old['profile']['opt_level'] == '3'
    excluded = {'elapsed_ns', 'terminal_node_probes', 'kernel_bytes'}
    for key, prior in left.items():
        current = right[key]
        assert {k: v for k, v in prior.items() if k not in excluded} == {
            k: v for k, v in current.items() if k not in excluded}, 'semantic/work replay differs'
        assert prior['allocations'] == prior['reallocations'] == 0
        assert prior['terminal_node_probes'] == 17 * prior['nodes']
        assert current['terminal_node_probes'] == 0
        assert current['kernel_bytes'] >= prior['kernel_bytes']
    summaries = []
    for nodes in [1, 8, 32, 256]:
        shape = {'nodes': nodes}
        for label, rows in [('before', left), ('after', right)]:
            selected = [row for row in rows.values() if row['nodes'] == nodes]
            assert len({row['events'] for row in selected}) == 1
            assert len({row['kernel_bytes'] for row in selected}) == 1
            assert all(isinstance(row['elapsed_ns'], int) and row['elapsed_ns'] >= 0 for row in selected)
            means = [statistics.mean(row['elapsed_ns'] for row in selected if row['process'] == process) for process in range(3)]
            shape[label] = {'process_mean_ns': means, 'mean_ns': statistics.mean(means),
                            'process_sd_ns': statistics.stdev(means), 'kernel_bytes': selected[0]['kernel_bytes']}
        summaries.append(shape)
    return {'before_source': old['source'], 'after_source': new['source'],
            'exact_sign_replay_pairs': len(left), 'artifact_hashes_verified': left_hashes + right_hashes,
            'summaries': summaries, 'limits': new['limits']}


def main():
    if not __debug__:
        raise RuntimeError('Python -O disables evidence checks')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before', type=Path, required=True)
    parser.add_argument('--after', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = compare(args.before, args.after)
    with args.output.open('x') as output:
        output.write(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    main()
