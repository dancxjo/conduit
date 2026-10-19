"""Refuse semantic corruptions of relocated typing evidence, then restore it."""
import argparse
import importlib.util
import json
from pathlib import Path

base = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('typing_verifier', base / 'verify-typing-reconstruction.py')
verifier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verifier)


def run(root, report):
    verifier.verify(root, report)
    names = ['journey-qmp.log', 'journey-proof.json', 'journey-frames/manifest.json', 'journey-frames/serial-observations.jsonl', 'journey-serial.log']
    originals = {name: (root / name).read_bytes() for name in names}
    original_report = report.read_bytes()
    cases = []
    def write(path, value):
        path.write_text(json.dumps(value))
    def trial(name, mutate):
        try:
            mutate()
            try:
                verifier.verify(root, report)
            except AssertionError:
                cases.append({'mutation': name, 'refused': True})
            else:
                raise RuntimeError('accepted semantic corruption: ' + name)
        finally:
            for path, data in originals.items():
                (root / path).write_bytes(data)
            report.write_bytes(original_report)
    def metric(field, value):
        data = json.loads(report.read_text())
        data['typing_actions'][0][field] = value
        write(report, data)
    trial('forged typing duration', lambda: metric('command_to_rendered_journey_observed_ns', 0))
    trial('forged rendered manifestation', lambda: metric('manifestation_id', 'foreign'))
    def identity():
        data = json.loads((root / 'journey-proof.json').read_text())
        data['protected_retained_text_domain']['play_id'] = 'foreign'
        write(root / 'journey-proof.json', data)
    trial('foreign retained-domain Play', identity)
    def command(kind):
        path = root / 'journey-qmp.log'
        rows = [json.loads(line) for line in path.read_text().splitlines()]
        for index, row in enumerate(rows):
            message = json.loads(row['message'])
            if row['direction'] == 'command' and str(message.get('id', '')).endswith(':journey-protected-text-input'):
                if kind == 'key':
                    message['arguments']['events'][0]['data']['key']['data'] = 'z'
                    row['message'] = json.dumps(message)
                else:
                    answer = json.loads(rows[index + 1]['message'])
                    assert answer['id'] == message['id'] and 'return' in answer
                    answer['error'] = answer.pop('return')
                    rows[index + 1]['message'] = json.dumps(answer)
                break
        else:
            raise RuntimeError('no ordinary text command')
        path.write_text(''.join(json.dumps(row) + '\n' for row in rows))
    trial('wrong physical text key', lambda: command('key'))
    trial('failed text QMP acknowledgement', lambda: command('ack'))
    verifier.verify(root, report)
    return {'cases': cases, 'all_refused': True, 'scope': 'Relocated full-capture verification including 22 RGBA digests before and after five typing corruptions. Original raw archive unchanged; scratch bytes restored.'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('capture', type=Path)
    parser.add_argument('derived', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    text = json.dumps(run(args.capture, args.derived), indent=2) + '\n'
    if args.output:
        with args.output.open('x') as stream:
            stream.write(text)
    else:
        print(text, end='')
