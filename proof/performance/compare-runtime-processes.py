#!/usr/bin/env python3
"""Compare separately observed, single-CPU hosted captures at one clean source."""
import argparse
import contextlib
import io
import hashlib
import importlib.util
import json
from pathlib import Path
import statistics
import sys
import tempfile

def compare(captures, control_path, process_path, output, verifier_path):
    assert 3 <= len(captures) <= 9
    assert len({p.resolve() for p in captures}) == len(captures), 'capture directory reused'
    assert all(not output.resolve().is_relative_to(p.resolve()) for p in captures), 'receipt must be outside immutable captures'
    control = json.loads(control_path.read_text())
    observations = json.loads(process_path.read_text())['workers']
    observed = {r['capture']: r for r in observations}
    assert len(observed) == len(captures)
    identities = {(r['pid'], r['start_time_ticks']) for r in observations}
    assert len(identities) == len(captures), 'worker process identity reused'
    cpu = control['selected_cpu']
    assert cpu in control['allowed_before'] and control['wrapper_affinity'] == [cpu]
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location('runtime_capture_verifier', verifier_path)
    verifier = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(verifier)
    receipts = []
    configs = []
    plot_hashes = []
    workers = []
    with tempfile.TemporaryDirectory(prefix='conduit-runtime-process-receipts-') as tmp:
        for index, capture in enumerate(captures):
            receipt_path = Path(tmp) / f'{index}.json'
            with contextlib.redirect_stdout(io.StringIO()):
                verifier.verify(capture, receipt_path)
            receipt = json.loads(receipt_path.read_text())
            status = json.loads((capture / 'status.json').read_text())
            assert receipt['source'] == control['source'], 'source differs across captures'
            config = status['config'].copy()
            original_path = config.pop('output')
            worker = observed[original_path]
            assert worker['affinity'] == str(cpu), 'observed worker affinity differs'
            assert receipt['metadata']['affinity'].split(':', 1)[1].strip() == str(cpu), 'worker affinity differs'
            configs.append(config)
            plot_hashes.append({k: v for k, v in receipt['artifact_sha256'].items() if k.endswith('.plot')})
            workers.append(worker)
            receipts.append(receipt)
    assert len({(r['pid'], r['start_time_ticks']) for r in workers}) == len(captures), 'worker process reused across captures'
    assert {r['capture'] for r in workers} == set(observed)
    assert len({r['executable'] for r in workers}) == 1
    assert all(c == configs[0] for c in configs)
    assert all(p == plot_hashes[0] for p in plot_hashes)
    for key in ['compiler', 'cargo', 'os', 'profile', 'debug_assertions', 'rustflags', 'release_overrides']:
        assert all(r['metadata'][key] == receipts[0]['metadata'][key] for r in receipts), key
    cpu_models = [{line for line in r['metadata']['cpu'].splitlines() if line.startswith('model name')} for r in receipts]
    assert cpu_models[0] and all(m == cpu_models[0] for m in cpu_models)
    plays = [r['active_play']['active_play_id'] for receipt in receipts for r in receipt['verified_pipeline_execution_identities']]
    assert len(plays) == len(set(plays)), 'Play identity reused across processes'
    grouped = []
    for receipt in receipts:
        grouped.append({(r['case'], r['mode'], r['metric']): r for r in receipt['repeated_batches']})
    assert all(g.keys() == grouped[0].keys() for g in grouped)
    comparisons = []
    for key in sorted(grouped[0]):
        batches = [g[key]['samples_ns'] for g in grouped]
        means = [statistics.mean(samples) for samples in batches]
        comparisons.append({
            'case': key[0], 'mode': key[1], 'metric': key[2],
            'process_batch_samples_ns': batches, 'process_means_ns': means,
            'mean_of_process_means_ns': statistics.mean(means),
            'sample_sd_process_means_ns': statistics.stdev(means),
            'minimum_process_mean_ns': min(means), 'maximum_process_mean_ns': max(means),
            'independent_processes': len(means),
        })
    result = {
        'source': control['source'], 'selected_cpu': cpu, 'config': configs[0],
        'observed_workers': workers, 'control': control,
        'verified_capture_artifact_hashes': sum(r['verified_artifact_hashes'] for r in receipts),
        'raw_records': sum(r['raw_records'] for r in receipts),
        'verified_batch_distributions': sum(r['verified_repeated_distributions'] for r in receipts),
        'metric_groups': comparisons,
        'capture_receipts': receipts,
        'verifier_sha256': hashlib.sha256(verifier_path.read_bytes()).hexdigest(),
        'comparison_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'control_sha256': hashlib.sha256(control_path.read_bytes()).hexdigest(),
        'process_observations_sha256': hashlib.sha256(process_path.read_bytes()).hexdigest(),
        'limits': ['Three or more separately observed processes; process means are the independent descriptive unit, not pooled batches.',
                   'Single-CPU affinity only: governor, thermal state, interrupts and other host load remain uncontrolled.',
                   'No calibrated regression threshold, inferential confidence interval, historical speedup or parent completion claim.'],
    }
    with output.open('x') as stream:
        json.dump(result, stream, indent=2)
        stream.write('\n')
    print(output)

def main():
    if not __debug__:
        raise SystemExit('verification requires Python assertions; do not use -O')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', action='append', type=Path, required=True)
    parser.add_argument('--control', type=Path, required=True)
    parser.add_argument('--process-observations', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    compare(args.capture, args.control, args.process_observations, args.output,
            Path(__file__).with_name('verify-runtime-capture.py'))
if __name__ == '__main__':
    main()
