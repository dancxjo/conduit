#!/usr/bin/env python3
"""Check instrumented empty-Host-table replay parity; no timing speedup claim."""
import argparse
import hashlib
import json
from pathlib import Path


def capture(path, tag, key):
    assert 'test result: ok. 1 passed; 0 failed;' in path.read_text()
    rows = [json.loads(line.split(tag + ' ', 1)[1])
            for line in path.read_text().splitlines() if tag + ' ' in line]
    expected = {(n, sparse, trial) for n in [1, 8, 32, 256]
                for sparse in [False, True] for trial in range(8)}
    indexed = {(row[key], row['sparse'], row['trial']): row for row in rows}
    assert len(rows) == len(indexed) == 64 and set(indexed) == expected
    return indexed


def without(row, keys):
    return {key: value for key, value in row.items() if key not in keys}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before', type=Path, required=True)
    parser.add_argument('--after', type=Path, required=True)
    parser.add_argument('--before-source', type=Path, required=True)
    parser.add_argument('--after-source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    sources = [path.read_text().strip() for path in [args.before_source, args.after_source]]
    assert sources[0] != sources[1]
    assert all(len(source) == 40 and all(c in '0123456789abcdef' for c in source) for source in sources)
    work = {'before': 0, 'after': 0}
    growth = set()
    for tag, key, omitted in [
        ('SCHEDULER_REPLAY_ROW', 'active', {'kernel_bytes'}),
        ('SCHEDULER_OVERHEAD_ROW', 'active_nodes', set()),
        ('SCHEDULER_SCAN_ROW', 'active_nodes',
         {'prepare_ns', 'step_ns', 'pending_slots_scanned', 'context_pending_slots'}),
    ]:
        before = capture(args.before, tag, key)
        after = capture(args.after, tag, key)
        for identity, old in before.items():
            new = after[identity]
            assert without(old, omitted) == without(new, omitted), (tag, identity)
            if tag == 'SCHEDULER_REPLAY_ROW':
                growth.add(new['kernel_bytes'] - old['kernel_bytes'])
            elif tag == 'SCHEDULER_SCAN_ROW':
                assert new['pending_slots_scanned'] == new['context_pending_slots'] == 0
                work['before'] += old['pending_slots_scanned'] + old['context_pending_slots']
                work['after'] += new['pending_slots_scanned'] + new['context_pending_slots']
            else:
                assert all(new[name] == 0 for name in [
                    'ingress_allocations', 'ingress_reallocations', 'step_allocations', 'step_reallocations'])
    assert work['before'] > 0 and growth == {8}
    report = {'before_source': sources[0], 'after_source': sources[1],
              'before_log_sha256': hashlib.sha256(args.before.read_bytes()).hexdigest(),
              'after_log_sha256': hashlib.sha256(args.after.read_bytes()).hexdigest(),
              'replays_verified': 64, 'exact_replay_and_non_timing_parity': True,
              'empty_pending_slot_probes': work, 'native_scheduler_growth_bytes': 8,
              'limits': ['Instrumented native debug/test replay; no wall-clock speedup claim.',
                         'Live Host Call table selection remains linear.',
                         'Other table queries and target layouts are outside this observation.']}
    with args.output.open('x') as output:
        json.dump(report, output, indent=2)
        output.write('\n')
    print(args.output)


if __name__ == '__main__':
    main()
