import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import sys
import tempfile
sys.dont_write_bytecode = True
source, capture = map(Path, sys.argv[1:])
spec = importlib.util.spec_from_file_location('resource_cost_verifier', source)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
module.verify(capture)
for mutation in ['wrong_total', 'changed_reference', 'expired_observation', 'failed_custody', 'reused_handle', 'missing_reservation']:
    with tempfile.TemporaryDirectory(prefix='resource-cost-negative-') as directory:
        root = Path(directory) / 'capture'
        shutil.copytree(capture, root)
        status = json.loads((root / 'status.json').read_text())
        data = json.loads((root / 'measurements.json').read_text())
        if mutation == 'wrong_total':
            data['trials'][0]['sequential_overhead']['upper']['numerator'] = '999999999'
        elif mutation == 'changed_reference':
            data['trials'][0]['stages'][0]['resource_reference_bytes'][0] ^= 1
        elif mutation == 'expired_observation':
            value = data['trials'][0]['stages'][0]['attribution']
            value['expires_tick'] = value['observed_tick']
        elif mutation == 'failed_custody':
            status['source_custody_verified'] = False
        elif mutation == 'reused_handle':
            copied = [r for r in data['trials'] if r['mode'] == 'prepared-local-copy']
            copied[1]['residences'][1]['handle'] = copied[0]['residences'][1]['handle']
        else:
            data['trials'][0]['residences'][0]['reservation'] = []
        path = root / 'measurements.json'
        path.write_text(json.dumps(data))
        status['artifact_sha256']['measurements.json'] = hashlib.sha256(path.read_bytes()).hexdigest()
        (root / 'status.json').write_text(json.dumps(status))
        try:
            module.verify(root)
        except AssertionError:
            print('refused', mutation)
        else:
            raise AssertionError('accepted ' + mutation)
print('real capture verified; six rehashed negatives refused')
