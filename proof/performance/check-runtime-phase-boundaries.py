#!/usr/bin/env python3
"""Refuse rehashed malformed phase observations in a verified real capture."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not __debug__:
        raise SystemExit('verification requires Python assertions; do not use -O')
    assert not args.output.resolve().is_relative_to(args.capture.resolve())
    verifier = Path(__file__).with_name('verify-runtime-capture.py')
    original = [json.loads(line) for line in (args.capture / 'raw.jsonl').read_text().splitlines()]
    results = []
    with tempfile.TemporaryDirectory(prefix='conduit-phase-mutation-') as temporary:
        root = Path(temporary)
        copied = root / 'capture'
        subprocess.run([sys.executable, str(verifier), '--capture', str(args.capture),
                        '--output', str(root / 'baseline.json')],
                       check=True, capture_output=True, text=True)
        shutil.copytree(args.capture, copied)
        for name, reason in [('partition', 'phase partition differs'),
                             ('order', 'phase markers not monotonic'),
                             ('outer-bound', 'phase call exceeds measured outer call')]:
            rows = copy.deepcopy(original)
            row = next(r for r in rows if 'kernel_phase_wall' in r)
            phase = row['kernel_phase_wall']
            if name == 'partition':
                phase['play_ns'] += 1
            elif name == 'order':
                phase['marker_offsets_ns'][2] = phase['marker_offsets_ns'][1] - 1
            else:
                phase['call_wall_ns'] = row['full_host_call']['wall_ns'] + 1
                phase['after_play_ns'] = phase['call_wall_ns'] - phase['marker_offsets_ns'][2]
            raw = copied / 'raw.jsonl'
            raw.write_text(''.join(json.dumps(r) + '\n' for r in rows))
            status = json.loads((copied / 'status.json').read_text())
            status['artifact_sha256']['raw.jsonl'] = hashlib.sha256(raw.read_bytes()).hexdigest()
            (copied / 'status.json').write_text(json.dumps(status))
            receipt = root / (name + '.json')
            run = subprocess.run([sys.executable, str(verifier), '--capture', str(copied),
                                  '--output', str(receipt)], text=True, capture_output=True)
            assert run.returncode != 0 and reason in run.stderr, run.stderr
            assert not receipt.exists(), 'refused capture wrote a success receipt'
            results.append({'mutation': name, 'refused': True, 'reason': reason})
    result = {
        'checks': results,
        'verifier_sha256': hashlib.sha256(verifier.read_bytes()).hexdigest(),
        'mutation_proof_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'capture_sha': json.loads((args.capture / 'status.json').read_text())['git_sha'],
        'scope': 'Rehashed temporary raw capture mutations, original capture untouched.',
    }
    with args.output.open('x') as stream:
        json.dump(result, stream, indent=2)
        stream.write('\n')
    print(args.output)


if __name__ == '__main__':
    main()
