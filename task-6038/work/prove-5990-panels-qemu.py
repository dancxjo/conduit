"""Internal bounded hardware proof; boots an unmodified normal product ISO.

Not a public workflow or replacement for xtask Host boot/journey verification.
Retains failures, exact image identity, serial transcript and actual scanout.
"""
import argparse
import hashlib
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import time


def digest(path):
    return hashlib.file_digest(path.open('rb'), 'sha256').hexdigest()


class Guest:
    def __init__(self, product, source, output, width, height):
        self.out = output
        output.mkdir(exist_ok=False)
        manifest = json.loads((product / 'build-manifest.json').read_text())
        assert manifest['source_identity'] == source
        assert manifest['artifact_role'] == 'product-host'
        image = manifest['image']
        iso = product / image['file']
        assert digest(iso) == image['sha256']
        self.receipt = dict(source=source, proof_runner_sha256=digest(Path(__file__)), image_sha256=image['sha256'],
                            proof_class='freestanding-emulator', viewport=[width, height],
                            status='running', captures=[], edits=[])
        self.serial = output / 'serial.log'
        self.temp = tempfile.TemporaryDirectory(prefix='creche-panels-')
        sock = str(Path(self.temp.name) / 'qmp')
        argv = ['qemu-system-x86_64', '-M', 'q35', '-cpu', 'max', '-m', '512M',
                '-smp', '1', '-display', 'none', '-vga', 'std',
                '-global', f'VGA.xres={width}', '-global', f'VGA.yres={height}',
                '-monitor', 'none', '-qmp', f'unix:{sock},server=on,wait=off',
                '-serial', f'file:{self.serial}', '-no-reboot', '-net', 'none',
                '-device', 'qemu-xhci,id=conduitos-xhci,p2=3,p3=0',
                '-device', 'usb-kbd,id=conduitos-keyboard,bus=conduitos-xhci.0,port=1',
                '-device', 'usb-mouse,id=conduitos-pointer,bus=conduitos-xhci.0,port=2',
                '-cdrom', str(iso), '-boot', 'd']
        self.err = (output / 'qemu-stderr.log').open('wb')
        started = time.monotonic()
        self.proc = subprocess.Popen(argv, stdout=self.err, stderr=self.err)
        self.receipt.update(argv=argv, owned_pid=self.proc.pid)
        self.save()
        try:
            self.wait(lambda: Path(sock).exists(), 10)
            self.sock = socket.socket(socket.AF_UNIX)
            self.sock.settimeout(20)
            self.sock.connect(sock)
            self.reader = self.sock.makefile('rb')
            assert 'QMP' in self.read()
            self.request('qmp_capabilities')
            self.wait(lambda: 'CONDUIT_BOOT_STAGE front-door-ready\n' in self.text(), 60)
            self.receipt['arrival_observed_ms'] = round((time.monotonic() - started) * 1000, 3)
            self.save()
        except BaseException as error:
            self.receipt.update(status='failed', error=repr(error))
            self.proc.terminate()
            self.proc.wait(timeout=10)
            self.receipt['exit_code'] = self.proc.returncode
            self.save()
            self.err.close()
            self.temp.cleanup()
            raise

    def save(self):
        (self.out / 'receipt.json').write_text(json.dumps(self.receipt, indent=2) + '\n')

    def text(self):
        if not self.serial.exists():
            return ''
        assert self.serial.stat().st_size <= 16 * 1024 * 1024
        raw = self.serial.read_bytes()
        # Prefix, UTF-8 JSON and newline are separate serial writes. Only a
        # complete newline-terminated record is an observation.
        end = raw.rfind(b'\n')
        return raw[:end + 1].decode('utf-8') if end >= 0 else ''

    def records(self, prefix):
        result = [json.loads(line[len(prefix):]) for line in self.text().splitlines()
                  if line.startswith(prefix)]
        assert len(result) <= 512
        return result

    def wait(self, predicate, seconds=20):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            assert self.proc.poll() is None, 'guest exited'
            if predicate():
                return
            time.sleep(.05)  # Read-only bounded observation; never performs an action.
        raise TimeoutError('guest observation deadline')

    def read(self):
        line = self.reader.readline(1024 * 1024)
        assert line
        return json.loads(line)

    def request(self, command, arguments=None):
        request = dict(execute=command)
        if arguments is not None:
            request['arguments'] = arguments
        with (self.out / 'qmp.jsonl').open('a') as out:
            out.write(json.dumps(request) + '\n')
        self.sock.sendall(json.dumps(request).encode() + b'\n')
        for _ in range(256):
            reply = self.read()
            if 'event' in reply:
                continue
            assert 'return' in reply, reply
            return reply['return']
        raise AssertionError('QMP event bound')

    def key(self, *keys):
        for down in [True, False]:
            self.request('input-send-event', dict(events=[dict(type='key', data=dict(
                down=down, key=dict(type='qcode', data=key))) for key in keys]))

    def focus(self):
        return self.records('CONDUIT_CRECHE_FOCUS ')[-1]

    def face(self):
        record = self.records('CONDUIT_NATIVE_FACE ')[-1]
        assert record['show']['face_id'] == record['face']['identity']
        assert record['show']['face_revision'] == record['face']['revision']
        return record

    def navigate(self, action, argument='value'):
        current = self.face()
        face = current['face']
        offered = next(a for a in face['actions'] if a['identity'] == action)
        assert offered['availability'] == 'Available'
        bound = sum(1 + len(a['arguments']) for a in face['actions'])
        for step in range(bound + 1):
            focus = self.focus()
            assert focus['face_id'] == face['identity']
            assert focus['show_id'] == current['show']['show_id']
            if (focus['action_id'], focus['argument_name']) == (action, argument):
                return
            assert step < bound, 'control not reachable within admitted action bound'
            before = len(self.records('CONDUIT_CRECHE_FOCUS '))
            self.key('tab')
            self.wait(lambda: len(self.records('CONDUIT_CRECHE_FOCUS ')) > before)

    def local(self, *keys):
        before = len(self.records('CONDUIT_CRECHE_FOCUS '))
        changes = len(self.records('CONDUIT_CRECHE_CHANGE '))
        self.key(*keys)
        self.wait(lambda: len(self.records('CONDUIT_CRECHE_FOCUS ')) > before)
        assert len(self.records('CONDUIT_CRECHE_CHANGE ')) == changes, 'draft mutated Source'

    def commit(self, action, value):
        before = self.face()
        count = len(self.records('CONDUIT_CRECHE_CHANGE '))
        started = time.monotonic()
        self.key('ret')
        self.wait(lambda: len(self.records('CONDUIT_CRECHE_CHANGE ')) > count)
        elapsed = round((time.monotonic() - started) * 1000, 3)
        assert len(self.records('CONDUIT_CRECHE_CHANGE ')) == count + 1
        change = self.records('CONDUIT_CRECHE_CHANGE ')[-1]
        after = self.face()
        assert change['disposition'] == 'source-accepted-and-shown'
        assert change['before_face_id'] == before['face']['identity']
        assert change['before_show_id'] == before['show']['show_id']
        assert change['after_face_id'] == after['face']['identity']
        assert change['after_show_id'] == after['show']['show_id']
        assert after['face']['revision'] == before['face']['revision'] + 1
        assert change['before_revision'] == before['face']['revision']
        assert change['after_revision'] == after['face']['revision']
        assert after['face']['basis'] == before['face']['basis']
        assert after['face']['basis']['body_id'] is None
        assert before['show']['show_id'] != after['show']['show_id']
        for field in ['host_id', 'boot_id', 'offer_generation']:
            assert before['show'][field] == after['show'][field]
        interaction = change['interaction']
        assert interaction['action_id'] == action
        assert interaction['face_id'] == before['face']['identity']
        assert interaction['face_revision'] == before['face']['revision']
        assert interaction['show_id'] == before['show']['show_id']
        prior_action = next(a for a in before['face']['actions'] if a['identity'] == action)
        assert prior_action['availability'] == 'Available'
        assert interaction['target'] == prior_action['target']
        assert len(interaction['arguments']) == 1
        assert interaction['arguments'][0]['name'] == 'value'
        assert interaction['arguments'][0]['value'] == list(value.encode())
        assert interaction['arguments'][0]['value_kind'] == 'value/text'
        action_definition = next(a for a in after['face']['actions'] if a['identity'] == action)
        current = next(p for p in after['face']['properties']
                       if p['subject'] == action_definition['target'] and p['name'] == 'current-value')
        assert current['value']['TypedValue']['bytes'] == list(value.encode())
        self.receipt['edits'].append(dict(change=change, observed_submit_ms=elapsed))
        self.save()

    def capture(self, name):
        assert len(self.receipt['captures']) < 32
        path = self.out / (name + '.ppm')
        self.request('screendump', dict(filename=str(path)))
        # Real PPM header, never resized or synthesized.
        with path.open('rb') as image:
            assert image.readline().strip() == b'P6'
            dimensions = image.readline()
            while dimensions.startswith(b'#'):
                dimensions = image.readline()
            assert list(map(int, dimensions.split())) == self.receipt['viewport']
        self.receipt['captures'].append(dict(file=path.name, sha256=digest(path),
                                            focus=self.focus()))
        self.save()

    def capture_pages(self, label):
        pages = self.focus()['pages']
        assert pages <= 16
        for _ in range(self.focus()['page'] - 1):
            self.local('pgup')
        visited = set()
        for page_index in range(pages):
            focus = self.focus()
            assert focus['page'] not in visited
            visited.add(focus['page'])
            self.capture(f"{label}-page-{focus['page']}")
            if page_index + 1 < pages:
                self.local('pgdn')
        assert len(visited) == pages

    def close(self):
        if self.proc.poll() is None:
            try:
                self.request('quit')
            except (OSError, AssertionError):
                pass
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.proc.terminate()
                self.proc.wait(timeout=10)
        self.receipt['exit_code'] = self.proc.returncode
        self.save()
        self.err.close()
        self.temp.cleanup()


def exercise(guest):
    guest.capture('arrival')
    initial = guest.face()
    choices = [a for a in initial['face']['actions'] if a['identity'].startswith('creche.plot.')]
    assert len(choices) > 4, 'actual Source must offer more than four candidates'
    guest.receipt['source_plot_candidates'] = choices
    bound = sum(1 + len(a['arguments']) for a in initial['face']['actions'])
    focus_start = guest.focus()
    observed = []
    pages = {focus_start['page']}
    for step in range(bound):
        focus = guest.focus()
        observed.append(focus)
        before = len(guest.records('CONDUIT_CRECHE_FOCUS '))
        guest.key('tab')
        guest.wait(lambda: len(guest.records('CONDUIT_CRECHE_FOCUS ')) > before)
        focus = guest.focus()
        if focus['page'] not in pages:
            guest.capture(f"navigation-page-{focus['page']}")
            pages.add(focus['page'])
        if (focus['action_id'], focus['argument_name']) == (
                focus_start['action_id'], focus_start['argument_name']):
            break
    else:
        raise AssertionError('focus cycle exceeded offered action bound')
    assert all(f['show_id'] == initial['show']['show_id'] for f in observed)
    second_column = any(f['hit_bounds'] and f['hit_bounds']['x'] > guest.receipt['viewport'][0] / 2
                        for f in observed)
    assert second_column == (guest.receipt['viewport'][0] == 1280)
    guest.receipt['initial_focus_cycle'] = observed
    guest.receipt['observed_second_column_control'] = second_column
    guest.save()
    guest.navigate('creche.naming')
    guest.local('left')  # Actual Source catalogue: surprise -> roman, wrapping.
    guest.capture('tradition-draft')
    guest.commit('creche.naming', 'roman')
    guest.capture('tradition-committed')
    guest.navigate('creche.name')
    name = 'café' + 'a' * 59
    for keys in [('c',), ('a',), ('f',), ('alt_r', 'e')] + [('a',)] * 59:
        guest.local(*keys)
    guest.capture('long-unicode-draft')
    guest.commit('creche.name', name)
    guest.capture_pages('long-unicode-committed')
    guest.navigate('creche.name')
    guest.capture('unicode-committed')
    guest.local('backspace')  # Fresh replacement buffer starts empty.
    guest.commit('creche.name', '')
    birth = next(a for a in guest.face()['face']['actions'] if a['identity'] == 'creche.birth')
    assert birth['availability'] == {'Unavailable': {
        'reason_code': 'birth/unavailable', 'explanation': 'InvalidName'}}
    guest.receipt['empty_name_birth_availability'] = birth['availability']
    guest.capture_pages('empty-name-unavailable')
    guest.navigate('creche.name')
    for key in ['c', 'a', 'f']:
        guest.local(key)
    guest.local('alt_r', 'e')
    guest.commit('creche.name', 'café')
    guest.navigate('creche.search')
    for key in ['z', 'z', 'z', 'z']:
        guest.local(key)
    guest.commit('creche.search', 'zzzz')
    guest.capture('filtered-review')
    face = guest.face()['face']
    selected = [s for s in face['subjects'] if '/birth-review-plot-' in s['identity']]
    assert selected, 'filtered selection remains reviewable'
    ids = {s['identity'] for s in selected}
    original_ids = {s['identity'] for s in initial['face']['subjects']
                    if '/birth-review-plot-' in s['identity']}
    assert ids == original_ids
    original_names = [t for t in initial['face']['text'] if t['subject'] in original_ids]
    assert [t for t in face['text'] if t['subject'] in ids] == original_names
    assert not any(d['subject'] in ids and d['level'] != 'Primary' for d in face['disclosures'])
    guest.receipt['filtered_selected_subjects'] = selected
    guest.receipt['filtered_selected_names'] = original_names
    # Read every filtered primary page through ordinary PageDown. Screenshot
    # evidence remains separate from the Source-only visibility assertions.
    guest.capture_pages('filtered-review')
    guest.receipt['status'] = 'passed'
    guest.save()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('product', type=Path)
    parser.add_argument('source')
    parser.add_argument('output', type=Path)
    parser.add_argument('--width', type=int, choices=[640, 1280], required=True)
    args = parser.parse_args()
    guest = Guest(args.product.resolve(), args.source, args.output.resolve(), args.width,
                  480 if args.width == 640 else 800)
    try:
        exercise(guest)
    except BaseException as error:
        guest.receipt.update(status='failed', error=repr(error))
        try:
            guest.capture('failure')
        except BaseException as capture_error:
            guest.receipt['failure_capture_error'] = repr(capture_error)
        raise
    finally:
        guest.close()
