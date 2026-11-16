"""Run the production transaction against copied actual private state only."""
import fcntl
import importlib.util
import json
import multiprocessing
import os
from pathlib import Path
import shutil
import tempfile

HERE = Path(__file__).absolute().parent
spec = importlib.util.spec_from_file_location('recovery_operator', HERE / 'operator.py')
op = importlib.util.module_from_spec(spec)
spec.loader.exec_module(op)
DECISION = json.loads((HERE / 'operator-decision.REVIEW-REQUIRED.json').read_text())

def prepared(base, name):
    parent = base / name
    parent.mkdir(mode=0o700)
    root = parent / 'host'
    shutil.copytree(HERE / 'actual-state-copy', root)
    for name in ['host-owner.lock', 'body-owner.lock']:
        (root / name).touch(mode=0o600)
    return root

def fixture_stopped(root):
    # OS checks are deliberately separate: actual PID162093 belongs to Victus.
    assert root.parent.parent.name.startswith('actual-blue-state-qualification-')

def run(root, evidence=None, observe=fixture_stopped, fail=None):
    return op.transaction(root, evidence or HERE / 'evidence', DECISION, observe=observe, fail=fail)

class Crash:
    def __init__(self, point):
        self.point = point
    def __eq__(self, name):
        if self.point == name:
            os._exit(73)
        return False

def crash_child(root, point):
    run(root, fail=Crash(point))

def qualify():
    cases = []
    with tempfile.TemporaryDirectory(prefix='actual-blue-state-qualification-', dir=HERE) as tmp:
        base = Path(tmp)
        for point in op.BOUNDARIES + [None]:
            root = prepared(base, point or 'complete')
            before = op.inventory(root)
            if point:
                process = multiprocessing.Process(target=crash_child, args=(root, point))
                process.start()
                process.join(20)
                assert not process.is_alive() and process.exitcode == 73, point
            else:
                run(root)
            archive = root.parent / op.OP
            released = not (root / op.CLAIM).exists()
            assert released == (point in ['claim-renamed', 'release-durable', None])
            for name, digest in before.items():
                if name != op.CLAIM:
                    assert op.digest(op.read(root / name)) == digest
            if released:
                assert op.digest(op.read(archive / 'state-before' / op.CLAIM)) == op.CLAIM_SHA
                assert op.digest(op.read(archive / 'released-original-claim.json')) == op.CLAIM_SHA
                decision = json.loads(op.read(archive / 'terminal-manual-decision.json'))
                assert decision['completed'] is False and decision['replay_old_request'] is False
                assert decision['evidence']['fresh_return_receipt_sha256.bin']
            else:
                assert op.digest(op.read(root / op.CLAIM)) == op.CLAIM_SHA
            try:
                run(root)
                raise AssertionError('prepared/spent operation replayed')
            except RuntimeError:
                pass
            cases.append({'case': point or 'complete', 'passed': True, 'claim_held': not released})
        refusal_names = ['host-lock', 'body-lock', 'live-owner', 'claim-mismatch',
                         'protected-mismatch', 'runtime-mismatch', 'unknown-mismatch',
                         'pending-request', 'owner-journal', 'admission-journal',
                         'state-symlink', 'ancestor-symlink', 'hardlink', 'evidence-mismatch',
                         'one-shot-policy', 'late-state-change']
        for name in refusal_names:
            root = prepared(base, name)
            evidence = HERE / 'evidence'
            observer = fixture_stopped
            held = None
            if name.endswith('-lock'):
                held = (root / (name.replace('-lock', '-owner.lock'))).open('r+')
                fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)
            elif name == 'live-owner':
                def observer(root):
                    raise RuntimeError('Owner service active')
            elif name in ['claim-mismatch', 'protected-mismatch', 'runtime-mismatch', 'unknown-mismatch']:
                target = {'claim-mismatch': op.CLAIM, 'protected-mismatch': 'body/biography.json',
                          'runtime-mismatch': 'runtime.json',
                          'unknown-mismatch': 'blue-return-103/after-selection-unknown.bin'}[name]
                with (root / target).open('ab') as f:
                    f.write(b'corrupt')
            elif name in ['pending-request', 'owner-journal', 'admission-journal']:
                target = op.PENDING[['pending-request', 'owner-journal', 'admission-journal'].index(name)]
                (root / target).touch()
            elif name == 'state-symlink':
                (root / 'extra-link').symlink_to(root / op.CLAIM)
            elif name == 'ancestor-symlink':
                (root.parent / 'alias').symlink_to(root, target_is_directory=True)
                root = root.parent / 'alias'
            elif name == 'hardlink':
                os.link(root / op.CLAIM, root / 'extra-hardlink')
            elif name == 'evidence-mismatch':
                evidence = root.parent / 'bad-evidence'
                shutil.copytree(HERE / 'evidence', evidence)
                (evidence / 'fresh_return_receipt_sha256.bin').write_bytes(b'wrong')
            elif name == 'one-shot-policy':
                p = root / 'source-check-selection.json'
                d = json.loads(p.read_text()); d['plays'] = [{'forbidden': 'replay'}]
                p.write_text(json.dumps(d))
            elif name == 'late-state-change':
                calls = [0]
                def observer(root):
                    fixture_stopped(root)
                    calls[0] += 1
                    if calls[0] == 4:
                        (root / 'unexpected-new-work').write_bytes(b'new request')
            try:
                try:
                    run(root, evidence=evidence, observe=observer)
                    raise AssertionError('refusal gate accepted: ' + name)
                except (RuntimeError, BlockingIOError):
                    pass
                assert (root / op.CLAIM).exists(), name
                cases.append({'case': name, 'passed': True, 'claim_held': True})
            finally:
                if held:
                    held.close()
    return {'scope': 'Production filesystem transaction; exact actual protected32 + runtime + UNKNOWN copied from Victus. Real flock contention and subprocess exits. Stopped-service/process observation simulated in disposable copies; no power-loss/device/production-service proof.',
            'passed': len(cases), 'cases': cases, 'live_effects': False}

if __name__ == '__main__':
    print(json.dumps(qualify(), indent=2))
