"""Private one-attempt owner/browser/native proof controller. Default synthetic only.
No services are installed/started here; Root supplies a live owner and private ISO.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import queue
import signal
import subprocess
import threading
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('look_qmp', HERE / '5628-owner-look-qmp.py')
qmp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(qmp)
require = qmp.require


def save(path, value):
    with path.open('x') as out:
        json.dump(value, out, indent=2)
        out.write('\n')


class Browser:
    def __init__(self, config, output):
        self.events = queue.Queue(maxsize=64)
        self.error = None
        self.threads = []
        self.used = set()
        argv = ['node', str(HERE / '5628-owner-browser-session.mjs')] + [config[k] for k in
                ('repo', 'owner', 'state', 'handbook', 'playwright')] + [str(output / 'browser')]
        self.proc = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, start_new_session=True)
        for name, stream in [('stdout', self.proc.stdout), ('stderr', self.proc.stderr)]:
            thread = threading.Thread(target=self.drain, args=(stream, output / f'browser-{name}.log', name), daemon=True)
            self.threads.append(thread)
            thread.start()

    def drain(self, stream, path, name):
        total = 0
        try:
            with path.open('xb') as out:
                while True:
                    line = stream.readline(2 * 1024 * 1024 + 1)
                    if not line:
                        return
                    total += len(line)
                    require(total <= 4 * 1024 * 1024 and len(line) <= 2 * 1024 * 1024, 'Node log bound')
                    out.write(line)
                    out.flush()
                    if name == 'stdout':
                        self.events.put_nowait(json.loads(line))
        except BaseException as error:
            self.error = error

    def receive(self, event, identity=None, timeout=90):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.error:
                raise self.error
            try:
                value = self.events.get(timeout=.1)
            except queue.Empty:
                require(self.proc.poll() is None, 'Node helper exited')
                continue
            require(value.get('event') == event, 'unexpected Node event')
            if identity is not None:
                require(value.get('id') == identity, 'Node command identity')
            return value
        raise TimeoutError('Node response deadline')

    def send(self, value):
        require(value['id'] not in self.used and len(self.used) < 32, 'command identity/bound')
        self.used.add(value['id'])
        self.proc.stdin.write((json.dumps(value) + '\n').encode())
        self.proc.stdin.flush()

    def close(self):
        if self.proc.poll() is None:
            os.killpg(self.proc.pid, signal.SIGTERM)
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(self.proc.pid, signal.SIGKILL)
                self.proc.wait(timeout=10)
        for thread in self.threads:
            thread.join(timeout=5)
        require(not any(thread.is_alive() for thread in self.threads), 'Node drains remain')


def raw_records(guest, prefix):
    with guest.lock:
        data = bytes(guest.buffers['serial'])
    require(len(data) <= qmp.SERIAL_MAX, 'serial bound')
    end = data.rfind(b'\n')
    return [json.loads(line[len(prefix):]) for line in data[:end + 1].decode().splitlines()
            if line.startswith(prefix)]


def standby(guest):
    parts = raw_records(guest, 'CONDUIT_NATIVE_GUEST_PART ')
    faces = raw_records(guest, 'CONDUIT_NATIVE_OWNER_FACE ')
    routes = raw_records(guest, 'CONDUIT_NATIVE_OWNER_ROUTE ')
    shown = [face for face in faces if face.get('status') == 'shown'
             and face.get('interactions_admitted') is False and face.get('local_show_available') is True]
    if (parts and shown and parts[-1].get('membership_installed') is True
            and any(route.get('status') == 'standby' and route.get('activation') == 'F5' for route in routes)):
        return dict(stage='standby', guest_part=parts[-1], face=shown[-1])
    return None


def run(config):
    os.umask(0o077)
    output = Path(config['out']).absolute()
    qmp.private(output.parent, directory=True)
    require(not output.exists(), 'fresh output required')
    output.mkdir(mode=0o700)
    browser = guest = None
    receipt = dict(schema='conduit.proof/private-owner-look-journey@1', status='running',
                   proof_class='live owner plus graphical normal-product QEMU and one pinned browser', observations=[])
    try:
        browser = Browser(config, output)
        joined = browser.receive('browser-joined')  # Lulled Body, browser selected and acknowledged.
        browser.send(dict(op='start-todo', id='start-todo'))
        started = browser.receive('todo-started', 'start-todo')
        seed = None
        if config.get('seed_text'):
            browser.send(dict(op='seed', id='seed-unicode', text=config['seed_text']))
            seed = browser.receive('seeded', 'seed-unicode')
        ready = dict(event='seeded-ready', browser_joined=joined, todo_started=started, seeded=seed)
        ready_path = Path(config['ready_file']).absolute()
        qmp.private(ready_path.parent, directory=True)
        save(ready_path, ready)
        print(json.dumps(dict(event='seeded-ready', ready_file=config['ready_file'])), flush=True)
        provision = Path(config['provision_ready'])
        require(not provision.exists(), 'provision marker must be fresh')
        deadline = time.monotonic() + 120
        while not provision.exists():
            require(time.monotonic() < deadline, 'passive provisioning deadline')
            require(browser.proc.poll() is None and browser.error is None, 'browser ended before provisioning')
            time.sleep(.1)
        marker_path = qmp.private(provision)
        require(marker_path.stat().st_size <= 8192, 'provision marker bound')
        marker = json.loads(marker_path.read_text())
        require(marker.get('membership_admitted') is False, 'provisioning grants no membership')
        require(marker['source'] == config['source'], 'provisioned Source')
        iso = qmp.private(config['spore'])
        require(hashlib.file_digest(iso.open('rb'), 'sha256').hexdigest() == marker['iso_sha256'], 'private ISO digest')
        argv = ['cargo', '+stable', 'xtask', 'make', 'conduitos', 'live-owner-boot',
                '--spore', config['spore'], '--candidate-id', marker['candidate_id'],
                '--owner-forward', config['owner_forward'], '--qmp-socket', config['qmp']]
        if config.get('xvfb', False):
            argv = ['xvfb-run', '-a', '--'] + argv
        guest = qmp.Session(argv, config['repo'], output / 'native', Path(config['qmp']))
        guest.wait(lambda: standby(guest) is not None, 90)
        standby_path = output / 'native-standby.json'
        save(standby_path, standby(guest))
        browser.send(dict(op='handoff', id='handoff-native', standby=str(standby_path)))
        preferred = browser.receive('native-preferred', 'handoff-native')
        wardrobe = preferred['wardrobe_revision']
        action_deadline = time.monotonic() + 180

        def key(*keys):
            require(time.monotonic() < action_deadline, '180-second action deadline')
            return guest.key(*keys)

        cursor = key('f5')  # Exactly once after passive native preference.
        guest.wait(lambda: any(p == qmp.ACK and r.get('status') == 'acknowledged'
                              for p, r in guest.rows()[cursor:]), 30)
        guest.capture('f5-acknowledged')
        # The generic F5 route may not emit a configured REAL record. F6 is the
        # first admitted compact realization; do not manufacture a generic REAL.
        cursor = key('f6')
        guest.wait(lambda: all(any(p == kind for p, _ in guest.rows()[cursor:])
                              for kind in (qmp.REAL, qmp.ACK, qmp.FOCUS)), 30)
        baseline, focus = qmp.acknowledged(guest.rows()[cursor:])
        face = baseline['face']
        add = next(a for a in face['actions'] if a['identity'] == 'todo.add' and a['availability'] == 'Available')
        require(len(add['arguments']) == 1, 'actual Add argument contract')
        named = (add['identity'], add['arguments'][0]['name'])
        bound = sum(1 + len(a['arguments']) for a in face['actions'] if a['availability'] == 'Available')
        for _ in range(bound + 1):
            if (focus['action_id'], focus['argument_name']) == named:
                break
            focus = guest.fresh_focus(key('tab'))
        else:
            raise AssertionError('Add argument unavailable in finite control cycle')
        qmp.focus_matches(baseline, focus, named)

        def report(label, real):
            nonlocal wardrobe
            require(time.monotonic() < action_deadline, '180-second report deadline')
            browser.send(dict(op='snapshot', id=label, after_revision=wardrobe,
                              expected_show_id=real['acknowledged_show']['show_id'],
                              expected_route_plan_id=real['route_plan_id'], expected_face_id=real['face']['identity'],
                              expected_face_revision=str(real['face']['revision'])))
            result = browser.receive('snapshot', label, timeout=30)
            wardrobe = result['wardrobe_revision']
            receipt['observations'].append(dict(label=label, browser_artifact=result['artifact'],
                                               show=real['acknowledged_show']['show_id'],
                                               face=real['face']['identity'], revision=str(real['face']['revision']),
                                               capture=guest.capture(label)))

        def switch(label, before, *keys):
            at = key(*keys)
            fresh, _ = guest.replacement(before, at, named)
            report(label, fresh)
            return fresh

        require(baseline['expression']['expression']['density'] == 'compact', 'compact density')
        require(baseline['expression']['expression']['typography'] == 'engineering', 'compact engineering typography')
        report('compact-engineering', baseline)
        draft_at = key('a')
        guest.fresh_focus(draft_at)
        guest.capture('draft-before-refusal')
        at = key('f7')
        guest.wait(lambda: any(p == qmp.REFUSE for p, _ in guest.rows()[at:]), 20)
        qmp.verify_refusal(baseline, guest.rows()[at:])
        report('provisional-refused', baseline)  # Includes independent installed CLI Face read.
        guest.capture('draft-after-refusal')
        guest.fresh_focus(key('esc'))
        spacious = switch('spacious-editorial', baseline, 'f7')
        require(spacious['expression']['expression']['density'] == 'spacious', 'spacious density')
        require(spacious['expression']['expression']['typography'] == 'editorial', 'spacious editorial typography')
        scaled = switch('minimum-scale-200', spacious, 'f9')
        require(scaled['expression']['expression']['text_scale_percent'] == 200, '200 percent admitted scale')
        forced = switch('forced-colors', scaled, 'shift', 'f9')
        require(forced['expression']['platform_forced_colors'] is True, 'forced colors admitted')
        restored_colors = switch('colors-restored', forced, 'shift', 'f9')
        require(restored_colors['expression']['platform_forced_colors'] is False, 'forced colors restored')
        restored_scale = switch('scale-restored', restored_colors, 'f9')
        require(restored_scale['expression']['expression']['text_scale_percent'] == spacious['expression']['expression']['text_scale_percent'], 'source scale restored')
        at = key('f2')
        guest.fresh_focus(at)
        report('details', restored_scale)
        guest.fresh_focus(key('f2'))
        require(time.monotonic() < action_deadline, '180-second action deadline')
        browser.send(dict(op='stop', id='stop'))
        browser.receive('stopped', 'stop', timeout=30)
        receipt['status'] = 'passed-pending-independent-capture-review'
        receipt['draft_boundary'] = 'Full draft preservation requires independent before/after scanout review; serial has no draft bytes.'
        receipt['baseline_boundary'] = 'F5 generic ACK captured; first full configured Face baseline is F6, independently owner-bound by Node snapshot.'
    except BaseException as error:
        receipt.update(status='failed', error=repr(error))
        raise
    finally:
        try:
            if guest is not None:
                guest.close()
        finally:
            if browser is not None:
                browser.close()
            save(output / 'receipt.json', receipt)


def selftest():
    data = b'CONDUIT_NATIVE_GUEST_PART {"membership_installed":true}\npartial'
    require(data.rfind(b'\n') > 0, 'complete-line boundary')
    qmp.selftest()
    print('synthetic adapter invariants only; no service/browser/guest launched')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--run', type=Path, help='private JSON config; default is synthetic self-test')
    args = parser.parse_args()
    if args.run is None:
        selftest()
    else:
        config_path = qmp.private(args.run)
        require(config_path.stat().st_size <= 8192, 'config bound')
        run(json.loads(config_path.read_text()))
