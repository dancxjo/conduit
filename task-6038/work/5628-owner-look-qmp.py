"""Private external proof adapter. Default: synthetic self-test, never a guest.

Raw output stays private; review/redact before any publication. No credential,
invitation, provisioned ISO, environment, or full launch argv is archived.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import stat
import subprocess
import threading
import time

REAL = 'CONDUIT_NATIVE_COMPONENT_REALIZATION '
ACK = 'CONDUIT_NATIVE_OWNER_FACE '
FOCUS = 'CONDUIT_CRECHE_FOCUS '
REFUSE = 'CONDUIT_NATIVE_COMPONENT_SELECTION '
SERIAL_MAX = 2 * 1024 * 1024
RECORD_MAX = 512
COMMAND_MAX = 512
CAPTURE_MAX = 32


def require(ok, message):
    if not ok:
        raise AssertionError(message)


def private(path, directory=False):
    p = Path(path).absolute()
    m = p.lstat()
    require(not stat.S_ISLNK(m.st_mode), 'symlink private path')
    require(m.st_uid == os.getuid() and not m.st_mode & 0o077, 'private ownership/mode')
    require(stat.S_ISDIR(m.st_mode) if directory else stat.S_ISREG(m.st_mode), 'private path type')
    return p


def records(data):
    require(len(data) <= SERIAL_MAX, 'serial bound')
    # Ignore an incomplete trailing UART write, including partial UTF-8.
    end = data.rfind(b'\n')
    result = []
    for line in data[:end + 1].decode('utf-8').splitlines():
        for prefix in (REAL, ACK, FOCUS, REFUSE):
            if line.startswith(prefix):
                value = json.loads(line[len(prefix):])
                require(isinstance(value, dict), 'record object')
                result.append((prefix, value))
    require(len(result) <= RECORD_MAX, 'record bound')
    return result


def focus_matches(real, focus, named=None):
    face, show = real['face'], real['acknowledged_show']
    require(focus['face_id'] == face['identity'] and focus['face_revision'] == face['revision'], 'focus Face')
    require(focus['show_id'] == show['show_id'], 'focus Show')
    if named is not None:
        require((focus['action_id'], focus['argument_name']) == tuple(named), 'named focus')
        action = next((a for a in face['actions'] if a['identity'] == named[0]), None)
        require(action is not None and action['availability'] == 'Available', 'available action')
        require(named[1] is None or any(a['name'] == named[1] for a in action['arguments']), 'offered argument')
    b, d = focus['hit_bounds'], focus['display_bounds']
    require(1 <= focus['page'] <= focus['pages'] <= 64, 'page bounds')
    require(b is not None and b['width'] > 0 and b['height'] > 0, 'focus hit')
    require(b['x'] >= 0 and b['y'] >= 0 and b['x'] + b['width'] <= d['width'] and b['y'] + b['height'] <= d['height'], 'hit extent')


def acknowledged(rows, named=None):
    candidates = [(i, r) for i, (p, r) in enumerate(rows) if p == REAL]
    require(candidates, 'missing realization')
    i, r = candidates[-1]
    s, f = r['acknowledged_show'], r['face']
    require(r['owner_show_acknowledged'] is True, 'realization not acknowledged')
    require(s['face_id'] == f['identity'] and s['face_revision'] == f['revision'], 'realization Face binding')
    acknowledgements = [n for n in range(i + 1, len(rows)) if rows[n][0] == ACK and rows[n][1].get('status') == 'acknowledged' and rows[n][1].get('show_id') == s['show_id']]
    require(acknowledgements, 'missing matching acknowledgement')
    focuses = [a for p, a in rows[acknowledgements[-1] + 1:] if p == FOCUS]
    require(focuses, 'missing fresh focus')
    focus_matches(r, focuses[-1], named)
    return r, focuses[-1]


def verify_switch(before, fresh_rows, named):
    after, focus = acknowledged(fresh_rows, named)
    require(after['face'] == before['face'], 'changed complete Face')
    require(after['configuration']['recipe'] == before['configuration']['recipe'], 'changed recipe')
    for realization in (before, after):
        component = realization['configuration'].get('component')
        require(isinstance(component, dict), 'missing checked component')
        require(all(isinstance(component.get(k), str) and component[k].strip() for k in ('source_document_id', 'checked_plot_id', 'fore_id')), 'empty checked component identity')
    require(after['configuration']['component'] == before['configuration']['component'], 'changed checked component')
    require(after['acknowledged_show']['show_id'] != before['acknowledged_show']['show_id'], 'reused Show')
    require(after['owner_host'] == before['owner_host'], 'changed owner Host')
    require(isinstance(after['route_plan_id'], str) and bool(after['route_plan_id'].strip()), 'missing fresh route Plan')
    # Mask replacement can reseal its route Plan. Journey caller must bind this
    # Plan to the independent current owner wardrobe report/new Show, distinct
    # from the owner Body Plan preserved by wardrobe-only handoff.
    return after, focus


def verify_refusal(before, rows):
    require(any(p == REFUSE and r.get('status') == 'refused' and r.get('code') == 'owner-look-provisional-input' for p, r in rows), 'missing provisional refusal')
    require(not any(p in (REAL, ACK) for p, _ in rows), 'refusal replaced/acknowledged Show')
    for p, f in rows:
        if p == FOCUS:
            focus_matches(before, f)
    # Draft bytes are not emitted by this observer; visible captures + owner
    # snapshots must establish preservation. Never invent an edit receipt here.


def launch_tail(argv):
    # Owned virtual-display wrapper only; never archive its environment/Xauthority.
    if argv[:3] == ['xvfb-run', '-a', '--']:
        argv = argv[3:]
    require(argv[:6] == ['cargo', '+stable', 'xtask', 'make', 'conduitos', 'live-owner-boot'], 'unsupported entrance')
    return argv[6:]


class Session:
    def __init__(self, argv, cwd, output, qmp_path):
        tail = launch_tail(argv)
        flags = ('--spore', '--candidate-id', '--owner-forward', '--qmp-socket')
        require(len(tail) == 8 and sorted(tail[::2]) == sorted(flags), 'exact route arguments required')
        args = dict(zip(tail[::2], tail[1::2]))
        require(args['--qmp-socket'] == str(qmp_path), 'QMP argv binding')
        private(args['--spore'])
        parent = private(Path(qmp_path).parent, directory=True)
        require(Path(qmp_path).is_absolute() and len(str(qmp_path).encode()) <= 96 and ',' not in str(qmp_path) and not any(ord(c) < 32 for c in str(qmp_path)), 'QMP path bound')
        require(not Path(qmp_path).exists(), 'QMP path exists')
        require(parent != Path('/tmp'), 'private QMP parent')
        self.out = Path(output).absolute()
        require(not self.out.exists(), 'output exists')
        self.out.mkdir(mode=0o700)
        self.lock = threading.Lock()
        self.buffers = {'serial': bytearray(), 'stderr': bytearray()}
        self.error = None
        self.proc = self.sock = self.reader = None
        self.threads = []
        self.commands = self.captures = 0
        self.deadline = time.monotonic() + 600
        self.history = []
        self.qmp_path = Path(qmp_path)
        try:
            self.proc = subprocess.Popen(argv, cwd=cwd, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
            for name, stream in [('serial', self.proc.stdout), ('stderr', self.proc.stderr)]:
                t = threading.Thread(target=self.drain, args=(name, stream), daemon=True)
                self.threads.append(t)
                t.start()
            self.wait(lambda: self.qmp_path.exists(), 120)
            self.sock = socket.socket(socket.AF_UNIX)
            self.sock.settimeout(10)
            self.sock.connect(str(qmp_path))
            self.reader = self.sock.makefile('rb')
            require('QMP' in self.read(), 'QMP greeting')
            self.request('qmp_capabilities')
        except BaseException:
            self.close()
            raise

    def drain(self, name, stream):
        try:
            fd = os.open(self.out / (name + '.log'), os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, 'wb') as out:
                while True:
                    chunk = os.read(stream.fileno(), 4096)
                    if not chunk:
                        break
                    with self.lock:
                        require(len(self.buffers[name]) + len(chunk) <= SERIAL_MAX, name + ' bound')
                        self.buffers[name].extend(chunk)
                    out.write(chunk)
                    out.flush()
        except BaseException as e:
            self.error = e
            if self.proc is not None:
                try:
                    os.killpg(self.proc.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass

    def rows(self):
        if self.error:
            raise self.error
        with self.lock:
            return records(bytes(self.buffers['serial']))

    def wait(self, predicate, seconds=120):
        deadline = min(self.deadline, time.monotonic() + seconds)
        while time.monotonic() < deadline:
            if self.error:
                raise self.error
            require(self.proc.poll() is None, 'guest/entrance exited')
            if predicate():
                return
            time.sleep(.05)  # Passive only; callers must not put actions here.
        raise TimeoutError('passive observation deadline')

    def read(self):
        line = self.reader.readline(1024 * 1024 + 1)
        require(line and len(line) <= 1024 * 1024 and line.endswith(b'\n'), 'QMP reply bound')
        value = json.loads(line)
        with (self.out / 'qmp-replies.jsonl').open('a') as log:
            log.write(json.dumps(value) + '\n')
        return value

    def request(self, command, arguments=None):
        require(time.monotonic() < self.deadline, 'session deadline')
        self.commands += 1
        require(self.commands <= COMMAND_MAX, 'QMP command bound')
        payload = dict(execute=command, id=self.commands)
        if arguments is not None:
            payload['arguments'] = arguments
        self.history.append(payload)
        with (self.out / 'qmp.jsonl').open('a') as log:
            log.write(json.dumps(payload) + '\n')
        self.sock.sendall(json.dumps(payload).encode() + b'\n')
        for _ in range(256):
            answer = self.read()
            if 'event' in answer:
                continue
            require(answer.get('id') == self.commands and 'return' in answer, 'QMP acknowledgement')
            return answer['return']
        raise AssertionError('QMP event bound')

    def key(self, *keys):
        require(1 <= len(keys) <= 2, 'key chord bound')
        cursor = len(self.rows())
        pressed = True  # Even an uncertain press acknowledgement needs release.
        try:
            self.request('input-send-event', dict(events=[dict(type='key', data=dict(down=True, key=dict(type='qcode', data=k))) for k in keys]))
        finally:
            if pressed:
                self.request('input-send-event', dict(events=[dict(type='key', data=dict(down=False, key=dict(type='qcode', data=k))) for k in reversed(keys)]))
        return cursor

    def fresh_focus(self, cursor):
        self.wait(lambda: any(p == FOCUS for p, _ in self.rows()[cursor:]))
        return [r for p, r in self.rows()[cursor:] if p == FOCUS][-1]

    def replacement(self, before, cursor, named):
        # Readiness checks only structure; substantive verifier errors propagate.
        self.wait(lambda: all(any(p == kind for p, _ in self.rows()[cursor:]) for kind in (REAL, ACK, FOCUS)))
        return verify_switch(before, self.rows()[cursor:], named)

    def capture(self, name):
        self.captures += 1
        require(self.captures <= CAPTURE_MAX and name.isascii() and name.replace('-', '').isalnum() and len(name) <= 64, 'capture bound/name')
        p = self.out / (name + '.ppm')
        require(not p.exists(), 'capture overwrite')
        self.request('screendump', dict(filename=str(p)))
        require(p.is_file() and p.stat().st_size <= 32 * 1024 * 1024, 'capture size')
        return dict(file=p.name, sha256=hashlib.sha256(p.read_bytes()).hexdigest(), bytes=p.stat().st_size)

    def close(self):
        if self.reader:
            self.reader.close()
        if self.sock:
            self.sock.close()
        if self.proc:
            try:
                os.killpg(self.proc.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(self.proc.pid, signal.SIGKILL)
                self.proc.wait(timeout=10)
            for t in self.threads:
                t.join(timeout=10)
            require(not any(t.is_alive() for t in self.threads), 'drain cleanup')
        # Do not unlink an uncertain/replaced path. Private directory is retained.


def selftest():
    face = dict(identity='face-1', revision=7, actions=[dict(identity='edit-1', availability='Available', arguments=[dict(name='text')])])
    before = dict(face=face, acknowledged_show=dict(show_id='show-1', face_id='face-1', face_revision=7), owner_show_acknowledged=True, configuration=dict(recipe='flat', component=dict(source_document_id='synthetic-source', checked_plot_id='synthetic-checked', fore_id='synthetic-fore')), owner_host='owner', route_plan_id='plan')
    after = copy.deepcopy(before)
    after['acknowledged_show']['show_id'] = 'show-2'
    focus = dict(face_id='face-1', face_revision=7, show_id='show-2', action_id='edit-1', argument_name='text', page=1, pages=1, hit_bounds=dict(x=1,y=1,width=10,height=10), display_bounds=dict(width=640,height=480))
    rows = [(REAL, after), (ACK, dict(status='acknowledged', show_id='show-2')), (FOCUS, focus)]
    verify_switch(before, rows, ('edit-1', 'text'))
    changed_route = copy.deepcopy(rows)
    changed_route[0][1]['route_plan_id'] = 'new-mask-route-plan'
    verify_switch(before, changed_route, ('edit-1', 'text'))
    cases = {}
    for name, mutate in [
        ('wrongFace', lambda r: r[0][1]['face'].update(revision=8)),
        ('reusedShow', lambda r: [r[0][1]['acknowledged_show'].update(show_id='show-1'), r[1][1].update(show_id='show-1'), r[2][1].update(show_id='show-1')]),
        ('missingAck', lambda r: r.pop(1)),
        ('mismatchedFocus', lambda r: r[2][1].update(argument_name='forged')),
        ('changedOwnerHost', lambda r: r[0][1].update(owner_host='foreign-owner')),
        ('emptyRoutePlan', lambda r: r[0][1].update(route_plan_id='')),
        ('preAckFocus', lambda r: r.insert(1, r.pop(2))),
        ('changedComponentSource', lambda r: r[0][1]['configuration']['component'].update(source_document_id='foreign-source')),
        ('changedComponentChecked', lambda r: r[0][1]['configuration']['component'].update(checked_plot_id='foreign-checked')),
        ('changedComponentFore', lambda r: r[0][1]['configuration']['component'].update(fore_id='foreign-fore')),
        ('missingComponent', lambda r: r[0][1]['configuration'].pop('component')),
        ('emptyComponent', lambda r: r[0][1]['configuration']['component'].update(fore_id='')),
        ('wrongRecipe', lambda r: r[0][1]['configuration'].update(recipe='grouped')),
        ('staleFocusShow', lambda r: r[2][1].update(show_id='show-1'))]:
        bad = copy.deepcopy(rows)
        mutate(bad)
        try:
            verify_switch(before, bad, ('edit-1', 'text'))
        except AssertionError:
            cases[name] = 'rejected'
        else:
            raise AssertionError(name + ' incorrectly accepted')
    refusal = [(REFUSE, dict(status='refused', code='owner-look-provisional-input'))]
    verify_refusal(before, refusal)
    try:
        verify_refusal(before, refusal + [(ACK, dict(status='acknowledged', show_id='show-1'))])
    except AssertionError:
        cases['refusalAck'] = 'rejected'
    else:
        raise AssertionError('refusal ack accepted')
    wire = b''.join(p.encode() + json.dumps(v).encode() + b'\n' for p,v in rows)
    require(records(wire + b'CONDUIT_CRECHE_FOCUS {"partial":"\xc3') == rows, 'partial UART record')
    require(records(wire)[1][1]['show_id'] == 'show-2', 'parser')
    entrance = ['cargo', '+stable', 'xtask', 'make', 'conduitos', 'live-owner-boot']
    require(launch_tail(entrance + ['arg']) == ['arg'], 'direct entrance')
    require(launch_tail(['xvfb-run', '-a', '--'] + entrance + ['arg']) == ['arg'], 'owned virtual display')
    # A stale acknowledgement before the per-key cursor cannot satisfy a fresh switch.
    try:
        verify_switch(before, rows[2:], ('edit-1', 'text'))
    except AssertionError:
        cases['preCursorAck'] = 'rejected'
    else:
        raise AssertionError('old acknowledgement reused')
    class FakeKeys:
        def __init__(self): self.sent = []
        def rows(self): return []
        def request(self, command, arguments):
            self.sent.append(arguments)
    fake = FakeKeys()
    require(Session.key(fake, 'shift', 'f9') == 0, 'key cursor')
    require([e['data']['key']['data'] for e in fake.sent[0]['events']] == ['shift', 'f9'], 'shiftedF9 press')
    require([e['data']['key']['data'] for e in fake.sent[1]['events']] == ['f9', 'shift'], 'shiftedF9 release')
    require(all(not e['data']['down'] for e in fake.sent[1]['events']), 'key release')
    class UncertainPress(FakeKeys):
        def request(self, command, arguments):
            super().request(command, arguments)
            if len(self.sent) == 1:
                raise TimeoutError('synthetic uncertain press acknowledgement')
    uncertain = UncertainPress()
    try:
        Session.key(uncertain, 'f7')
    except TimeoutError:
        pass
    else:
        raise AssertionError('uncertain press lost')
    require(len(uncertain.sent) == 2 and not uncertain.sent[-1]['events'][0]['data']['down'], 'uncertain press release')
    for bad in (b'x' * (SERIAL_MAX + 1), (REFUSE.encode() + b'{}\n') * (RECORD_MAX + 1)):
        try:
            records(bad)
        except AssertionError:
            pass
        else:
            raise AssertionError('finite transcript bound accepted')
    print(json.dumps(dict(status='selftest-passed', actual_guest_runs=0, positive_switch=True, changed_route_plan_accepted=True, mutation_checks=cases, complete_record_cursor=True), indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', metavar='PRIVATE_CONFIG', help='EXPLICIT private run only; default is synthetic self-test')
    args = parser.parse_args()
    if not args.run:
        selftest()
        return
    config = json.loads(private(args.run).read_text())
    require(set(config) == {'argv', 'cwd', 'output', 'qmp_socket'}, 'closed private launch config')
    # Launch-only adapter. A caller imports Session for the reviewed owner/browser
    # coordination and exact per-key assertions; this CLI sends no user action.
    s = Session(config['argv'], config['cwd'], config['output'], config['qmp_socket'])
    try:
        # Explicit CLI run is a bounded passive boot observation, not the proof.
        # Import Session for the reviewed per-key sequence. No automatic actions.
        deadline = time.monotonic() + 120
        while time.monotonic() < deadline:
            require(s.proc.poll() is None, 'guest/entrance exited')
            s.rows()
            time.sleep(.05)
    finally:
        s.close()


if __name__ == '__main__':
    main()
