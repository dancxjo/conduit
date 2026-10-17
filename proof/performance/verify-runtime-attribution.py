#!/usr/bin/env python3
"""Independently join source-pinned kernel and native Sign attribution observations."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import statistics
import sys


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    assert __debug__, 'verification requires assertions; do not use -O'
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--kernel', type=Path, required=True)
    parser.add_argument('--sign', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert all(not args.output.resolve().is_relative_to(p.resolve())
               for p in [args.kernel, args.sign])
    sys.dont_write_bytecode = True
    verifier_path = Path(__file__).with_name('verify-kernel-refresh.py')
    spec = importlib.util.spec_from_file_location('kernel_verifier', verifier_path)
    verifier = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(verifier)
    meta, indexed, replays, status = verifier.capture(args.kernel)
    for key, row in indexed.items():
        scan, overhead = row['scan'], row['overhead']
        assert all(scan[k] == overhead[k] for k in ['active_nodes', 'sparse', 'trial'])
        assert overhead['ingress_store_calls'] == scan['ready_inputs']
        assert overhead['ingress_value_store_copy_bytes'] == scan['ready_inputs']
        assert overhead['step_value_store_copy_bytes'] == 0
        assert all(overhead[k] == 0 for k in ['ingress_allocations', 'ingress_reallocations',
                                             'step_allocations', 'step_reallocations'])
        assert scan['queue_pressure_refused'] is True
    hashes = json.loads((args.sign / 'sha256.json').read_text())
    assert len(hashes) == 13
    assert all(digest(args.sign / p) == h for p, h in hashes.items())
    sign_meta = json.loads((args.sign / 'metadata.json').read_text())
    assert sign_meta['source'] == meta['source']
    observations = json.loads((args.sign / 'observations.json').read_text())
    expected = {(p, c, t) for p in range(3) for c in [1, 8, 32, 128, 1024] for t in range(32)}
    rows = observations['rows']
    assert len(rows) == len(expected)
    assert {(r['process'], r['capacity'], r['trial']) for r in rows} == expected
    for process in range(3):
        assert 'Exit status: 0' in (args.sign / f'process-{process}.resource.log').read_text()
        raw = [json.loads(line.split('SIGN_RECORD_COST_ROW ', 1)[1])
               for line in (args.sign / f'process-{process}.stdout.log').read_text().splitlines()
               if 'SIGN_RECORD_COST_ROW ' in line]
        assert [{"process": process, **r} for r in raw] == [r for r in rows if r['process'] == process]
    for r in rows:
        assert r['records_per_phase'] == r['capacity']
        assert r['retained_event_bytes'] == r['capacity'] * r['event_size']
    summaries = observations['summaries']
    assert len(summaries) == 15
    assert {(r['capacity'], r['phase']) for r in summaries} == {
        (c, p) for c in [1, 8, 32, 128, 1024]
        for p in ['prepare_ns', 'append_ns', 'full_retention_ns']}
    for s in summaries:
        means = [statistics.mean(r[s['phase']] for r in rows
                                 if r['process'] == p and r['capacity'] == s['capacity'])
                 for p in range(3)]
        assert s['process_means_ns'] == means
        assert s['mean_ns'] == statistics.mean(means)
        assert s['process_mean_sample_sd_ns'] == statistics.stdev(means)
    receipt = {'source': meta['source'], 'kernel_observations_verified': len(indexed),
               'kernel_artifact_sha256': status['artifacts_sha256'],
               'kernel_summary': [{'active': n, 'sparse': sparse, **verifier.describe(indexed, n, sparse)}
                                  for n in sorted({k[1] for k in indexed}) for sparse in [False, True]],
               'sign_observations_verified': len(rows), 'sign_artifact_sha256': hashes,
               'sign_summary': summaries, 'proof_sha256': digest(Path(__file__)),
               'kernel_verifier_sha256': digest(verifier_path),
               'limits': 'Instrumented fixed-kernel byte sinks and real native Sign-record batches; not whole-Host CPU share, total memory traffic, physical or browser proof.'}
    with args.output.open('x') as f:
        json.dump(receipt, f, indent=2)
        f.write('\n')
    print(args.output)


if __name__ == '__main__':
    main()
