#!/usr/bin/env python3
"""Compare source-pinned kernel captures without treating timings as a gate."""
import argparse
import hashlib
import json
from pathlib import Path
import statistics


def capture(folder):
    status = json.loads((folder / 'status.json').read_text())
    assert status['success'], 'capture did not finish successfully'
    for name, digest in status['artifacts_sha256'].items():
        assert hashlib.sha256((folder / name).read_bytes()).hexdigest() == digest, name
    metadata = json.loads((folder / 'metadata.json').read_text())
    assert metadata['source'] == status['source']
    runs = metadata.get('runs', 3)
    nodes = metadata.get('node_counts', [1, 8, 32])
    assert 3 <= runs <= 9 and nodes in [[1, 8, 32], [1, 8, 32, 256]]
    assert len(status['artifacts_sha256']) == 6 + 4 * runs + (2 if 'clean_command' in metadata else 0), 'incomplete capture'
    raw = json.loads((folder / 'raw.json').read_text())
    expected = {(p, n, sparse, trial) for p in range(runs) for n in nodes
                for sparse in [False, True] for trial in range(8)}
    indexed = {(r['process'], r['scan']['active_nodes'], r['scan']['sparse'], r['scan']['trial']): r
               for r in raw}
    assert len(raw) == len(indexed) == len(expected) and set(indexed) == expected
    replays = {}
    for process in range(runs):
        assert 'Exit status: 0' in (folder / f'process-{process}.resource.log').read_text()
        rows = json.loads((folder / f'process-{process}.replays.json').read_text())
        assert len(rows) == len(nodes) * 16
        for row in rows:
            key = (process, row['active'], row['sparse'], row['trial'])
            assert key not in replays
            replays[key] = row
    assert set(replays) == expected
    return metadata, indexed, replays, status


def describe(rows, active, sparse):
    selected = [row['scan'] for key, row in rows.items() if key[1:3] == (active, sparse)]
    means = [statistics.mean(row['scan']['step_ns'] for key, row in rows.items()
                             if key[:3] == (process, active, sparse)) for process in sorted({key[0] for key in rows})]
    outputs = 1 if sparse else active
    return {'process_mean_fixture_advance_ns': means,
            'mean_fixture_advance_ns': statistics.mean(means),
            'sample_sd_process_mean_ns': statistics.stdev(means),
            'mean_ns_per_useful_received_value': statistics.mean(means) / outputs,
            'useful_received_values_per_fixture': outputs,
            'samples': len(selected)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ['before', 'after', 'output']:
        parser.add_argument('--' + key, type=Path, required=True)
    args = parser.parse_args()
    old_meta, old, old_replay, old_status = capture(args.before)
    new_meta, new, new_replay, new_status = capture(args.after)
    assert old_meta['source'] != new_meta['source']
    for key in ['profile', 'rustc', 'machine', 'build_command']:
        assert old_meta[key] == new_meta[key], key
    assert old.keys() == new.keys(), 'different fixture coverage'
    if 'capture_harness_sha256' in old_meta and 'capture_harness_sha256' in new_meta:
        assert old_meta['capture_harness_sha256'] == new_meta['capture_harness_sha256']
    same_hardware = all(old_meta[k] == new_meta[k] for k in ['cpu_model', 'affinity'])
    assert old_meta['executable_sha256'] != new_meta['executable_sha256'], 'same executable cannot establish a changed-kernel comparison'
    work_keys = {'ready_nodes_scanned', 'ready_bytes_scanned', 'pending_slots_scanned',
                 'context_pending_slots', 'input_ports_checked', 'cords_scanned', 'queue_peeks'}
    work_changes = {}
    sizes = {}
    for key, before in old.items():
        after = new[key]
        assert before['overhead'] == after['overhead'], ('allocation/copy', key)
        ignored = work_keys | {'prepare_ns', 'step_ns'}
        assert {k: v for k, v in before['scan'].items() if k not in ignored} == {
            k: v for k, v in after['scan'].items() if k not in ignored}, key
        for field in work_keys:
            a, b = before['scan'][field], after['scan'][field]
            if a != b:
                work_changes.setdefault(field, set()).add((key[1], key[2], a, b))
        a, b = old_replay[key], new_replay[key]
        assert {k: v for k, v in a.items() if k != 'kernel_bytes'} == {
            k: v for k, v in b.items() if k != 'kernel_bytes'}, ('replay', key)
        sizes[str(key[1])] = {'before': a['kernel_bytes'], 'after': b['kernel_bytes'],
                              'delta': b['kernel_bytes'] - a['kernel_bytes']}
    summary = []
    for active in sorted({key[1] for key in old}):
        for sparse in [False, True]:
            a, b = describe(old, active, sparse), describe(new, active, sparse)
            summary.append({'active': active, 'sparse': sparse, 'before': a, 'after': b,
                            'descriptive_after_before_ratio': (b['mean_fixture_advance_ns'] / a['mean_fixture_advance_ns'] if same_hardware else None)})
    report = {'before_source': old_meta['source'], 'after_source': new_meta['source'],
              'same_reported_hardware_and_affinity': same_hardware,
              'timing_comparison_permitted': same_hardware,
              'verified_artifact_hashes': len(old_status['artifacts_sha256']) + len(new_status['artifacts_sha256']), 'cases_per_capture': len(old),
              'exact_sign_remote_output_readiness_value_terminal_replay_parity': True,
              'allocation_and_copy_counter_parity': True,
              'work_changes': {k: sorted(v) for k, v in work_changes.items()},
              'instrumented_scheduler_bytes': sizes, 'timing_summary': summary,
              'before_metadata': old_meta, 'after_metadata': new_meta,
              'capture_artifact_hashes': {'before': old_status['artifacts_sha256'], 'after': new_status['artifacts_sha256']},
              'limits': ['Uncontrolled captures at different times; different reported CPUs prohibit timing ratios. Even matching CPUs do not prove causal speedup.',
                         'Instrumented native fixed-kernel one-byte sinks; not checked hosted Plan/Play or FARGAN.',
                         'Step time spans the fixture advance, with harness counters, not one logical invocation.',
                         'No Host Calls or quiescence tracks in these fixtures; no measurement of those optimizations.',
                         'Object sizes include test counters; not production footprint evidence.',
                         'No physical, browser, stable-release or parent-issue completion claim.']}
    with args.output.open('x') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    print(args.output)


if __name__ == '__main__':
    main()
