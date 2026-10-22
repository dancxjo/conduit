"""Supplement the original selected-controls verifier with five-choice Source checks.

Usage: /usr/bin/python3 SCRIPT PACKET EXPECTED_SOURCE OUTPUT.json
Requires the complete original packet, including retained ELF/ISO files.
"""
from pathlib import Path
import json
import runpy
import sys

module = runpy.run_path(str(Path(__file__).with_name('6158-verify-selected-controls.py')))
packet = Path(sys.argv[1]).resolve()
expected_source = sys.argv[2]
output = Path(sys.argv[3])
receipt = module['verify'](packet)
assert receipt['source'] == expected_source
decode = module['decode']
proof = decode((packet / 'creche-controls-proof.json').read_bytes())
initial = proof['initial_acknowledged_face']['face']
identities = ['creche.plot.' + str(i) for i in range(5)]
names = ['Keyboard canvas', 'Text editor', 'Tutorial', 'Patchbay', 'Hello, world.']


def inventory(face):
    actions = [a for a in face['actions'] if a['identity'].startswith('creche.plot.')]
    assert [a['identity'] for a in actions] == identities
    subjects = {s['identity']: s for s in face['subjects']}
    for i, action in enumerate(actions):
        assert subjects[action['target']]['name'] == names[i]
        selected = [p['value'] for p in face['properties']
                    if p['subject'] == action['target'] and p['name'] == 'selected']
        binding = [p['value'] for p in face['properties']
                   if p['subject'] == action['target'] and p['name'] == 'boolean-change-action']
        assert selected == [{'Flag': i < 4}]
        assert binding == [{'Identity': action['identity']}]
        expected = 'Available' if i < 4 else {'Unavailable': {
            'explanation': 'native-body-plot-unavailable', 'reason_code': 'birth/unavailable'}}
        assert action['availability'] == expected
    return actions


baseline = inventory(initial)
raw = (packet / 'journey-serial.log').read_bytes()
complete = raw[:raw.rfind(b'\n') + 1]
count = 0
for line in complete.splitlines():
    if line.startswith(b'CONDUIT_NATIVE_FACE '):
        observed = decode(line.split(b' ', 1)[1])
        assert inventory(observed['face']) == baseline
        count += 1
assert count > 4
result = {'status': 'verified', 'source': expected_source,
          'packet': str(packet), 'viewport': receipt['viewport'],
          'acknowledged_faces_checked': count, 'plot_choices': 5,
          'selected_available': 4, 'unavailable': names[-1],
          'unavailable_reason': baseline[-1]['availability'],
          'source_actions_unchanged': True,
          'original_controls_verification': receipt,
          'scope': 'Original five-choice Source inventory and action/selection constancy '
                   'across acknowledged Faces, plus sealed controls checks. '
                   'No independent Face digest recomputation or full issue acceptance.'}
output.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({k: result[k] for k in ['status', 'source', 'plot_choices',
                                      'selected_available', 'acknowledged_faces_checked']}))
