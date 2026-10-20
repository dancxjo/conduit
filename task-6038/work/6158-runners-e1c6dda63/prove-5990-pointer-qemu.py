"""Internal supplemental real USB pointer proof; no product instrumentation.

Tab exposes each Source-derived current Show hit, then actual relative USB
reports reach it. Hover is physical reachability, never claimed as dispatch.
Field clicks transfer focus from a distinct same-page control; one Boolean toggles/restores; Birth is deliberately last.
"""
import argparse
import importlib.util
import re
from pathlib import Path

BASE = Path(__file__).with_name('prove-5990-panels-qemu.py')
spec = importlib.util.spec_from_file_location('panels_proof', BASE)
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
POINTER = 'CONDUIT_NATIVE_POINTER '
HID_QMP = Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/work/native-field-pagination/targets/conduitos/make/xtask/hid_qmp.rs')
# Reuse the supported journey's cold preparation budget, not the ordinary
# twenty-second input acknowledgement budget. Missing/changed source refuses.
COLD_PREPARATION_WAIT_TIMEOUT = int(re.search(
    r'const COLD_PREPARATION_WAIT_TIMEOUT: Duration = Duration::from_secs\((\d+)\);',
    HID_QMP.read_text()).group(1))


class PointerGuest(base.Guest):
    def sample(self):
        r = self.records(POINTER)[-1]
        assert r['schema'] == 'conduit.native-pointer/v1'
        assert r['coalesced'] == r['dropped'] == 0
        for axis in ('x', 'y'):
            assert 0 <= r['position_' + axis] <= 1_000_000
        return r

    def await_sample(self, sequence, predicate):
        def observed():
            return any(r['sequence'] > sequence and predicate(r)
                       for r in self.records(POINTER))
        self.wait(observed)
        r = next(r for r in self.records(POINTER)
                 if r['sequence'] > sequence and predicate(r))
        assert r['coalesced'] == r['dropped'] == 0
        return r

    def motion(self, dx, dy):
        assert -127 <= dx <= 127 and -127 <= dy <= 127
        old = self.records(POINTER)
        seq = old[-1]['sequence'] if old else 0
        self.request('input-send-event', dict(events=[dict(type='rel', data=dict(axis=a, value=v))
                                                     for a, v in [('x', dx), ('y', dy)]]))
        if hasattr(self, 'scale'):
            expected = [max(0, min(1_000_000, old[-1]['position_' + a] + d * self.scale[i]))
                        for i, (a, d) in enumerate([('x', dx), ('y', dy)])]
            return self.await_sample(seq, lambda r: [r['position_x'], r['position_y']] == expected
                                     and not r['primary_pressed'])
        return self.await_sample(seq, lambda r: r['delta_x'] > 0 if dx else r['delta_y'] > 0)

    def bind(self, sample, focus):
        for field in ('face_id', 'face_revision', 'show_id'):
            assert sample[field] == focus[field], (field, sample, focus)

    def calibrate(self):
        scales = []
        for axis, delta in [('x', (1, 0)), ('y', (0, 1))]:
            sample = self.motion(*delta)
            self.bind(sample, self.focus())
            assert not sample['primary_pressed'] and not sample['press_edge']
            scale = sample['delta_' + axis]
            assert 1 <= scale <= 1_000_000
            scales.append(scale)
        self.scale = scales
        self.receipt['pointer_scale'] = scales
        return scales

    def reach(self, scale, target_focus=None):
        focus = target_focus or self.focus()
        current = self.focus()
        acknowledged = self.face()
        assert focus['page'] == current['page'], 'retained target must remain on displayed page'
        for field in ('face_id', 'face_revision', 'show_id'):
            assert focus[field] == current[field]
        assert focus['show_id'] == acknowledged['show']['show_id']
        assert focus['face_id'] == acknowledged['face']['identity']
        assert focus['face_revision'] == acknowledged['face']['revision']
        box, display = focus['hit_bounds'], focus['display_bounds']
        assert box and display['x'] == display['y'] == 0
        for axis, extent in [('x', 'width'), ('y', 'height')]:
            assert 0 <= box[axis] < box[axis] + box[extent] <= display[extent] <= 16384
        moves = []
        for step in range(5):
            sample = self.sample()
            if step:
                self.bind(sample, focus)
            assert not sample['primary_pressed'] and not sample['press_edge']
            inside = all(box[a] <= sample['display_' + a] < box[a] + box[e]
                         for a, e in [('x', 'width'), ('y', 'height')])
            # Refresh an old-Show sample with a real bounded motion, never fake binding.
            if inside and all(sample[f] == focus[f] for f in ('face_id', 'face_revision', 'show_id')):
                return dict(focus=focus, sample=sample, moves=moves)
            assert step < 4, 'pointer target not reached in four physical reports'
            delta = []
            for i, (a, e) in enumerate([('x', 'width'), ('y', 'height')]):
                target = (box[a] + box[e] // 2) * 1_000_000 // display[e]
                difference = target - sample['position_' + a]
                value = ((abs(difference) + scale[i] // 2) // scale[i]) * (1 if difference >= 0 else -1)
                delta.append(max(-127, min(127, value)))
            if delta == [0, 0]:
                delta = [1, 0]  # Actual harmless refresh; subsequently rechecked inside hit.
            moves.append(self.motion(*delta))
        raise AssertionError('unreachable')

    def click(self, target):
        before = self.sample()['sequence']
        self.request('input-send-event', dict(events=[dict(type='btn', data=dict(button='left', down=True))]))
        press = self.await_sample(before, lambda r: r['primary_pressed'] and r['press_edge'])
        self.bind(press, target['focus'])
        box = target['focus']['hit_bounds']
        assert box['x'] <= press['display_x'] < box['x'] + box['width']
        assert box['y'] <= press['display_y'] < box['y'] + box['height']
        self.request('input-send-event', dict(events=[dict(type='btn', data=dict(button='left', down=False))]))
        release = self.await_sample(press['sequence'], lambda r: not r['primary_pressed'])
        assert not release['press_edge']
        return dict(press=press, release=release)


def expected_controls(face):
    """Independently derive renderer traversal from checked Face order/arguments."""
    assert len(face['subjects']) <= 32768 and len(face['actions']) <= 32768
    edges = set()
    for relation in face['relationships']:
        kind = relation['kind']
        ordered = kind in ('Contains', 'Connects', 'Describes', 'Realizes', 'Observes') if isinstance(kind, str) else isinstance(kind, dict) and str(kind.get('Semantic', '')).endswith('/precedes')
        if ordered:
            edges.add((relation['source'], relation['target']))
    for relation in face['composition']:
        a, b = relation['source'], relation['target']
        edges.add((b, a) if relation['kind'] in ('Group', 'RevealAfter') else (a, b))
    remaining = sorted(s['identity'] for s in face['subjects'])
    order = []
    while remaining:
        candidate = next((c for c in remaining if not any(p != c and (p, c) in edges for p in remaining)), remaining[0])
        order.append(candidate)
        remaining.remove(candidate)
    expected = []
    for subject in order:
        definition = next(s for s in face['subjects'] if s['identity'] == subject)
        excluded = any(d['subject'] == subject and (d['level'] in ('CurrentAction', 'SelectedDetail', 'ExactProvenance') or d['level'] == 'Context' and definition['role'] not in ('Collection', 'Document')) for d in face['disclosures'])
        if excluded:
            continue
        for action in sorted((a for a in face['actions'] if a['target'] == subject), key=lambda a: a['identity']):
            if action['availability'] != 'Available':
                continue
            expected.append((action['identity'], None))
            arguments = action['arguments']
            if len(arguments) == 1 and arguments[0]['contract']['value_kind'] == 'value/bool':
                assert arguments[0]['contract']['maximum_bytes'] == 1
                assert arguments[0]['contract']['constraints'] == [], 'unsupported Boolean constraint proof'
                selected = [p for p in face['properties'] if p['subject'] == subject and p['name'] == 'selected']
                bindings = [p for p in face['properties'] if p['subject'] == subject and p['name'] == 'boolean-change-action']
                assert len(selected) == len(bindings) == 1
                assert isinstance(selected[0]['value']['Flag'], bool)
                assert bindings[0]['value']['Identity'] == action['identity']
            else:
                expected.extend((action['identity'], argument['name']) for argument in arguments)
    assert expected and len(expected) == len(set(expected))
    return expected


def controls(guest):
    initial = guest.face()
    start = guest.focus()
    expected = expected_controls(initial['face'])
    found = []
    bound = sum(1 + len(a['arguments']) for a in initial['face']['actions'])
    for _ in range(bound):
        f = guest.focus()
        key = (f['action_id'], f['argument_name'])
        assert key not in found
        action = next(a for a in initial['face']['actions'] if a['identity'] == key[0])
        assert action['availability'] == 'Available'
        assert f['show_id'] == initial['show']['show_id']
        found.append(key)
        guest.local('tab')
        if (guest.focus()['action_id'], guest.focus()['argument_name']) == (start['action_id'], start['argument_name']):
            index = expected.index(found[0])
            assert found == expected[index:] + expected[:index], (found, expected)
            guest.receipt['expected_source_control_order'] = expected
            guest.save()
            return found
    raise AssertionError('control cycle did not close within Source bound')


def exercise(guest):
    guest.receipt.update(pointer_runner_sha256=base.digest(Path(__file__)),
                         imported_runner_sha256=base.digest(BASE), pointer_hits=[], dispatches=[],
                         boundary='All eligible hits physically reached; argument fields pointer-focused from distinct same-page controls without Source mutation; one Boolean toggled/restored; Birth enacted last. Main action rows with arguments and Suggest are reached without dispatch.')
    guest.capture('arrival')
    expected = controls(guest)
    scale = guest.calibrate()
    for action, argument in expected:
        guest.navigate(action, argument)
        before = guest.face()
        target = guest.reach(scale)
        entry = dict(action=action, argument=argument, target=target)
        if argument is not None:
            # Keyboard locates a distinct available control on the SAME page;
            # its type does not matter because we never activate this origin.
            retained = target['focus']
            origin = None
            for other_action, other_argument in expected:
                if (other_action, other_argument) == (action, argument):
                    continue
                guest.navigate(other_action, other_argument)
                candidate = guest.focus()
                if candidate['page'] == retained['page']:
                    origin = candidate
                    break
            assert origin is not None, 'no distinct same-page focus origin; refuse transfer claim'
            assert (origin['action_id'], origin['argument_name']) != (action, argument)
            target = guest.reach(scale, retained)
            count = len(guest.records('CONDUIT_CRECHE_CHANGE '))
            focus_count = len(guest.records('CONDUIT_CRECHE_FOCUS '))
            entry['transfer_origin'] = origin
            entry['target'] = target
            entry['click'] = guest.click(target)
            guest.wait(lambda: len(guest.records('CONDUIT_CRECHE_FOCUS ')) > focus_count)
            f = guest.focus()
            assert (f['action_id'], f['argument_name']) == (action, argument)
            assert f['show_id'] == retained['show_id']
            entry['transferred_focus'] = f
            assert len(guest.records('CONDUIT_CRECHE_CHANGE ')) == count
            assert guest.face() == before
        guest.receipt['pointer_hits'].append(entry)
        guest.save()
    assert len(guest.receipt['pointer_hits']) == len(expected)
    # Reversible selected Boolean dispatch, typed current Source and Show checked.
    boolean = next(a for a in guest.face()['face']['actions'] if a['availability'] == 'Available'
                   and len(a['arguments']) == 1 and a['arguments'][0]['contract']['value_kind'] == 'value/bool')
    initial_property = next(p for p in guest.face()['face']['properties']
                            if p['subject'] == boolean['target'] and p['name'] == 'selected')
    initial_value = [int(initial_property['value']['Flag'])]
    assert initial_value in ([0], [1])
    for iteration in range(2):
        guest.navigate(boolean['identity'], None)
        before = guest.face()
        target = guest.reach(scale)
        count = len(guest.records('CONDUIT_CRECHE_CHANGE '))
        click = guest.click(target)
        guest.wait(lambda: len(guest.records('CONDUIT_CRECHE_CHANGE ')) > count)
        assert len(guest.records('CONDUIT_CRECHE_CHANGE ')) == count + 1
        change = guest.records('CONDUIT_CRECHE_CHANGE ')[-1]
        assert change['disposition'] == 'source-accepted-and-shown'
        interaction = change['interaction']
        for field in ('face_id', 'face_revision', 'show_id'):
            assert interaction[field] == target['focus'][field]
        assert interaction['action_id'] == boolean['identity'] and interaction['target'] == boolean['target']
        arg = interaction['arguments'][0]
        assert arg['name'] == 'value' and arg['value_kind'] == 'value/bool'
        assert arg['value'] in ([0], [1])
        assert arg['value'] == ([1 - initial_value[0]] if iteration == 0 else initial_value)
        current_property = next(p for p in guest.face()['face']['properties']
                                if p['subject'] == boolean['target'] and p['name'] == 'selected')
        assert [int(current_property['value']['Flag'])] == arg['value']
        assert guest.face()['face']['revision'] == before['face']['revision'] + 1
        guest.receipt['dispatches'].append(dict(click=click, change=change))
        guest.save()
    guest.navigate('creche.birth', None)
    before = guest.face()
    target = guest.reach(scale)
    count = len(guest.records('CONDUIT_NATIVE_BIRTH '))
    # Exactly one physical press begins Birth. Crèche pointer observations cease
    # when its owner entrance closes, so release is QMP-acknowledged only.
    sequence = guest.sample()['sequence']
    guest.receipt['birth_attempt'] = dict(target=target,
                                         cold_preparation_timeout_seconds=COLD_PREPARATION_WAIT_TIMEOUT,
                                         timeout_source_sha256=base.digest(HID_QMP))
    guest.save()
    click = {}
    try:
        guest.request('input-send-event', dict(events=[dict(type='btn', data=dict(button='left', down=True))]))
        press = guest.await_sample(sequence, lambda r: r['primary_pressed'] and r['press_edge'])
        guest.bind(press, target['focus'])
        box = target['focus']['hit_bounds']
        assert box['x'] <= press['display_x'] < box['x'] + box['width']
        assert box['y'] <= press['display_y'] < box['y'] + box['height']
        click['press'] = press
        guest.receipt['birth_attempt']['physical_press'] = press
        guest.save()
        guest.wait(lambda: len(guest.records('CONDUIT_NATIVE_BIRTH ')) > count,
                   COLD_PREPARATION_WAIT_TIMEOUT)
    finally:
        guest.request('input-send-event', dict(events=[dict(type='btn', data=dict(button='left', down=False))]))
        click['release'] = dict(qmp_acknowledged=True, physical_guest_observation=False,
                                boundary='Crèche pointer observation ends when Birth closes its entrance')
        guest.receipt['birth_attempt']['release'] = click['release']
        guest.save()
    assert len(guest.records('CONDUIT_NATIVE_BIRTH ')) == count + 1
    birth = guest.records('CONDUIT_NATIVE_BIRTH ')[-1]
    assert birth['requested_intent'] == 'conduit.intent/creche.birth@1'
    assert 'Accepted' in birth['disposition'] and birth['accepted_effects'] == 1
    assert birth['before']['body_id'] is None and birth['before']['born_sign_id'] is None
    assert birth['after']['body_id'] and birth['after']['born_sign_id']
    for field in ('face_id', 'face_revision', 'show_id'):
        assert birth[field] == target['focus'][field]
    for field in ('host_id', 'boot_id', 'offer_generation'):
        assert birth['before'][field] == birth['after'][field]
    guest.receipt.update(birth=dict(click=click, receipt=birth), status='passed')
    guest.save()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('product', type=Path)
    parser.add_argument('source')
    parser.add_argument('output', type=Path)
    parser.add_argument('--width', type=int, choices=[640, 1280], required=True)
    args = parser.parse_args()
    guest = PointerGuest(args.product.resolve(), args.source, args.output.resolve(), args.width,
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
