"""Check retained pure preflight evidence; native wire remains a separate gate."""
import argparse
import hashlib
import json
import pathlib
import re


def check(serial, package, supported, root=None):
    digest = hashlib.sha256(package).hexdigest()
    lines = serial.splitlines()
    def unique(prefix):
        found = [i for i, line in enumerate(lines) if line.startswith(prefix)]
        assert len(found) == 1, (prefix, len(found))
        return found[0]
    plan = unique('CONDUIT_USB_PREFLIGHT_PLAN ')
    identities = dict(re.findall(r'(\w+)=(\S+)', lines[plan]))
    assert set(identities) == {'source', 'checked', 'expanded', 'plan', 'class_source', 'class_checked'}
    package_index = unique('CONDUIT_USB_PREFLIGHT_PACKAGE sha256=')
    assert lines[package_index + 1] == digest
    if root is not None:
        root_index = unique('CONDUIT_USB_PREFLIGHT_ROOT ')
        root_bytes = bytes.fromhex(lines[root_index + 1])
        root_metadata = re.fullmatch(r'CONDUIT_USB_PREFLIGHT_ROOT bytes=(\d+) sha256=([0-9a-f]{64})', lines[root_index])
        assert root_metadata and len(root_bytes) == int(root_metadata[1])
        assert hashlib.sha256(root_bytes).hexdigest() == root_metadata[2]
        def normalized(value):
            value = dict(value)
            for key in ('preflight', 'endpoint_read', 'endpoint_write'):
                if value.get(key) is None:
                    value.pop(key, None)
            if value.get('additional_remote_sign_items', 0) == 0:
                value.pop('additional_remote_sign_items', None)
            return value
        assert normalized(json.loads(root_bytes)) == normalized(json.loads(root))
        assert package_index < root_index
    attachment = unique('CONDUIT_USB_PREFLIGHT_ATTACHMENT ')
    facts = {k: int(v) for k, v in re.findall(r'(\w+)=(\d+)', lines[attachment])}
    assert facts['port'] == 1 and facts['epoch'] == 1
    assert facts['slot'] > 0 and facts['bar'] > 0
    assert facts['requested'] == facts['actual'] == 67
    observation = unique('CONDUIT_USB_PREFLIGHT_OBSERVATION ')
    observation_bytes = bytes.fromhex(lines[observation + 1])
    assert len(observation_bytes) == int(re.search(r'bytes=(\d+)', lines[observation])[1])
    output = unique('CONDUIT_USB_PREFLIGHT_OUTPUT ')
    assert re.search(r'kind=value/bool bytes=1$', lines[output])
    assert lines[output + 1] == ('01' if supported else '00')
    retired = unique('CONDUIT_USB_PREFLIGHT_RETIRED ')
    assert lines[retired] == 'CONDUIT_USB_PREFLIGHT_RETIRED ' + ('supported' if supported else 'unsupported')
    assert plan < package_index < attachment < observation < output < retired
    class_events = [i for i, line in enumerate(lines) if line.startswith(('CONDUIT_PROTOCOL_BASE ', 'CONDUIT_PROTOCOL_PLAY '))]
    if supported:
        assert class_events and min(class_events) > retired
        assert 'CONDUIT_PROTOCOL_RETIRED complete' in serial
    else:
        assert not class_events
        assert 'protocol-usb-profile-unsupported' in serial
        assert 'CONDUIT_PROTOCOL_RETIRED complete' not in serial
    return {'package_sha256': digest, 'supported': supported, 'retired_pure_discovery': True,
            'class_effects_after_verdict': supported, 'native_wire_validation': 'separate-required-gate'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('serial', type=pathlib.Path)
    parser.add_argument('package', type=pathlib.Path)
    parser.add_argument('verdict', choices=['supported', 'unsupported'])
    parser.add_argument('--root', type=pathlib.Path)
    args = parser.parse_args()
    print(json.dumps(check(args.serial.read_text(), args.package.read_bytes(), args.verdict == 'supported', args.root.read_bytes() if args.root else None)))
