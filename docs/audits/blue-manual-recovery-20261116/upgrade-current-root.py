#!/usr/bin/env python3
"""Exact Victus Root maintenance controller. Staged; execution needs reviewed pins."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import importlib.util
import signal

HERE = Path(__file__).absolute().parent
STAGE = HERE.parent / 'root-next-release-staging-20261116'
STATE = Path('/home/dancxjo/.local/state/conduit/host')
SOURCE = '0b07493780ded2876856d443dd7fa75ff1326f8c'
OLD_SOURCE = '980999cfa400aade3c134cc09573530675a25476'
OLD_EXE_SHA = '942a46398b5dcc27bccd4369fd45411f8550c601e6b28208d8a2e1dab4afa99f'
OPERATOR_SHA = '4063b045543b567c1922999eb8c16b2518fd60f5b24ca38113984f2b606b7ea3'
BODY = '28fb79351715c5837393add72c705a7dda8e7a4abd81468ece08380adaf21d3a'
HOST = 'host/installed/a5df8bd0de854216bdf68abcbbaf40960c0ef52db7eb7d06386882670434742d'
OLD_BOOT = 'boot/installed/43496b36176fda31182eb9f52154c6309279b4a4dbf43ae301dac56d0c47bd38'
ROOT_UNIT = 'conduit-host.service'
ROUTES = ['conduit-member-route-threebee-43496b.service',
          'conduit-member-route-motherbrain-43496b.service']
QUEUE = {'entry': 'yellow-output', 'host_id': 'esp32/efuse/b0a732286178',
         'front_port_id': 'frame', 'direction': 'Input', 'track': 'payload',
         'expected_item_capacity': 1, 'expected_byte_capacity': 268,
         'selected_item_capacity': 1, 'selected_byte_capacity': 651}

def need(ok, message):
    if not ok:
        raise RuntimeError(message)

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def data(path):
    return json.loads(Path(path).read_bytes())

def run(args, timeout=90):
    r = subprocess.run([str(a) for a in args], capture_output=True, timeout=timeout)
    need(r.returncode == 0, 'command refused: ' + str(args[0]) + ' ' + str(args[1:4])
         + '; nonzero exit; no automatic retry')
    return r.stdout

def service(unit):
    raw = run(['systemctl', '--user', 'show', '--all', unit, '-p', 'Id', '-p', 'ActiveState',
               '-p', 'MainPID', '-p', 'UnitFileState', '-p', 'ExecStart', '-p', 'ExecStop',
               '-p', 'KillSignal', '-p', 'FragmentPath', '-p', 'DropInPaths', '-p', 'Restart'])
    properties = dict(l.split('=', 1) for l in raw.decode().splitlines() if '=' in l)
    properties.setdefault('ExecStop', '')
    properties.setdefault('DropInPaths', '')
    return properties

def operator():
    need(sha(HERE / 'operator.py') == OPERATOR_SHA, 'accepted operator script changed')
    spec = importlib.util.spec_from_file_location('exact_blue_operator', HERE / 'operator.py')
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m

def status(executable):
    return json.loads(run([executable, 'body', 'status', '--state-dir', STATE, '--json']))

def truth(value, held):
    need(value['biography']['body_id'] == BODY and value['biography']['body']['state'] == 'Lulled',
         'Body is not original Lulled Body')
    need(value['realization'] is None and value['recovered_source_claim_unresolved'] is held,
         'unexpected realization/recovered claim status')

def image_invariants(old, new):
    need(set(old) == set(new), 'IMAGE shape changed')
    for section in ['manifest', 'payload']:
        allowed = {'build_id', 'image_id', 'source_identity'} if section == 'manifest' else {'build_id'}
        need(set(old[section]) == set(new[section]), 'IMAGE fields changed')
        need(all(old[section][k] == new[section][k] for k in old[section] if k not in allowed),
             'original IMAGE resource/configuration bounds changed')
    need(new['manifest']['source_identity'] == SOURCE, 'wrong new source')

def preflight(request_path, prepare_result):
    need(os.uname().nodename.split('.')[0] == 'victus', 'run only on Victus')
    op = operator()
    need(sha(HERE / 'case-recovery-plan.json') == op.CASE_SHA, 'case changed')
    i = data(STATE / 'installation.json')
    oldexe = Path(i['product_executable'])
    need(sha(oldexe) == OLD_EXE_SHA and sha('/proc/162093/exe') == OLD_EXE_SHA, 'old executable mismatch')
    need(i['host_id'] == HOST and i['release_source_identity'] == OLD_SOURCE, 'old Host/source mismatch')
    runtime = data(STATE / 'runtime.json')
    need(runtime['process_id'] == 162093 and runtime['boot_id'] == OLD_BOOT
         and runtime['body_id'] == BODY and runtime['host_id'] == HOST, 'current Root CAS mismatch')
    units = {u: service(u) for u in [ROOT_UNIT, *ROUTES]}
    u = units[ROOT_UNIT]
    need(u['ActiveState'] == 'active' and u['MainPID'] == '162093'
         and u['UnitFileState'] == 'enabled' and u['KillSignal'] == '15' and not u['ExecStop'],
         'Root service preconditions changed')
    sig = next(l.split()[1] for l in Path('/proc/162093/status').read_text().splitlines()
               if l.startswith('SigCgt:'))
    need(not int(sig, 16) & (1 << 14), 'Root SIGTERM handler differs from reviewed stop')
    for fd in Path('/proc/162093/fd').iterdir():
        try:
            target = os.readlink(fd)
        except FileNotFoundError:
            continue
        need(not target.startswith(('/dev/tty', '/dev/serial/')), 'old Root owns a TTY; stop not released')
    for route in ROUTES:
        need(units[route]['ActiveState'] == 'active' and units[route]['Restart'] == 'no'
             and units[route]['UnitFileState'] == 'transient', 'old434 route changed')
    case, baseline = op.inputs()
    for name, expected in baseline.items():
        need(sha(STATE / name) == expected, 'protected preflight CAS mismatch: ' + name)
    need(not os.path.lexists(STATE.parent / op.OP), 'manual operator already prepared/spent')
    need(not any(os.path.lexists(STATE / n) for n in op.PENDING), 'pending request/journal')
    policy = data(STATE / 'source-check-selection.json')
    need(all(policy.get(k) == [] for k in ['plays', 'returns', 'admissions', 'continuation_outputs']),
         'one-shot policy action present')
    # No source/device action is inferred from the diagnostic claim's contents.
    truth(status(oldexe), True)
    release_path = STAGE / 'reviewed-release/reviewed-artifact.json'
    release = data(release_path)
    image_path = STAGE / 'prepared-image/image.json'
    image = data(image_path)
    need(release['source_identity'] == SOURCE, 'new release source mismatch')
    files = [f for f in release['files'] if Path(f['path']).name.startswith('conduit-')]
    need(len(files) == 1, 'ambiguous reviewed executable')
    exe = release_path.parent / files[0]['path']
    op.no_links(exe)
    need(exe.is_file() and 0 < exe.stat().st_size <= 67108864
         and 'sha256:' + sha(exe) == files[0]['sha256'], 'new executable mismatch')
    need(data(STAGE / 'prepared-image/image.json')['manifest']['source_identity'] == SOURCE,
         'unreviewed IMAGE')
    image_invariants(i['selected_host_configuration']['host_image'], image)
    need((STAGE / 'root.host.conduit').read_text() == i['selected_host_configuration']['source'],
         'checked configuration source bytes changed')
    preparation = STAGE / 'qualification-state/source-policy-preparation.json'
    need(preparation.is_file() and bool(data(preparation)), 'reviewed prepare-only evidence missing')
    op.no_links(prepare_result)
    terminal = data(prepare_result)
    need(terminal.get('returncode') == 0 and terminal.get('source_identity') == SOURCE
         and terminal.get('executable_sha256') == sha(exe)
         and terminal.get('preparation_sha256') == sha(preparation),
         'successful terminal prepare receipt must bind exact observed executable and preparation')
    op.no_links(request_path)
    request = data(request_path)
    expected = {'original_policy_sha256': 'sha256:' + sha(STATE / 'source-check-selection.json'),
                'previous_release_bundle_sha256': policy['release_bundle_sha256'],
                'installed_release_bundle_sha256': release['bundle_sha256'],
                'expected': data(STATE / 'body/biography.json'), 'retire_actions': False,
                'configuration_pin_replacements': [], 'local_fore_queue_replacements': [QUEUE]}
    need(request == expected, 'queue CAS request differs from exact reviewed boundary')
    # Read-only preview checks every file, link and special entry before stopping.
    full = op.inventory(STATE)
    size = sum((STATE / n).stat().st_size for n in full)
    # One old-state archive + standard release copy + conservative 256MiB margin.
    required = size + exe.stat().st_size * 2 + 256 * 1024**2
    need(shutil.disk_usage(STATE).free >= required, 'insufficient measured maintenance headroom')
    pinned_paths = [Path(__file__).absolute(), HERE / 'operator.py', release_path, image_path,
                    STAGE / 'root.host.conduit', exe, preparation, Path(request_path).absolute(),
                    HERE / 'operator-decision.REVIEW-REQUIRED.json', HERE / 'MANIFEST.json',
                    Path(prepare_result).absolute()]
    # All local evidence/linked files reviewed now; operator verifies original317a pins again.
    pinned_paths += list((HERE / 'evidence').iterdir()) + list((HERE / 'linked-evidence').iterdir())
    pinned_paths += [HERE / 'linked-evidence-manifest.json', HERE / 'extra-baseline.json',
                     HERE / 'case-current-protected-baseline.json', HERE / 'case-recovery-plan.json']
    for p in pinned_paths:
        op.no_links(p)
    pins = {str(p): sha(p) for p in pinned_paths}
    unit_files = {}
    for unit, properties in units.items():
        for path in [properties['FragmentPath'], *properties['DropInPaths'].split()]:
            if path:
                unit_files[path] = sha(path)
    return {'schema': 'conduit.operator/exact-current-root-upgrade@1', 'source': SOURCE,
            'case_sha256': op.CASE_SHA, 'operator_sha256': OPERATOR_SHA, 'pins': pins,
            'units_before': units, 'unit_files': unit_files, 'request': str(Path(request_path).absolute()),
            'prepare_result': str(Path(prepare_result).absolute()),
            'new_executable': str(exe), 'new_bundle': release['bundle_sha256'],
            'original_configuration': i['selected_host_configuration'], 'old_body_state': i['body_state'],
            'old_policy': policy, 'full_state_inventory': full,
            'measured_archive_bytes': size, 'required_free_bytes': required,
            'successful_prepare_terminal_reviewed': True, 'execution_authorized_by_file': False}

def execute(review):
    op = operator()
    need(preflight(review['request'], review['prepare_result']) == review,
         'fresh read-only preflight differs from reviewed pins')
    run_dir = HERE / ('upgrade-once-' + review['new_bundle'].removeprefix('sha256:'))
    op.newdir(run_dir)  # Any partial/spent directory prevents automatic replay.
    op.put(run_dir / 'review.json', json.dumps(review, sort_keys=True).encode())
    old_i = data(STATE / 'installation.json')
    original = review['full_state_inventory']
    protected = {n: h for n, h in original.items() if n.startswith('body/') or n == 'control.token'}
    effects = False
    def logged(name, command, timeout=90):
        r = subprocess.run([str(x) for x in command], capture_output=True, timeout=timeout)
        op.put(run_dir / (name + '.stdout'), r.stdout)
        op.put(run_dir / (name + '.stderr'), r.stderr)
        need(r.returncode == 0, name + ' failed; inspect private logs; no replay')
        return r.stdout
    def check_protected():
        need(all(sha(STATE / n) == h for n, h in protected.items()), 'protected keys/history/Body changed')
    def terminate(signum, frame):
        raise InterruptedError('controller interrupted by signal ' + str(signum))
    previous_term = signal.signal(signal.SIGTERM, terminate)
    try:
        effects = True
        # Local forwarding units only; no SSH/device commands on either Pi.
        for index, unit in enumerate([*ROUTES, ROOT_UNIT]):
            logged('stop-' + str(index), ['systemctl', '--user', 'stop', unit])
        need(all(service(u)['ActiveState'] == 'inactive' for u in [ROOT_UNIT, *ROUTES]),
             'Root/routes not stopped')
        need(service(ROOT_UNIT)['UnitFileState'] == 'enabled', 'startup enablement changed on stop')
        need(not Path('/proc/162093').exists(), 'old Owner survived stop')
        need(op.inventory(STATE) == original, 'state changed during maintenance stop')
        logged('exact-claim-operator', [sys.executable, HERE / 'operator.py', '--state', STATE,
               '--evidence', HERE / 'evidence', '--operator-decision',
               HERE / 'operator-decision.REVIEW-REQUIRED.json', '--execute-reviewed-case', op.CASE_SHA])
        archive = STATE.parent / op.OP
        need(not (STATE / op.CLAIM).exists() and sha(archive / 'released-original-claim.json') == op.CLAIM_SHA,
             'exact claim release not verified')
        archived = data(archive / 'state-before-manifest.json')
        need(archived == original, 'operator archive does not cover exact full old state')
        need(op.inventory(archive / 'state-before') == original, 'full old-state archive verification failed')
        check_protected()
        exe = review['new_executable']
        common = [exe, 'host', 'service', 'install', STAGE / 'reviewed-release/reviewed-artifact.json',
                  '--host-image', STAGE / 'prepared-image/image.json', '--state-dir', STATE]
        logged('install-no-start', [*common, '--no-start'])
        need(service(ROOT_UNIT)['MainPID'] == '0', 'no-start install started Owner')
        installed = data(STATE / 'installation.json')
        need(installed['host_id'] == HOST and installed['body_state'] == old_i['body_state']
             and installed['release_source_identity'] == SOURCE
             and installed['release_bundle_sha256'] == review['new_bundle'], 'installed identity mismatch')
        selected = installed['selected_host_configuration']
        need(selected['source'] == review['original_configuration']['source']
             and selected['configuration_id'] == review['original_configuration']['configuration_id'],
             'selected configuration changed')
        image_invariants(review['original_configuration']['host_image'], selected['host_image'])
        need(data(STATE / 'source-check-selection.json') == review['old_policy'], 'install changed policy')
        check_protected()
        logged('queue-reselect', [exe, 'host', 'service', 'reselect-source', review['request'], '--state-dir', STATE])
        policy = json.loads(json.dumps(review['old_policy']))
        policy['release_bundle_sha256'] = review['new_bundle']
        policy['expected'] = data(STATE / 'body/biography.json')
        matches = [p for a in policy['local_fore_adapters'] if a['entry'] == QUEUE['entry']
                   and a['host_id'] == QUEUE['host_id'] for p in a['ports']
                   if all(p[k] == QUEUE[k] for k in ['front_port_id', 'direction', 'track'])]
        need(len(matches) == 1 and matches[0]['item_capacity'] == 1 and matches[0]['byte_capacity'] == 268,
             'old queue boundary mismatch')
        matches[0]['byte_capacity'] = 651
        need(data(STATE / 'source-check-selection.json') == policy, 'reselect changed unrelated policy')
        check_protected()
        need(not (STATE / op.CLAIM).exists() and not any(os.path.lexists(STATE / n) for n in op.PENDING),
             'claim/request/journal present before activation')
        # Standard verified installation activation, not an ad hoc service process.
        logged('activate-standard', common)
        for _ in range(80):
            runtime = data(STATE / 'runtime.json')
            unit = service(ROOT_UNIT)
            if unit['ActiveState'] == 'active' and unit['MainPID'] != '0' \
                    and runtime['process_id'] == int(unit['MainPID']) and runtime['boot_id'] != OLD_BOOT:
                break
            time.sleep(.25)
        else:
            raise RuntimeError('fresh standard Owner not verified')
        need(runtime['host_id'] == HOST and runtime['body_id'] == BODY
             and runtime['release_bundle_sha256'] == review['new_bundle'], 'fresh Owner identity mismatch')
        need(sha(Path('/proc') / unit['MainPID'] / 'exe') == sha(exe), 'running executable mismatch')
        current = status(exe)
        truth(current, False)
        check_protected()
        need(data(STATE / 'source-check-selection.json') == policy and not (STATE / op.CLAIM).exists(),
             'post-start policy/claim mismatch')
        need(unit['UnitFileState'] == review['units_before'][ROOT_UNIT]['UnitFileState'], 'enabled state changed')
        need(all(service(u)['ActiveState'] == 'inactive' for u in ROUTES), 'old routes restarted')
        op.put(run_dir / 'fresh-status.json', json.dumps(current, sort_keys=True).encode())
        op.put(run_dir / 'success.json', json.dumps({'boot': runtime['boot_id'], 'pid': runtime['process_id'],
               'body_id': BODY, 'host_id': HOST, 'bundle': review['new_bundle'], 'source': SOURCE,
               'old_state_archive': str(archive), 'routes_paused': ROUTES,
               'device_effects': False, 'source_play_proved': False}, sort_keys=True).encode())
        print('Fresh same-Body Root verified Lulled. Old434 routes remain paused; no device/Source Play effects.')
    except BaseException:
        if effects:
            # No old executable/Root rollback and no automatic transaction retry.
            r = subprocess.run(['systemctl', '--user', 'stop', ROOT_UNIT], capture_output=True, timeout=90)
            op.put(run_dir / 'failure-stop.stdout', r.stdout)
            op.put(run_dir / 'failure-stop.stderr', r.stderr)
            op.put(run_dir / 'FAILED-REQUIRES-MANUAL-REVIEW', b'No automatic retry or old Root rollback.\n')
        raise
    finally:
        signal.signal(signal.SIGTERM, previous_term)

if __name__ == '__main__':
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--stage-review', type=Path)
    mode.add_argument('--execute-review', type=Path)
    parser.add_argument('--queue-request', type=Path)
    parser.add_argument('--prepare-result', type=Path)
    parser.add_argument('--successful-prepare-reviewed', action='store_true')
    parser.add_argument('--review-sha256')
    args = parser.parse_args()
    if args.stage_review:
        need(args.queue_request is not None and args.prepare_result is not None and args.successful_prepare_reviewed,
             'exact request and successful terminal prepare-only review required')
        result = preflight(args.queue_request, args.prepare_result)
        operator().put(args.stage_review, json.dumps(result, sort_keys=True).encode())
        print('Inert review staged; SHA256 ' + sha(args.stage_review) + '; no maintenance/device effects')
    else:
        need(args.review_sha256 is not None and sha(args.execute_review) == args.review_sha256,
             'separately approved exact review SHA required')
        execute(data(args.execute_review))
