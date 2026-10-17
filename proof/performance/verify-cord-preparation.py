#!/usr/bin/env python3
"""Verify cold-preparation captures without claiming a release speedup."""
import argparse
import hashlib
import json
from pathlib import Path


def capture(folder):
    status = json.loads((folder / 'status.json').read_text())
    assert status['success'] and len(status['artifacts_sha256']) == 7
    for name, digest in status['artifacts_sha256'].items():
        assert hashlib.sha256((folder / name).read_bytes()).hexdigest() == digest, name
    report = json.loads((folder / 'report.json').read_text())
    assert status['source'] == report['source'] and report['exact_replay_parity']
    for label, result in report['results'].items():
        log = (folder / f'{label}.stdout.log').read_text()
        assert 'test result: ok. 1 passed; 0 failed' in log
        for tag, field in [('SPARSE_LARGE_ROW', 'work'), ('SPARSE_LARGE_REPLAY', 'replay')]:
            rows = [json.loads(line.split(tag + ' ', 1)[1]) for line in log.splitlines() if tag + ' ' in line]
            assert rows == [result[field]], (label, tag)
    return report, status


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['before', 'after', 'output']:
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    old, old_status = capture(args.before)
    new, new_status = capture(args.after)
    assert old['source'] != new['source']
    for key in ['rustc', 'machine', 'system', 'harness_sha256']:
        assert old[key] == new[key], key
    observations = {}
    for label in ['packed', 'linear']:
        a, b = old['results'][label], new['results'][label]
        assert a['profile'] == b['profile'] and a['replay'] == b['replay']
        assert a['executable_sha256'] != b['executable_sha256']
        ignored = {'prepare_ns', 'advance_ns'}
        assert {k: v for k, v in a['work'].items() if k not in ignored} == {
            k: v for k, v in b['work'].items() if k not in ignored}
        observations[label] = {'before': a['work'], 'after': b['work']}
    report = {'before_source': old['source'], 'after_source': new['source'],
              'verified_artifact_hashes': 14, 'exact_replay_and_runtime_work_parity': True,
              'observations': observations, 'capture_hashes': {'before': old_status['artifacts_sha256'], 'after': new_status['artifacts_sha256']},
              'limits': ['Single native debug observation per selector/source, not a repeated release speed study.',
                         'Ordered independent ingress layout; unordered validation still uses pairwise fallback.',
                         'No physical, browser, complete authored Plan/Play, or parent completion claim.']}
    with args.output.open('x') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    print(args.output)


if __name__ == '__main__':
    main()
