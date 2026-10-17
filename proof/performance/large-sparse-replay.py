#!/usr/bin/env python3
"""Retain exact fixed-kernel/reference replay for two wakes among 10,000 nodes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
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
        results = {}
        for label, features in [('packed', []), ('linear', ['--features', 'linear-ready-reference-test'])]:
            custody(source)
            with (output / f'{label}.clean.log').open('w') as log:
                subprocess.run(['cargo', 'clean', '-p', 'conduit-kernel', '--profile', 'test'], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
            command = ['cargo', 'test', '--locked', '-p', 'conduit-kernel', '--lib', '--message-format=json', *features, TEST,
                       '--', '--exact', '--nocapture', '--test-threads=1']
            with (output / f'{label}.stdout.log').open('w') as stdout, (output / f'{label}.stderr.log').open('w') as stderr:
                subprocess.run(command, cwd=ROOT, env=env, stdout=stdout, stderr=stderr, check=True)
            log = (output / f'{label}.stdout.log').read_text()
            assert 'test result: ok. 1 passed; 0 failed' in log
            artifacts = [json.loads(line) for line in log.splitlines() if line.startswith('{') and json.loads(line).get('executable')]
            assert len(artifacts) == 1 and artifacts[0]['fresh'] is False, 'measured package must rebuild'
            results[label] = {'command': command, 'executable_sha256': digest(Path(artifacts[0]['executable'])),
                              'profile': artifacts[0]['profile'], 'work': tagged(log, 'SPARSE_LARGE_ROW'),
                              'replay': tagged(log, 'SPARSE_LARGE_REPLAY')}
            custody(source)
        assert results['packed']['replay'] == results['linear']['replay'], 'replay differs'
        for label, result in results.items():
            work = result['work']
            assert (work['nodes'], work['ready'], work['logical_steps'], work['allocations'], work['reallocations']) == (10000, 2, 4, 0, 0)
            assert work['legacy'] == (label == 'linear')
        assert results['packed']['work']['candidate_nodes_scanned'] == 4
        assert results['packed']['work']['ready_bytes'] <= 1000
        assert results['linear']['work']['candidate_nodes_scanned'] >= 10000
        assert results['packed']['work']['kernel_bytes'] == results['linear']['work']['kernel_bytes']
        report = {'source': source, 'harness_sha256': digest(Path(__file__)), 'exact_replay_parity': True,
                  'rustc': subprocess.check_output(['rustc', '-Vv'], env=env, text=True),
                  'machine': platform.machine(), 'system': platform.platform(), 'results': results,
                  'limits': ['Native debug fixed-kernel conformance, not a release speed comparison.',
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
