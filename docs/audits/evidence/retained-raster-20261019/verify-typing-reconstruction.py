"""Independently reconstruct six editor bounds from an archived ordinary journey.

The optional report is derived evidence; the original capture remains immutable.
No timing acceptance, sustained typing throughput, or physical display claim.
"""
import argparse
import importlib.util
import json
from pathlib import Path


def reconstruct(root):
    frames = root / 'journey-frames'
    manifest = json.loads((frames / 'manifest.json').read_text())
    proof = json.loads((root / 'journey-proof.json').read_text())
    identity = proof['protected_retained_text_domain']
    clock = 'host/process-monotonic-nanoseconds@1'
    start = manifest['context']['observer_clock']['start_ns']
    trace = []
    last = start
    for line in (root / 'journey-qmp.log').read_text().splitlines():
        row = json.loads(line)
        assert row['observer_clock'] == clock
        assert row['direction'] in ('command', 'response')
        assert row['observer_timestamp_ns'] >= last
        last = row['observer_timestamp_ns']
        trace.append((row, json.loads(row['message'])))
    commands = []
    for index, (row, message) in enumerate(trace):
        if row['direction'] != 'command' or message.get('execute') != 'input-send-event':
            continue
        if not message.get('id', '').endswith((':journey-protected-text-input', ':journey-unicode-input')):
            continue
        events = message['arguments']['events']
        if all(e['data']['down'] is False for e in events):
            continue
        assert events and all(e['type'] == 'key' and e['data']['down'] is True and e['data']['key']['type'] == 'qcode' for e in events)
        answers = []
        for response, payload in trace[index + 1:]:
            if response['direction'] == 'command':
                break
            if payload.get('id') == message['id']:
                answers.append((response, payload))
        assert len(answers) == 1 and 'return' in answers[0][1] and 'error' not in answers[0][1]
        commands.append((message['id'], row['observer_timestamp_ns'], answers[0][0]['observer_timestamp_ns'], [e['data']['key']['data'] for e in events]))
    expected = [(['a'], 'A'), (['a'], 'a'), (['backspace'], ''), (['c'], 'c'), (['a'], 'ca'), (['f'], 'caf'), (['alt_r', 'e'], 'café')]
    assert len(commands) == len(expected)
    raw = (root / 'journey-serial.log').read_bytes()
    records = []
    for line in (frames / 'serial-observations.jsonl').read_text().splitlines():
        row = json.loads(line)
        if row['kind'] == 'record':
            text = raw[row['serial_byte_start']:row['serial_byte_end']].decode()
            if text.startswith('CONDUIT_PRODUCT_JOURNEY '):
                records.append((row, json.loads(text.split(' ', 1)[1])))
    result = []
    previous_revision = 0
    previous_face = ''
    for index, command in enumerate(commands):
        assert command[3] == expected[index][0]
        if index == 0:
            continue
        stop = commands[index + 1][1] if index + 1 < len(commands) else float('inf')
        selected = []
        for row, value in records:
            if not command[1] <= row['read_completed_ns'] < stop:
                continue
            if value['result'] != expected[index][1] or value['text_editor_active'] is not True:
                continue
            assert value['status'] == 'quiescent-awaiting-input'
            for left, right in [('plan_id', 'plan_id'), ('active_play_id', 'play_id'), ('checked_plot_id', 'checked_plot_id'), ('partition_plan_id', 'partition_plan_id')]:
                assert value[left] == identity[right] and identity[right]
            selected.append((row, value))
        assert selected
        row, value = selected[0]
        assert value['revision'] > previous_revision and value['face_id'] != previous_face
        assert all(isinstance(value[k], str) and value[k] for k in ('face_id', 'manifestation_id', 'input_sign_id', 'result_sign_id'))
        previous_revision, previous_face = value['revision'], value['face_id']
        result.append(dict(qmp_id=command[0], command_ns=command[1], qmp_accepted_ns=command[2], keys=command[3], result=value['result'], revision=value['revision'], face_id=value['face_id'], manifestation_id=value['manifestation_id'], input_sign_id=value['input_sign_id'], result_sign_id=value['result_sign_id'], serial_byte_start=row['serial_byte_start'], command_to_rendered_journey_observed_ns=row['read_completed_ns'] - command[1]))
    return result


def verify(root, derived):
    spec = importlib.util.spec_from_file_location('base_verifier', Path(__file__).with_name('verify-live-conformance.py'))
    base = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(base)
    original = base.verify(root)
    report = json.loads(derived.read_text())
    for key in ('source_commit', 'image_sha256', 'pointer_actions', 'boot_stage_observations'):
        assert report[key] == json.loads((root / 'journey-frames/latency.json').read_text())[key]
    assert report['typing_actions'] == reconstruct(root)
    original['typing_rows_independently_reconstructed'] = 6
    original['scope'] += ' Six serialized editor transitions verified; no sustained-throughput claim.'
    return original


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('capture', type=Path)
    parser.add_argument('derived', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    text = json.dumps(verify(args.capture, args.derived), indent=2) + '\n'
    if args.output:
        with args.output.open('x') as stream:
            stream.write(text)
    else:
        print(text, end='')
