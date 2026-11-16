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
CASE_SHA = '317a66450847cd81051dbd6bdfeae0545c7190b6d137ab789c71d5d69cb1633d'
BASE_SHA = '6f45bff85ba2605c0174a853bd3ebe8af12a020231a4b811984c035df2711cc1'
EXTRA_SHA = 'f9cf3370b0a1f06836b93acee403a5c7b8717e6e9ece3f034da81da3723fc8b5'
LINKED_SHA = '8580cb0ab655b93385d9cc96b3595825a0350d323d19350981b392f84e86dc74'
CLAIM_SHA = '7b41ec462dce31ea8b3314dd560a0c07c49b78e28b4278a234658c47dda19c2e'
CLAIM = 'source-execution/claimed.json'
OP = 'blue-physical-replacement-' + CLAIM_SHA
PENDING = ['source-execution/pending.json', 'body/owner-transaction.json',
           'body/admission-transaction.json']
BOUNDARIES = ['archive-created', 'snapshot-durable', 'evidence-durable',
              'decision-durable', 'final-checks', 'claim-renamed', 'release-durable']

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
        except PermissionError as e:
            raise RuntimeError('cannot verify same-user process custody') from e

def inputs():
    case_bytes = read(HERE / 'case-recovery-plan.json')
    base_bytes = read(HERE / 'case-current-protected-baseline.json')
    require(digest(case_bytes) == CASE_SHA and digest(base_bytes) == BASE_SHA, 'case/baseline drift')
    extra = read(HERE / 'extra-baseline.json')
    require(digest(extra) == EXTRA_SHA, 'runtime/UNKNOWN baseline drift')
    case = json.loads(case_bytes)
    return case, dict(json.loads(base_bytes)['protected'], **json.loads(extra))

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
        'operation': OP, 'outcome': 'UNKNOWN_RETIRED_BY_PHYSICAL_REPLACEMENT',
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
        required = ['reset_audit_sha256', 'identity_review_sha256',
                    'physical_update_receipt_sha256', 'fresh_return_receipt_sha256']
        blobs = {}
        for key in required:
            b = read(Path(evidence) / (key + '.bin'))
            require(digest(b) == case['inputs'][key], 'evidence mismatch: ' + key)
            blobs[key + '.bin'] = b
        linked_bytes = read(HERE / 'linked-evidence-manifest.json')
        require(digest(linked_bytes) == LINKED_SHA, 'linked evidence manifest drift')
        linked = json.loads(linked_bytes)
        for name, item in linked.items():
            b = read(HERE / 'linked-evidence' / name)
            require(digest(b) == item['sha256'], 'old linked evidence mismatch')
            blobs['old-linked-' + name] = b
        blobs['old-linked-manifest.json'] = linked_bytes
        unknown = 'blue-return-103/after-selection-unknown.bin'
        require(unknown in all_before, 'original UNKNOWN evidence missing')
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
        put(archive / 'case317a.json', read(HERE / 'case-recovery-plan.json'))
        step('evidence-durable')
        receipt = dict(decision, original_claim_sha256=CLAIM_SHA,
                       body_id=case['body_id'], part_id=case['part_id'], host_id=case['host_id'],
                       old_boot=case['old_member_boot'], fresh_boot=case['authenticated_replacement_boot'],
                       completed=False, started=False, normal_protocol_retirement=False,
                       original_unknown_sha256=all_before[unknown],
                       protected_baseline_sha256=BASE_SHA,
                       state_manifest_sha256=digest(read(archive / 'state-before-manifest.json')),
                       evidence={n: digest(b) for n, b in blobs.items()},
                       script_sha256=digest(read(HERE / 'operator.py')))
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
