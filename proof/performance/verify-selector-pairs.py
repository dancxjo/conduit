#!/usr/bin/env python3
"""Verify alternating same-source production/reference selector captures."""
import argparse
import hashlib
import json
from pathlib import Path
import statistics


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(folder):
    status = json.loads((folder / 'status.json').read_text())
    meta = json.loads((folder / 'metadata.json').read_text())
    assert status['success'] and status['source'] == meta['source']
    assert status['pairs'] == 9 and status['raw_samples'] == 1152
    assert len(meta['affinity']) == 1
    assert digest(folder.parent / 'capture-selector-alternating.py') == meta['harness_sha256']
    for name, expected in status['artifacts_sha256'].items():
        path = (folder / name).resolve()
        assert path.is_relative_to(folder.resolve())
        assert digest(path) == expected, name
    assert meta['builds']['packed']['profile'] == meta['builds']['linear']['profile']
    for label in ['packed', 'linear']:
        assert digest(folder / 'bin' / label) == meta['builds'][label]['executable_sha256']
        artifacts = [json.loads(s) for s in (folder / f'{label}.build.jsonl').read_text().splitlines()
                     if s.startswith('{') and json.loads(s).get('executable')]
        assert len(artifacts) == 1 and artifacts[0]['fresh'] is False
        assert artifacts[0]['profile']['opt_level'] == '3'
        assert artifacts[0]['profile'] == meta['builds'][label]['profile']
    raw = json.loads((folder / 'raw.json').read_text())
    indexed = {(r['pair'], r['selector'], r['scan']['active_nodes'],
                r['scan']['sparse'], r['scan']['trial']): r for r in raw}
    expected = {(p, l, n, s, t) for p in range(9) for l in ['packed', 'linear']
                for n in [1, 8, 32, 256] for s in [False, True] for t in range(8)}
    assert len(raw) == len(indexed) == len(expected) and set(indexed) == expected
    canonical = {}
    order = []
    for pair in range(9):
        for label in (['packed', 'linear'] if pair % 2 == 0 else ['linear', 'packed']):
            order.append((pair, label))
            stem = f'{pair}-{label}'
            assert 'Exit status: 0' in (folder / f'{stem}.resource.log').read_text()
            parsed = {tag: [] for tag in ['SCHEDULER_SCAN_ROW', 'SCHEDULER_OVERHEAD_ROW', 'SCHEDULER_REPLAY_ROW']}
            text = (folder / f'{stem}.stdout.log').read_text()
            assert 'test result: ok. 1 passed; 0 failed' in text
            for line in text.splitlines():
                for tag in parsed:
                    if tag + ' ' in line:
                        parsed[tag].append(json.loads(line.split(tag + ' ', 1)[1]))
            assert all(len(v) == 64 for v in parsed.values())
            seen = set()
            for scan, overhead, replay in zip(*parsed.values()):
                key = (scan['active_nodes'], scan['sparse'], scan['trial'])
                assert key not in seen
                seen.add(key)
                assert key == (replay['active'], replay['sparse'], replay['trial'])
                assert indexed[pair, label, *key] == {'pair': pair, 'selector': label, 'scan': scan, 'overhead': overhead}
                assert replay['legacy'] == (label == 'linear')
                assert all(overhead[k] == 0 for k in ['ingress_allocations', 'ingress_reallocations',
                           'step_allocations', 'step_reallocations', 'step_value_store_copy_bytes'])
                semantic = {k: v for k, v in replay.items() if k != 'legacy'}
                if key in canonical:
                    assert semantic == canonical[key], ('semantic replay', pair, label, key)
                else:
                    canonical[key] = semantic
    processes = json.loads((folder / 'processes.json').read_text())
    assert [(p['pair'], p['selector']) for p in processes] == order
    assert json.loads((folder / 'replays.json').read_text()) == list(canonical.values())
    ignored = {'prepare_ns', 'step_ns', 'ready_nodes_scanned', 'ready_bytes_scanned', 'pending_slots_scanned'}
    for pair in range(9):
        for n, sparse, trial in canonical:
            a, b = (indexed[pair, label, n, sparse, trial] for label in ['packed', 'linear'])
            assert a['overhead'] == b['overhead']
            assert {k: v for k, v in a['scan'].items() if k not in ignored} == {
                k: v for k, v in b['scan'].items() if k not in ignored}
    summaries = json.loads((folder / 'summary.json').read_text())
    assert len(summaries) == 8
    for summary, (n, sparse) in zip(summaries, [(n, s) for n in [1, 8, 32, 256] for s in [False, True]]):
        assert (summary['nodes'], summary['sparse']) == (n, sparse)
        values = {label: [statistics.mean(indexed[p, label, n, sparse, t]['scan']['step_ns']
                                         for t in range(8)) for p in range(9)] for label in ['packed', 'linear']}
        assert summary['process_mean_fixture_advance_ns'] == values
        assert summary['mean_ns'] == {k: statistics.mean(v) for k, v in values.items()}
        assert summary['sample_sd_ns'] == {k: statistics.stdev(v) for k, v in values.items()}
        ratios = [values['packed'][p] / values['linear'][p] for p in range(9)]
        assert summary['paired_packed_over_linear_ratios'] == ratios
        assert summary['paired_ratio_mean'] == statistics.mean(ratios)
        assert summary['paired_ratio_range'] == [min(ratios), max(ratios)]
    return {'source': meta['source'], 'pairs': 9, 'raw_samples': len(indexed),
            'verified_artifact_hashes': len(status['artifacts_sha256']),
            'exact_sign_output_readiness_value_terminal_size_replay_parity': True,
            'allocation_copy_nonselection_work_parity': True, 'summaries': summaries,
            'metadata': meta, 'limits': status['limits'], 'verifier_sha256': digest(Path(__file__))}


def main():
    if not __debug__:
        raise SystemExit('verification requires Python assertions')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    with args.output.open('x') as output:
        json.dump(verify(args.capture), output, indent=2)
        output.write('\n')
    print(args.output)


if __name__ == '__main__':
    main()
