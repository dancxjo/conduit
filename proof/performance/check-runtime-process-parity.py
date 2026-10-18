#!/usr/bin/env python3
"""Mutation proof for cross-process expression output comparison on real captures."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', action='append', type=Path, required=True)
    parser.add_argument('--control', type=Path, required=True)
    parser.add_argument('--process-observations', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not __debug__:
        raise SystemExit('verification requires Python assertions; do not use -O')
    comparator = Path(__file__).with_name('compare-runtime-processes.py')
    assert 3 <= len(args.capture) <= 9
    assert all(not args.output.resolve().is_relative_to(p.resolve()) for p in args.capture)
    checks = []
    with tempfile.TemporaryDirectory(prefix='conduit-runtime-parity-') as temporary:
        root = Path(temporary)
        mutated = root / 'capture'
        shutil.copytree(args.capture[0], mutated)
        original_rows = [json.loads(line) for line in (mutated / 'raw.jsonl').read_text().splitlines()]
        for field in ['validation_sum', 'validation_sha256']:
            rows = json.loads(json.dumps(original_rows))
            changed = 0
            for row in rows:
                if row.get('phase') == 'warm-expression' and row['case'] == 'u64-add1':
                    value = row['run'][field]
                    row['run'][field] = str(int(value) + 1) if field == 'validation_sum' else ('1' if value[0] == '0' else '0') + value[1:]
                    changed += 1
            assert changed == 2 * json.loads((mutated / 'status.json').read_text())['config']['repetitions']
            raw = mutated / 'raw.jsonl'
            raw.write_text(''.join(json.dumps(row) + '\n' for row in rows))
            status = json.loads((mutated / 'status.json').read_text())
            status['artifact_sha256']['raw.jsonl'] = hashlib.sha256(raw.read_bytes()).hexdigest()
            (mutated / 'status.json').write_text(json.dumps(status))
            command = [sys.executable, str(comparator)]
            for capture in [mutated, *args.capture[1:]]:
                command += ['--capture', str(capture)]
            receipt = root / (field + '.json')
            command += ['--control', str(args.control), '--process-observations', str(args.process_observations), '--output', str(receipt)]
            result = subprocess.run(command, capture_output=True, text=True)
            assert result.returncode != 0 and 'expression outputs differ across processes' in result.stderr, result.stderr
            assert not receipt.exists(), 'refused comparison wrote a receipt'
            checks.append({'field': field, 'changed_rows': changed, 'refused': True,
                           'reason': 'expression outputs differ across processes'})
    result = {'checks': checks,
              'comparison_sha256': hashlib.sha256(comparator.read_bytes()).hexdigest(),
              'verifier_sha256': hashlib.sha256(comparator.with_name('verify-runtime-capture.py').read_bytes()).hexdigest(),
              'proof_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'scope': 'Rehashed mutated copies; original immutable captures are untouched.'}
    with args.output.open('x') as stream:
        json.dump(result, stream, indent=2)
        stream.write('\n')
    print(args.output)


if __name__ == '__main__':
    main()
