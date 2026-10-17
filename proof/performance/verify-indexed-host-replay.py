#!/usr/bin/env python3
"""Verify indexed-host replay parity and sampled native storage; no speed claim."""
import argparse
import hashlib
import json
from pathlib import Path
import runpy

# Share exact coverage and successful-capture checks with the preceding slice.
shared = runpy.run_path(str(Path(__file__).with_name('verify-empty-host-replay.py')))
capture, without = shared['capture'], shared['without']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['before', 'after', 'before-source', 'after-source', 'output']:
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    sources = [p.read_text().strip() for p in [args.before_source, args.after_source]]
    assert sources[0] != sources[1]
    assert all(len(s) == 40 and all(c in '0123456789abcdef' for c in s) for s in sources)
    growth = {}
    for tag, key, omitted in [
        ('SCHEDULER_REPLAY_ROW', 'active', {'kernel_bytes'}),
        ('SCHEDULER_OVERHEAD_ROW', 'active_nodes', set()),
        ('SCHEDULER_SCAN_ROW', 'active_nodes', {'prepare_ns', 'step_ns'}),
    ]:
        before, after = [capture(p, tag, key) for p in [args.before, args.after]]
        for identity, old in before.items():
            new = after[identity]
            assert without(old, omitted) == without(new, omitted), (tag, identity)
            if tag == 'SCHEDULER_REPLAY_ROW':
                n = old['capacity']
                delta = new['kernel_bytes'] - old['kernel_bytes']
                assert delta == {1: 0, 8: 32, 32: 128, 256: 1024}[n]
                growth[str(n)] = delta
            elif tag == 'SCHEDULER_OVERHEAD_ROW':
                assert all(new[k] == 0 for k in ['ingress_allocations', 'ingress_reallocations',
                                                'step_allocations', 'step_reallocations'])
    report = {'before_source': sources[0], 'after_source': sources[1], 'replays_verified': 64,
              'exact_replay_and_non_timing_parity': True, 'native_growth_by_node_capacity': growth,
              'before_log_sha256': hashlib.sha256(args.before.read_bytes()).hexdigest(),
              'after_log_sha256': hashlib.sha256(args.after.read_bytes()).hexdigest(),
              'limits': ['Native instrumented debug/test replay; no wall-clock speedup claim.',
                         'Live-table work bounds are checked by the separate 4096-slot fixture.',
                         'Other Host Call queries remain linear; other layouts unmeasured.']}
    with args.output.open('x') as output:
        json.dump(report, output, indent=2)
        output.write('\n')
    print(args.output)


if __name__ == '__main__':
    main()
