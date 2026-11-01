"""One-machine rollout using the product installer, with retained identity checks."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

manifest = Path(sys.argv[1]).resolve()
expected_source = sys.argv[2]
home = Path.home()
state = home / '.local/state/conduit'
receipt_dir = home / '.local/state/conduit-update-20261031'
receipt_dir.mkdir(mode=0o700, exist_ok=True)
release = json.loads(manifest.read_text())
assert release['source_identity'] == expected_source, 'wrong release source'
product = next(f for f in release['files'] if f['path'].startswith('conduit-') and 'tour' not in f['path'])
binary = manifest.parent / product['path']
assert hashlib.sha256(binary.read_bytes()).hexdigest() == product['sha256'].removeprefix('sha256:'), 'wrong executable digest'
binary.chmod(binary.stat().st_mode | 0o100)
installation = state / 'installation.json'
old_bytes = installation.read_bytes()
old = json.loads(old_bytes)
unit = state / 'conduit-host.service'
user_unit = home / '.config/systemd/user/conduit-host.service'
old_unit = unit.read_bytes()
old_user_link = os.readlink(user_unit) if user_unit.is_symlink() else None
old_user_bytes = None if old_user_link is not None else user_unit.read_bytes()
protected = ['host_id', 'body_state', 'joined_body_state', 'selected_host_configuration', 'selected_speech', 'selected_model', 'selected_todo_checkpoint']
control = (state / 'control.token').read_bytes()

def command(args):
    return subprocess.run([str(a) for a in args], text=True, capture_output=True, timeout=90)

def systemctl(*args):
    result = command(['systemctl', '--user', *args])
    if result.returncode:
        raise RuntimeError(result.stderr.strip())
    return result.stdout.strip()

try:
    result = command([binary, 'host', 'service', 'install', manifest, '--state-dir', state])
    (receipt_dir / 'install.stdout').write_text(result.stdout)
    (receipt_dir / 'install.stderr').write_text(result.stderr)
    if result.returncode:
        raise RuntimeError('product installer refused; see private stderr')
    current = json.loads(installation.read_text())
    assert all(current.get(key) == old.get(key) for key in protected), 'retained identity/equipment changed'
    assert (state / 'control.token').read_bytes() == control, 'control identity changed'
    systemctl('daemon-reload')
    systemctl('restart', 'conduit-host.service')
    status = None
    deadline = time.monotonic() + 120
    while time.monotonic() < deadline:
        time.sleep(2)
        try:
            result = subprocess.run([str(binary), 'host', 'service', 'status', '--state-dir', str(state), '--json'], text=True, capture_output=True, timeout=8)
        except subprocess.TimeoutExpired:
            continue
        if result.returncode == 0:
            try:
                observed = json.loads(result.stdout)
                if observed.get('process_id') and observed.get('release_bundle_sha256') == release['bundle_sha256']:
                    status = observed
                    break
            except json.JSONDecodeError:
                pass
    assert status is not None, 'new live Host failed to become current'
    assert status['host_id'] == old['host_id'], 'live Host identity differs'
    assert systemctl('is-active', 'conduit-host.service') == 'active'
    receipt = {'source': expected_source, 'binary_sha256': product['sha256'], 'identity_and_equipment_preserved': True, 'runtime': status, 'service': systemctl('show', 'conduit-host.service', '-p', 'MainPID', '-p', 'NRestarts'), 'rolled_back': False}
    (receipt_dir / 'adoption.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt))
except Exception as failure:
    systemctl('stop', 'conduit-host.service')
    installation.write_bytes(old_bytes)
    unit.write_bytes(old_unit)
    if user_unit.is_symlink() or user_unit.exists():
        user_unit.unlink()
    if old_user_link is not None:
        user_unit.symlink_to(old_user_link)
    else:
        user_unit.write_bytes(old_user_bytes)
    systemctl('daemon-reload')
    systemctl('start', 'conduit-host.service')
    receipt = {'source': expected_source, 'rolled_back': True, 'reason': str(failure), 'service': systemctl('is-active', 'conduit-host.service')}
    (receipt_dir / 'adoption.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt))
    raise SystemExit(1)
