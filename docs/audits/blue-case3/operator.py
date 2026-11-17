#!/usr/bin/env python3
"""One exact, separately approved, stopped-owner recovery. No device/control RPC."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess

HERE = Path(__file__).absolute().parent
CASE_SHA = '2b81eab9f484a130d5a8beb87a7f644dfba8e43240246bb6e255be0f47cd6d3d'
CLAIM_SHA = 'ff9aa4cee4f649ae3aa74294425fa8f199b143a3376203a4a7a8d52377b1cbb7'
CLAIM = 'source-execution/claimed.json'
OP = 'blue-physical-replacement-' + CLAIM_SHA
PENDING = ['source-execution/pending.json', 'body/owner-transaction.json',
           'body/admission-transaction.json']
BOUNDARIES = ['archive-created', 'snapshot-durable', 'evidence-durable',
              'decision-durable', 'final-checks', 'claim-renamed', 'release-durable']
OS_SCOPE_OBSERVATIONS = {}

def infrastructure_exception(identity, manager_pid):
    """Trusted local OS case only; comm is not hostile-code attestation."""
    require(identity['uid'] == os.getuid(), 'foreign process exception refused')
    comm = identity['comm']
    accepted = ((comm == 'systemd' and identity['pid'] == manager_pid)
                or (comm == '(sd-pam)' and identity['ppid'] == manager_pid)
                or comm in ['ssh-agent', 'sshd-session'])
    require(accepted, 'inaccessible unrecognized/possible Conduit process; custody unknown')
    return dict(identity, observation='descriptor visibility unavailable',
                basis='trusted OS systemd/PAM/SSH infrastructure; not all-FD proof')

def process_identity(proc):
    text = (proc / 'stat').read_text()
    rest = text[text.rfind(')') + 2:].split()
    return {'pid': int(proc.name), 'uid': proc.stat().st_uid,
            'comm': (proc / 'comm').read_text().strip(),
            'ppid': int(rest[1]), 'start_ticks': int(rest[19])}

def digest(b):
    return hashlib.sha256(b).hexdigest()

def require(ok, reason):
    if not ok:
        raise RuntimeError(reason)

def no_links(path):
    path = Path(os.path.abspath(path))
    for p in [*reversed(path.parents), path]:
        require(not p.is_symlink(), 'symlink path refused: ' + str(p))
    return path

def read(path):
    no_links(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        s = os.fstat(fd)
        require(stat.S_ISREG(s.st_mode) and s.st_uid == os.getuid(), 'not owned regular file')
        require(s.st_nlink == 1, 'hardlinked file refused')
        with os.fdopen(os.dup(fd), 'rb') as f:
            return f.read()
    finally:
        os.close(fd)

def syncdir(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)

def newdir(path):
    os.mkdir(path, 0o700)
    syncdir(path.parent)

def put(path, data):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        with os.fdopen(os.dup(fd), 'wb') as f:
            f.write(data)
            f.flush()
            os.fsync(f.fileno())
    finally:
        os.close(fd)
    syncdir(path.parent)

def inventory(root):
    result = {}
    for directory, dirs, files in os.walk(root, followlinks=False):
        for name in dirs + files:
            p = Path(directory) / name
            no_links(p)
            s = p.lstat()
            require(s.st_uid == os.getuid(), 'foreign-owned state')
            if stat.S_ISDIR(s.st_mode):
                continue
            if stat.S_ISSOCK(s.st_mode) and p == root / 'control.sock':
                # Stale endpoint is not executable authority and cannot be copied.
                continue
            require(stat.S_ISREG(s.st_mode), 'special state entry refused')
            result[str(p.relative_to(root))] = digest(read(p))
    return result

def stopped(root):
    # Read-only OS checks. This script never stops/disables/restarts a service.
    r = subprocess.run(['systemctl', '--user', 'show', 'conduit-host.service',
                        '-p', 'ActiveState', '-p', 'MainPID'], check=True,
                       capture_output=True, text=True)
    properties = dict(l.split('=', 1) for l in r.stdout.splitlines() if '=' in l)
    require(properties.get('ActiveState') == 'inactive' and properties.get('MainPID') == '0',
            'Owner service must already be inactive with MainPID=0')
    old_pid = json.loads(read(root / 'runtime.json'))['process_id']
    require(not Path('/proc', str(old_pid)).exists(), 'recorded Owner PID still exists')
    own = os.getpid()
    manager_pid = int(subprocess.check_output(['systemctl', 'show', 'user@' + str(os.getuid()) + '.service',
                                              '-p', 'MainPID', '--value'], text=True).strip())
    require(manager_pid > 0, 'user service manager identity unavailable')
    serial = os.path.realpath('/dev/serial/by-id/usb-Silicon_Labs_CP2102_USB_to_UART_Bridge_Controller_0001-if00-port0')
    for proc in Path('/proc').iterdir():
        if not proc.name.isdigit() or int(proc.name) == own:
            continue
        try:
            if proc.stat().st_uid != os.getuid():
                continue
            # Also reject a peer whose working directory is inside the state.
            handles = [proc / 'cwd', proc / 'exe', *(proc / 'fd').iterdir()]
            for handle in handles:
                try:
                    target = os.readlink(handle).removesuffix(' (deleted)')
                except FileNotFoundError:
                    continue
                require(target != str(root) and not target.startswith(str(root) + '/'),
                        'another same-user process owns a state descriptor/cwd')
                require(target != serial, 'another process owns Blue serial')
        except FileNotFoundError:
            continue
        except PermissionError:
            try:
                observation = infrastructure_exception(process_identity(proc), manager_pid)
            except FileNotFoundError:
                continue
            OS_SCOPE_OBSERVATIONS[str(observation['pid'])] = observation

def inputs():
    case_bytes = read(HERE / 'case-v3.json')
    require(digest(case_bytes) == CASE_SHA, 'case3 drift')
    case = json.loads(case_bytes)
    baseline = read(HERE / 'baseline.json')
    require(digest(baseline) == case['baseline_sha256'], 'current baseline drift')
    return case, json.loads(baseline)['protected']

RESET_RECEIPT = HERE / 'post-attempt-reset-receipt.json'

def checked_reset(case):
    raw = read(RESET_RECEIPT)
    r = json.loads(raw)
    require(r['case_sha256'] == CASE_SHA and r['claim_sha256'] == CLAIM_SHA,
            'reset belongs to another attempt')
    require(r['controller_sha256'] == digest(read(HERE / 'recover-once.py')),
            'reset controller changed')
    require(r['tool_sha256'] == case['tool_sha256'] and r['physical_mac'] == case['physical_mac']
            and r['before_flash_sha256'] == case['before_flash_sha256']
            and r['read_flash_exit'] == 0 and r['hard_reset_exit'] == 0
            and r['root_stopped_pid'] == case['root_pid']
            and r['serial_closed'] is True and r['after_this_claim_attempt'] is True,
            'post-attempt qualified reset not established')
    return raw

def transaction(root, evidence, decision, observe=stopped, fail=None):
    """observe/fail are qualification seams; CLI always uses real stopped()."""
    root = no_links(root)
    require(root.is_dir() and root.stat().st_uid == os.getuid()
            and root.stat().st_mode & 0o077 == 0, 'state not private')
    require(root.parent.stat().st_uid == os.getuid()
            and root.parent.stat().st_mode & 0o077 == 0, 'archive parent not private')
    case, baseline = inputs()
    # Explicit operator decision is separate from diagnostic saved claims.
    require(decision == {
        'case_sha256': CASE_SHA, 'accept_reset_assumptions': True,
        'operation': OP, 'outcome': 'UNKNOWN_RETIRED_BY_EXPLICIT_QUALIFIED_RESET',
        'authorize_exact_claim_release': True, 'replay_old_request': False,
    }, 'exact separately reviewed operator decision required')
    archive = root.parent / OP
    require(not os.path.lexists(archive), 'operation already prepared/spent; manual inspection required')
    observe(root)
    locks = []
    try:
        # Same order as ordinary reselect; Rust File::try_lock is flock on Linux.
        # Require existing lock inodes; never replace/create lock files.
        for name in ['host-owner.lock', 'body-owner.lock']:
            p = root / name
            read(p)
            fd = os.open(p, os.O_RDWR | os.O_NOFOLLOW)
            locks.append(fd)
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        observe(root)
        def checked():
            observe(root)
            require(not any(os.path.lexists(root / n) for n in PENDING), 'pending request/Body journal')
            for name, expected in baseline.items():
                require(digest(read(root / name)) == expected, 'protected baseline mismatch: ' + name)
            require(digest(read(root / CLAIM)) == CLAIM_SHA, 'claim mismatch')
            policy = json.loads(read(root / 'source-check-selection.json'))
            for action in ['plays', 'returns', 'admissions', 'continuation_outputs']:
                require(policy.get(action) == [], 'saved policy can replay one-shot action: ' + action)
        checked()
        all_before = inventory(root)
        blobs = {}
        for name, expected in case['evidence'].items():
            raw = read(Path(evidence) / name)
            require(digest(raw) == expected, 'pinned historical evidence mismatch: ' + name)
            blobs[name] = raw
        blobs['post-attempt-reset-receipt.json'] = checked_reset(case)
        reset = json.loads(blobs['post-attempt-reset-receipt.json'])
        required_logs = {'qualified-read-flash.stdout', 'qualified-read-flash.stderr',
                         'qualified-final-hard-reset.stdout', 'qualified-final-hard-reset.stderr',
                         'before-reset-flash.bin', 'review.json'}
        require(required_logs <= set(reset['logs']), 'actual reset evidence files missing')
        require(reset['logs']['before-reset-flash.bin'] == case['before_flash_sha256'],
                'reset readback does not match exact prior seed flash')
        for name, expected in reset['logs'].items():
            require(Path(name).name == name, 'reset evidence path not finite')
            raw = read(HERE / 'effect-once' / name)
            require(digest(raw) == expected, 'reset evidence changed')
            blobs['reset-' + name] = raw
        unknown = 'source-execution/claimed.json'
        require(unknown in all_before, 'original uncertain claim missing')
        def step(name):
            if fail == name:
                raise InterruptedError(name)
        newdir(archive)
        step('archive-created')
        snapshot = archive / 'state-before'
        newdir(snapshot)
        for name, expected in sorted(all_before.items()):
            dst = snapshot / name
            for parent in reversed(dst.parent.parents):
                if parent == snapshot or snapshot in parent.parents:
                    if not parent.exists():
                        newdir(parent)
            if not dst.parent.exists():
                newdir(dst.parent)
            b = read(root / name)
            require(digest(b) == expected, 'state changed during snapshot')
            put(dst, b)
        put(archive / 'state-before-manifest.json', json.dumps(all_before, sort_keys=True).encode())
        require(inventory(snapshot) == all_before, 'archive verification failed')
        syncdir(snapshot)
        syncdir(archive)
        step('snapshot-durable')
        newdir(archive / 'evidence')
        for name, b in blobs.items():
            put(archive / 'evidence' / name, b)
        put(archive / 'case-v3.json', read(HERE / 'case-v3.json'))
        step('evidence-durable')
        receipt = dict(decision, original_claim_sha256=CLAIM_SHA,
                       body_id=case['body_id'], part_id=case['part_id'], host_id=case['host_id'],
                       old_boot=case['old_member_boot'], fresh_boot=case['authenticated_replacement_boot'],
                       completed=False, started=False, normal_protocol_retirement=False,
                       original_unknown_sha256=all_before[unknown],
                       protected_baseline_sha256=case['baseline_sha256'],
                       state_manifest_sha256=digest(read(archive / 'state-before-manifest.json')),
                       evidence={n: digest(b) for n, b in blobs.items()},
                       script_sha256=digest(read(HERE / 'operator.py')))
        receipt['os_scope'] = 'Stopped actual Root + ordinary exclusive locks + exact baseline; trusted local OS'
        receipt['unreadable_infrastructure_observations'] = list(OS_SCOPE_OBSERVATIONS.values())
        receipt['all_process_fd_visibility_claimed'] = False
        put(archive / 'terminal-manual-decision.json', json.dumps(receipt, sort_keys=True).encode())
        syncdir(archive)
        step('decision-durable')
        checked()
        require(inventory(root) == all_before, 'late full-state mismatch')
        step('final-checks')
        # THE LAST RELEASE EDGE. Both names share a filesystem; target is in an
        # exclusively created private directory. No parsing/replay of claim bytes.
        require(root.stat().st_dev == archive.stat().st_dev, 'cross-filesystem release refused')
        os.rename(root / CLAIM, archive / 'released-original-claim.json')
        step('claim-renamed')
        syncdir(root / 'source-execution')
        syncdir(archive)
        step('release-durable')
        for name, expected in all_before.items():
            if name != CLAIM:
                require(digest(read(root / name)) == expected, 'post-release unrelated state changed')
        require(digest(read(archive / 'released-original-claim.json')) == CLAIM_SHA,
                'released claim archive mismatch')
        return {'operation': OP, 'archive': str(archive), 'fresh_owner_required': True}
    finally:
        for fd in reversed(locks):
            fcntl.flock(fd, fcntl.LOCK_UN)
            os.close(fd)

if __name__ == '__main__':
    a = argparse.ArgumentParser(description=__doc__)
    a.add_argument('--state', required=True)
    a.add_argument('--evidence', required=True)
    a.add_argument('--operator-decision', required=True)
    a.add_argument('--execute-reviewed-case', required=True)
    args = a.parse_args()
    require(args.execute_reviewed_case == CASE_SHA, 'exact case confirmation required')
    require(os.uname().nodename.split('.')[0] == 'victus', 'wrong host')
    require(args.state == '/home/dancxjo/.local/state/conduit/host', 'wrong installed state')
    print(json.dumps(transaction(Path(args.state), args.evidence,
                                 json.loads(read(Path(args.operator_decision))))))
