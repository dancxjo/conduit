#!/usr/bin/env python3
"""Retain exact fixed-kernel/reference replay for two wakes among 10,000 nodes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
TEST = 'scheduler::scan_characterization::sparse_large::two_ready_among_ten_thousand_preserve_fairness_pressure_and_ownership'


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, text=True).strip()


def custody(source):
    assert git('rev-parse', 'HEAD') == source and not git('status', '--porcelain'), 'source changed'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tagged(log, tag):
    rows = [json.loads(line.split(tag + ' ', 1)[1]) for line in log.splitlines() if tag + ' ' in line]
    assert len(rows) == 1, ('missing or duplicate observation', tag)
    return rows[0]


def main():
    if not __debug__:
        raise RuntimeError('Python optimization disables evidence checks; run without -O')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--release', action='store_true', help='Measure optimized production-kernel replay instead of debug conformance')
    parser.add_argument('--runs', type=int, default=1, help='Independent processes per selector (release requires at least three)')
    args = parser.parse_args()
    if not 1 <= args.runs <= 9 or (args.release and args.runs < 3):
        parser.error('runs must be 1..9; release requires 3..9')
    profile = ['--release'] if args.release else []
    output = args.output.resolve()
    if output.is_relative_to(ROOT):
        assert subprocess.run(['git', 'check-ignore', '-q', '--', str(output.relative_to(ROOT))], cwd=ROOT).returncode == 0, 'output must be outside source or ignored'
    source = git('rev-parse', 'HEAD')
    custody(source)
    output.mkdir(parents=True, exist_ok=False)
    status = {'source': source, 'success': False}
    try:
        env = os.environ.copy()
        env.update(RUSTUP_TOOLCHAIN=tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['channel'],
                   CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='2', CARGO_PROFILE_TEST_OPT_LEVEL='0',
                   CARGO_PROFILE_TEST_DEBUG_ASSERTIONS='true', CARGO_PROFILE_TEST_OVERFLOW_CHECKS='true')
        if args.release:
            env.update(CARGO_PROFILE_RELEASE_OPT_LEVEL='3', CARGO_PROFILE_RELEASE_DEBUG='0',
                       CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS='false', CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS='true')
        results = {}
        for label, features in [('packed', []), ('linear', ['--features', 'linear-ready-reference-test'])]:
            custody(source)
            with (output / f'{label}.clean.log').open('w') as log:
                subprocess.run(['cargo', 'clean', '-p', 'conduit-kernel', '--profile', 'release' if args.release else 'test'], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
            command = ['cargo', 'test', '--locked', '-p', 'conduit-kernel', '--lib', '--message-format=json', *profile, *features, '--no-run']
            with (output / f'{label}.stdout.log').open('w') as stdout, (output / f'{label}.stderr.log').open('w') as stderr:
                subprocess.run(command, cwd=ROOT, env=env, stdout=stdout, stderr=stderr, check=True)
            log = (output / f'{label}.stdout.log').read_text()
            artifacts = [json.loads(line) for line in log.splitlines() if line.startswith('{') and json.loads(line).get('executable')]
            assert len(artifacts) == 1 and artifacts[0]['fresh'] is False, 'measured package must rebuild'
            executable = Path(artifacts[0]['executable'])
            executable_digest = digest(executable)
            samples = []
            replay = None
            for run in range(args.runs):
                custody(source)
                assert digest(executable) == executable_digest, 'measurement executable changed'
                arguments = ['/usr/bin/time', '-v', '-o', str(output / f'{label}-{run}.resource.log'),
                             str(executable), TEST, '--exact', '--nocapture', '--test-threads=1']
                with (output / f'{label}-{run}.stdout.log').open('w') as stdout, (output / f'{label}-{run}.stderr.log').open('w') as stderr:
                    subprocess.run(arguments, cwd=ROOT, env=env, stdout=stdout, stderr=stderr, check=True)
                observed = (output / f'{label}-{run}.stdout.log').read_text()
                assert 'test result: ok. 1 passed; 0 failed' in observed
                current_replay = tagged(observed, 'SPARSE_LARGE_REPLAY')
                assert replay is None or replay == current_replay, 'within-selector replay differs'
                replay = current_replay
                samples.append(tagged(observed, 'SPARSE_LARGE_ROW'))
            results[label] = {'command': command, 'executable_sha256': executable_digest,
                              'profile': artifacts[0]['profile'], 'samples': samples,
                              'work': samples[0], 'replay': replay}
            custody(source)
        assert results['packed']['executable_sha256'] != results['linear']['executable_sha256'], 'selector executables must differ'
        assert results['packed']['replay'] == results['linear']['replay'], 'replay differs'
        for label, result in results.items():
            for sample in result['samples']:
                assert {k: v for k, v in sample.items() if k not in ('prepare_ns', 'advance_ns')} == {k: v for k, v in result['work'].items() if k not in ('prepare_ns', 'advance_ns')}, 'work differs between processes'
            work = result['work']
            assert (work['nodes'], work['ready'], work['logical_steps'], work['allocations'], work['reallocations']) == (10000, 2, 4, 0, 0)
            assert work['legacy'] == (label == 'linear')
        assert results['packed']['work']['candidate_nodes_scanned'] == 4
        assert results['packed']['work']['ready_bytes'] <= 1000
        assert results['linear']['work']['candidate_nodes_scanned'] >= 10000
        assert results['packed']['work']['kernel_bytes'] == results['linear']['work']['kernel_bytes']
        summaries = {}
        for label, result in results.items():
            summaries[label] = {}
            for metric in ('prepare_ns', 'advance_ns'):
                values = [sample[metric] for sample in result['samples']]
                summaries[label][metric] = {'raw_process_samples': values, 'mean': statistics.mean(values),
                                          'sample_sd': statistics.stdev(values) if len(values) > 1 else None}
            summaries[label]['advance_ns_per_useful_output'] = [sample['advance_ns'] / 2 for sample in result['samples']]
        report = {'source': source, 'optimized': args.release, 'runs_per_selector': args.runs,
                  'cpu_model': next((line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')), 'unavailable'),
                  'affinity': sorted(os.sched_getaffinity(0)), 'summaries': summaries, 'harness_sha256': digest(Path(__file__)), 'exact_replay_parity': True,
                  'rustc': subprocess.check_output(['rustc', '-Vv'], env=env, text=True),
                  'machine': platform.machine(), 'system': platform.platform(), 'results': results,
                  'limits': ['Instrumented native fixed-kernel fixture; shared-host timings are descriptive, not a regression gate.',
                             'Preparation and four Steps to Idle measured separately; startup, ingress, thread creation and serialization outside advance timing.',
                             'Per-process resource logs cover the whole fixture, not isolated Step CPU/RSS.',
                             'Linear reference shares current membership and lifecycle code; not a historical old-kernel speedup.',
                             '64 MiB diagnostic thread stack; not a constrained-device memory proof.',
                             'Direct admitted kernel topology, not a complete checked Plot/Plan/std Host pipeline.',
                             'Startup transient fuel Signs retire with an explicit gap; mandatory lifecycle Signs remain.',
                             'No CI, browser, physical, or parent-issue completion claim.']}
        (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        status['success'] = True
    finally:
        status['artifacts_sha256'] = {p.name: digest(p) for p in sorted(output.iterdir()) if p.is_file()}
        (output / 'status.json').write_text(json.dumps(status, indent=2) + '\n')
    print(output)


if __name__ == '__main__':
    main()
