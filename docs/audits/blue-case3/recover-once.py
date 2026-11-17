#!/usr/bin/env python3
"""One exact ff9 first-Return recovery cycle. STAGED; no default execution."""
import argparse
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

HERE = Path(__file__).absolute().parent
STATE = Path('/home/dancxjo/.local/state/conduit/host')
UNIT = 'conduit-host.service'
TOOL = Path('/home/dancxjo/.local/state/conduit/blue-admitted-recovery-20261107-a3a1b5b943/espflash')
PORT = '/dev/serial/by-id/usb-Silicon_Labs_CP2102_USB_to_UART_Bridge_Controller_0001-if00-port0'
OLD_SOURCE = '0b07493780ded2876856d443dd7fa75ff1326f8c'
EXPECTED_SOURCE = 'b341d1c6191d4e5bdafe593fb75b4efd5ec4313c'
EXPECTED_STAGE = HERE.parent / 'root-uart-release-b341d1c619-20261116'
OLD_BOOT = 'boot/installed/8b49f4cf1ce5a4a7621a0005d33c8599858f99aca91f9248009f51a22333307d'
OLD_PID = 296013

def need(ok, message):
    if not ok:
        raise RuntimeError(message)

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def doc(p):
    return json.loads(Path(p).read_bytes())

def call(args, timeout=90):
    return subprocess.run(list(map(str, args)), capture_output=True, timeout=timeout)

def checked(args, timeout=90):
    r = call(args, timeout)
    need(r.returncode == 0, 'command refused: ' + str(args[:4]))
    return r.stdout

def service():
    r = checked(['systemctl', '--user', 'show', '--all', UNIT, '-p', 'ActiveState',
                 '-p', 'MainPID', '-p', 'UnitFileState', '-p', 'KillSignal', '-p', 'ExecStop',
                 '-p', 'FragmentPath', '-p', 'DropInPaths'])
    d = dict(line.split('=', 1) for line in r.decode().splitlines() if '=' in line)
    d.setdefault('ExecStop', '')
    d.setdefault('DropInPaths', '')
    return d

def load_operator():
    spec = importlib.util.spec_from_file_location('exact_case3_operator', HERE / 'operator.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

def invariant_image(old, new, source):
    need(set(old) == set(new), 'IMAGE shape changed')
    for section in ['manifest', 'payload']:
        allowed = {'build_id', 'image_id', 'source_identity'} if section == 'manifest' else {'build_id'}
        need(set(old[section]) == set(new[section]), 'IMAGE fields changed')
        need(all(old[section][k] == new[section][k] for k in old[section] if k not in allowed),
             'original128 envelope/configuration/resources changed')
    need(new['manifest']['source_identity'] == source, 'new source mismatch')

def membership_prefix(old, new):
    """Preserve all original history; permit ONLY normal membership suffixes."""
    need(old['body_id'] == new['body_id'], 'Body changed')
    for key in old:
        if key not in ['records', 'membership', 'body']:
            need(old[key] == new[key], 'non-membership biography field changed: ' + key)
    need(new['records'][:len(old['records'])] == old['records'], 'original records prefix changed')
    for record in new['records'][len(old['records']):]:
        need(isinstance(record['kind'], dict) and set(record['kind']) in [{'HostLeft'}, {'HostJoined'}],
             'unexpected non-membership record append')
    for key in old['body']:
        if key == 'events':
            need(new['body'][key][:len(old['body'][key])] == old['body'][key], 'original Body events changed')
            need(len(new['body'][key]) == len(old['body'][key]), 'unexpected Body lifecycle event')
        else:
            need(old['body'][key] == new['body'][key], 'Body workset/history/state changed')
    before, after = old['membership'], new['membership']
    need(before['body_id'] == after['body_id'] and after['revision'] >= before['revision'], 'membership scope regressed')
    need(after['events'][:len(before['events'])] == before['events'], 'original membership events changed')
    known_parts = {p['part_id'] for p in before['parts']}
    need({p['part_id'] for p in after['parts']} == known_parts, 'Part identities changed')
    for event in after['events'][len(before['events']):]:
        need(event['body_id'] == old['body_id'] and event['part_id'] in known_parts
             and isinstance(event['kind'], dict) and set(event['kind']) in [{'HostDetached'}, {'HostAttached'}],
             'unexpected membership suffix')
    suffix = after['events'][len(before['events']):]
    need(after['revision'] == before['revision'] + len(suffix), 'membership revision/suffix mismatch')
    need(len(new['records']) - len(old['records']) == len(suffix), 'membership record/event count mismatch')

def admission_prefix(old, new):
    need(set(old) == set(new), 'admission custody shape changed')
    for key in old:
        if key == 'receipts':
            need(new[key][:len(old[key])] == old[key], 'old authenticated receipts changed or removed')
        else:
            need(new[key] == old[key], 'admission key/intent/custody field changed: ' + key)

def preflight(stage, source, prepare_result, test_report):
    need(os.uname().nodename.split('.')[0] == 'victus', 'Victus only')
    op = load_operator(); case, baseline = op.inputs()
    need(source == EXPECTED_SOURCE, 'exact qualified b341 UsbUart source required')
    stage = Path(stage).absolute(); prepare_result = Path(prepare_result).absolute(); test_report = Path(test_report).absolute()
    need(stage == EXPECTED_STAGE, 'wrong reviewed b341 artifact stage')
    installation = doc(STATE / 'installation.json'); runtime = doc(STATE / 'runtime.json')
    need(runtime['process_id'] == OLD_PID and runtime['boot_id'] == OLD_BOOT, 'actual current Root CAS changed')
    need(installation['release_source_identity'] == OLD_SOURCE
         and sha(installation['product_executable']) == case['root_executable_sha256']
         and sha(Path('/proc') / str(OLD_PID) / 'exe') == case['root_executable_sha256'], 'current Root binary changed')
    u = service()
    need(u['ActiveState'] == 'active' and u['MainPID'] == str(OLD_PID) and u['UnitFileState'] == 'enabled'
         and u['KillSignal'] == '15' and not u['ExecStop'], 'Root service changed')
    sig = next(l.split()[1] for l in Path('/proc/296013/status').read_text().splitlines() if l.startswith('SigCgt:'))
    need(not int(sig, 16) & (1 << 14), 'Root stop handler changed')
    # A retained failed Return may own exactly Blue. Stopping this original owner
    # is explicitly part of the reviewed cycle, followed by qualified chip reset.
    allowed_serial = os.path.realpath(PORT)
    for fd in Path('/proc/296013/fd').iterdir():
        try: target = os.readlink(fd)
        except FileNotFoundError: continue
        need(not target.startswith('/dev/tty') or target == allowed_serial, 'unrelated Root TTY')
    need(all(sha(STATE / n) == h for n, h in baseline.items()), 'exact current claim/state baseline changed')
    need(not any(os.path.lexists(STATE / n) for n in op.PENDING), 'pending request/Body journal')
    need(not os.path.lexists(STATE.parent / op.OP) and not os.path.lexists(op.RESET_RECEIPT), 'case already prepared/spent')
    status = json.loads(checked([installation['product_executable'], 'body', 'status', '--state-dir', STATE, '--json']))
    need(status['biography']['body_id'] == case['body_id'] and status['realization'] is None
         and status['biography']['body']['state'] == 'Lulled', 'unexpected current Source/Body state')
    # Disk claim denies resubmission even if this original process's monotonic
    # recovered flag remains false. No live file release is performed.
    need(sha(STATE / op.CLAIM) == case['original_claim_sha256'], 'new attempt claim changed')
    release_path = stage / 'reviewed-release/reviewed-artifact.json'; release = doc(release_path)
    image_path = stage / 'prepared-image/image.json'; image = doc(image_path)
    need(release['source_identity'] == source, 'release source mismatch')
    files = [f for f in release['files'] if Path(f['path']).name.startswith('conduit-')]
    need(len(files) == 1, 'ambiguous executable')
    exe = release_path.parent / files[0]['path']
    need(0 < exe.stat().st_size <= 67108864 and 'sha256:' + sha(exe) == files[0]['sha256'], 'new executable mismatch')
    invariant_image(installation['selected_host_configuration']['host_image'], image, source)
    preparation = stage / 'qualification-state/source-policy-preparation.json'
    terminal = doc(prepare_result); tests = doc(test_report)
    need(terminal.get('returncode') == 0 and terminal.get('source_identity') == source
         and terminal.get('executable_sha256') == sha(exe) and terminal.get('preparation_sha256') == sha(preparation),
         'new observed successful prepare receipt required')
    need(tests.get('source_identity') == source and tests.get('passed') is True
         and tests.get('usb_uart_no_dtr_physical_usb_binding') is True and tests.get('pty_rejected') is True,
         'focused product qualification receipt required')
    policy = doc(STATE / 'source-check-selection.json')
    need(all(policy.get(k) == [] for k in ['plays', 'returns', 'admissions', 'continuation_outputs']), 'one-shot policy replay')
    queue = [p for a in policy['local_fore_adapters'] if a['entry'] == 'yellow-output'
             and a['host_id'] == case['host_id'] for p in a['ports']
             if p['front_port_id'] == 'frame' and p['direction'] == 'Input' and p['track'] == 'payload']
    need(len(queue) == 1 and queue[0]['item_capacity'] == 1 and queue[0]['byte_capacity'] == 651, 'existing651 policy changed')
    full = op.inventory(STATE); required = sum((STATE / n).stat().st_size for n in full) + 2 * exe.stat().st_size + 272 * 1024**2
    need(shutil.disk_usage(STATE).free >= required, 'insufficient measured archive/reset/release headroom')
    files_to_pin = [HERE / 'operator.py', Path(__file__).absolute(), HERE / 'case-v3.json', HERE / 'baseline.json',
                    release_path, image_path, exe, preparation, prepare_result, test_report]
    files_to_pin += list((HERE / 'evidence').iterdir())
    for p in files_to_pin: op.no_links(p)
    need(sha(TOOL) == case['tool_sha256'], 'qualified espflash bytes changed')
    return {'schema': 'conduit.operator/claim-ff9-reset-upgrade-review@3', 'case_sha256': op.CASE_SHA,
            'new_source': source, 'stage': str(stage), 'prepare_result': str(prepare_result), 'test_report': str(test_report),
            'new_executable': str(exe), 'new_bundle': release['bundle_sha256'], 'policy': policy,
            'installation': installation, 'full_inventory': full, 'pins': {str(p): sha(p) for p in files_to_pin},
            'service': u, 'unit_pins': {p: sha(p) for p in [u['FragmentPath'], *u['DropInPaths'].split()]},
            'biography': doc(STATE / 'body/biography.json'), 'required_free_bytes': required,
            'physical_reset_assumptions_accepted_for_manual_review': True, 'effects_authorized_by_file': False}

def execute(review):
    need(preflight(review['stage'], review['new_source'], review['prepare_result'], review['test_report']) == review,
         'fresh complete preflight differs from approved case review')
    op = load_operator(); case, _ = op.inputs(); run_dir = HERE / 'effect-once'
    op.newdir(run_dir); op.put(run_dir / 'review.json', json.dumps(review, sort_keys=True).encode())
    effects = False
    def logged(name, args, timeout=90):
        r = call(args, timeout); op.put(run_dir / (name + '.stdout'), r.stdout); op.put(run_dir / (name + '.stderr'), r.stderr)
        need(r.returncode == 0, name + ' refused; retain evidence, no replay')
        return r.stdout + r.stderr
    def protected():
        # Biography alone may gain normal membership records at fresh startup.
        for n, h in review['full_inventory'].items():
            if (n.startswith('body/') and n not in ['body/biography.json', 'body/admission.json']) or n == 'control.token':
                need(sha(STATE / n) == h, 'immutable original keys/history file changed: ' + n)
    def interrupted(signum, frame): raise InterruptedError('maintenance signal')
    previous = signal.signal(signal.SIGTERM, interrupted)
    try:
        effects = True; logged('stop-root', ['systemctl', '--user', 'stop', UNIT])
        op.stopped(STATE)
        need(op.inventory(STATE) == review['full_inventory'], 'state changed during stop')
        locks = []
        try:
            for name in ['host-owner.lock', 'body-owner.lock']:
                fd = os.open(STATE / name, os.O_RDWR | os.O_NOFOLLOW); locks.append(fd)
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            op.stopped(STATE)
            need(sha(TOOL) == case['tool_sha256'], 'flasher changed')
            base = [TOOL, 'read-flash', '--non-interactive', '--skip-update-check', '--port', PORT,
                    '--baud', '921600', '--chip', 'esp32', '--after', 'no-reset', '0x0', '0x400000', run_dir / 'before-reset-flash.bin']
            output = logged('qualified-read-flash', base)
            need(case['physical_mac'] in output.decode(errors='replace').lower(), 'actual MAC not matched')
            flash = run_dir / 'before-reset-flash.bin'
            need(flash.stat().st_size == 4194304 and sha(flash) == case['before_flash_sha256'], 'full flash differs from exact seed readback')
            with flash.open('rb') as f: os.fsync(f.fileno())
            output = logged('qualified-final-hard-reset', [TOOL, 'board-info', '--non-interactive', '--skip-update-check',
                '--port', PORT, '--baud', '921600', '--chip', 'esp32', '--after', 'hard-reset'])
            need(case['physical_mac'] in output.decode(errors='replace').lower(), 'reset physical MAC mismatch')
            op.stopped(STATE)
            reset = {'schema': 'conduit.operator/exact-post-ff9-reset@1', 'case_sha256': op.CASE_SHA,
                     'claim_sha256': op.CLAIM_SHA, 'controller_sha256': sha(Path(__file__).absolute()),
                     'tool_sha256': case['tool_sha256'], 'physical_mac': case['physical_mac'],
                     'before_flash_sha256': sha(flash), 'root_stopped_pid': OLD_PID,
                     'read_flash_exit': 0, 'hard_reset_exit': 0, 'after_this_claim_attempt': True, 'serial_closed': True,
                     'at_unix_ns': time.time_ns(), 'measured_en_waveform': False,
                     'logs': {p.name: sha(p) for p in run_dir.iterdir() if p.is_file()}}
            op.put(op.RESET_RECEIPT, json.dumps(reset, sort_keys=True).encode())
        finally:
            for fd in reversed(locks): fcntl.flock(fd, fcntl.LOCK_UN); os.close(fd)
        # No archived/claimed intent is replayed. This explicit local case decision
        # is review authority; reset JSON alone is diagnostic, not a grant.
        decision = {'case_sha256': op.CASE_SHA, 'accept_reset_assumptions': True, 'operation': op.OP,
                    'outcome': 'UNKNOWN_RETIRED_BY_EXPLICIT_QUALIFIED_RESET',
                    'authorize_exact_claim_release': True, 'replay_old_request': False}
        op.transaction(STATE, HERE / 'evidence', decision)
        archive = STATE.parent / op.OP
        need(op.inventory(archive / 'state-before') == review['full_inventory'], 'full old-state archive mismatch')
        protected(); stage = Path(review['stage']); exe = review['new_executable']
        common = [exe, 'host', 'service', 'install', stage / 'reviewed-release/reviewed-artifact.json',
                  '--host-image', stage / 'prepared-image/image.json', '--state-dir', STATE]
        logged('install-no-start', [*common, '--no-start'])
        need(service()['MainPID'] == '0', 'no-start installation started Owner')
        installed = doc(STATE / 'installation.json')
        need(installed['host_id'] == review['installation']['host_id']
             and installed['body_state'] == review['installation']['body_state']
             and installed['release_bundle_sha256'] == review['new_bundle'], 'installed identity/binding changed')
        invariant_image(review['installation']['selected_host_configuration']['host_image'],
                        installed['selected_host_configuration']['host_image'], review['new_source'])
        need(installed['selected_host_configuration']['source'] == review['installation']['selected_host_configuration']['source'], 'configuration changed')
        need(doc(STATE / 'source-check-selection.json') == review['policy'], 'install changed policy')
        request = {'original_policy_sha256': 'sha256:' + sha(STATE / 'source-check-selection.json'),
                   'previous_release_bundle_sha256': review['policy']['release_bundle_sha256'],
                   'installed_release_bundle_sha256': review['new_bundle'], 'expected': doc(STATE / 'body/biography.json'),
                   'retire_actions': False, 'configuration_pin_replacements': [], 'local_fore_queue_replacements': []}
        op.put(run_dir / 'release-only-reselection.json', json.dumps(request, sort_keys=True).encode())
        logged('ordinary-release-reselect', [exe, 'host', 'service', 'reselect-source', run_dir / 'release-only-reselection.json', '--state-dir', STATE])
        want = dict(review['policy'], expected=request['expected'], release_bundle_sha256=review['new_bundle'])
        need(doc(STATE / 'source-check-selection.json') == want, 'unrelated policy/651 preference changed')
        protected(); logged('activate-standard', common)
        # Wait for runtime AND actual published BIO/control readiness, not merely PID.
        deadline = time.monotonic() + 120; ready = None
        while time.monotonic() < deadline:
            try:
                rt = doc(STATE / 'runtime.json'); u = service()
                if u['ActiveState'] == 'active' and int(u['MainPID']) == rt['process_id'] > 0 and rt['boot_id'] != OLD_BOOT:
                    r = call([exe, 'body', 'status', '--state-dir', STATE, '--json'], timeout=10)
                    if r.returncode == 0:
                        value = json.loads(r.stdout)
                        if (value.get('biography') is not None
                                and doc(STATE / 'body/biography.json') == value['biography']
                                and not any(os.path.lexists(STATE / n) for n in op.PENDING)):
                            ready = (rt, u, value); break
            except (OSError, ValueError, KeyError, subprocess.TimeoutExpired): pass
            time.sleep(1)
        need(ready is not None, 'fresh Owner BIO publication not ready by120s; no retry')
        rt, u, status = ready
        need(rt['body_id'] == case['body_id'] and rt['host_id'] == review['installation']['host_id']
             and rt['release_bundle_sha256'] == review['new_bundle'] and sha(Path('/proc') / str(rt['process_id']) / 'exe') == sha(exe), 'fresh Root mismatch')
        need(status['realization'] is None and status['biography']['body']['state'] == 'Lulled'
             and status['recovered_source_claim_unresolved'] is False and not (STATE / op.CLAIM).exists(), 'fresh Root not idle/unheld')
        membership_prefix(review['biography'], status['biography'])
        admission_prefix(doc(archive / 'state-before/body/admission.json'), doc(STATE / 'body/admission.json'))
        protected()
        need(doc(STATE / 'source-check-selection.json') == want and u['UnitFileState'] == 'enabled', 'policy/startup changed')
        op.put(run_dir / 'ready-status.json', json.dumps(status, sort_keys=True).encode())
        op.put(run_dir / 'success.json', json.dumps({'pid': rt['process_id'], 'boot': rt['boot_id'],
            'source': review['new_source'], 'claim_retired': op.CLAIM_SHA, 'old_history_prefix_preserved': True,
            'next_gate': 'fresh newRoot seed/readback and same-key Return+Play usb-uart; no Source request submitted',
            'source_play_proved': False}, sort_keys=True).encode())
    except BaseException:
        if effects:
            r = call(['systemctl', '--user', 'stop', UNIT]); op.put(run_dir / 'failure-stop.stdout', r.stdout); op.put(run_dir / 'failure-stop.stderr', r.stderr)
            op.put(run_dir / 'FAILED-NO-AUTOMATIC-RETRY', b'Retain current state/archive. Never restore old Root or replay old request.\n')
        raise
    finally: signal.signal(signal.SIGTERM, previous)

if __name__ == '__main__':
    os.umask(0o077); p = argparse.ArgumentParser(description=__doc__); m = p.add_mutually_exclusive_group(required=True)
    m.add_argument('--stage-review', type=Path); m.add_argument('--execute-review', type=Path)
    p.add_argument('--new-stage', type=Path); p.add_argument('--new-source'); p.add_argument('--prepare-result', type=Path)
    p.add_argument('--product-test-report', type=Path); p.add_argument('--review-sha256'); a = p.parse_args()
    if a.stage_review:
        need(all([a.new_stage, a.new_source, a.prepare_result, a.product_test_report]), 'all new qualified inputs required')
        value = preflight(a.new_stage, a.new_source, a.prepare_result, a.product_test_report)
        load_operator().put(a.stage_review, json.dumps(value, sort_keys=True).encode()); print('STAGED ONLY review SHA256 ' + sha(a.stage_review))
    else:
        need(a.review_sha256 is not None and sha(a.execute_review) == a.review_sha256, 'separate primary exact review release required')
        execute(doc(a.execute_review))
