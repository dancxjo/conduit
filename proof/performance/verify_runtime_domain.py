"""Independent semantic and identity validation for control/projection/Value trials."""
import hashlib
import json
import math
import statistics
import struct
from fractions import Fraction
from verify_runtime_stream import distribution

UNIT_HASHES = {
    'centimetre_unit': 'e5721e2abe7e54a58dad7d0c199c519c685e0f20f2583954e1787b0a4efc4b6b',
    'millimetre_unit': '65fddd2f008ac5db091e25db52ecac1abfdee85d29968acd2ffb6cfb668c65ca',
    'hertz_unit': '7ad7f2e9d9f03bea0a7631405a1ca8f52f09e54e2f9849b9faf9c473d527a971',
}
NUMERICAL_SOURCE = 'plot numerics (\n exact: NumericalResult <= 8192B >>\n loose: NumericalResult <= 8192B >>\n) {\n precise: math/realize(request={value:0.3,maximum-error:0})\n approximate: math/realize(request={value:0.3,maximum-error:1})\n precise.result >> exact\n approximate.result >> loose\n}\n'


class Cursor:
    def __init__(self, data):
        self.data, self.pos = bytes(data), 0

    def take(self, n):
        assert n >= 0 and self.pos + n <= len(self.data), 'truncated structured receipt'
        data = self.data[self.pos:self.pos + n]
        self.pos += n
        return data

    def integer(self, n):
        return int.from_bytes(self.take(n), 'little')

    def blob(self):
        return self.take(self.integer(4))

    def text(self):
        return self.blob().decode('utf-8')


def typed(c, depth=0):
    assert depth < 16
    tag = c.integer(1)
    if tag == 0:
        return ('leaf', c.text())
    if tag == 5:
        return ('nominal', c.text(), typed(c, depth + 1))
    if tag in [2, 3]:
        schema = c.text()
        count = c.integer(4)
        assert count <= 64
        fields = [(c.text(), typed(c, depth + 1)) for _ in range(count)]
        assert len({f[0] for f in fields}) == count
        return ('record' if tag == 2 else 'variant', schema, fields)
    raise AssertionError('unexpected numerical receipt type')


def value(c, ty, depth=0):
    assert depth < 16
    if ty[0] == 'nominal':
        return value(c, ty[2], depth + 1)
    tag = c.integer(1)
    if ty[0] == 'leaf':
        assert tag == 0
        return c.blob()
    if ty[0] == 'record':
        assert tag == 2 and c.integer(4) == len(ty[2])
        result = {}
        for name, child in ty[2]:
            assert c.text() == name, 'numerical record field order differs'
            result[name] = value(c, child, depth + 1)
        return result
    assert ty[0] == 'variant' and tag == 3
    name = c.text()
    fields = dict(ty[2])
    assert name in fields
    return (name, value(c, fields[name], depth + 1))


def numerical_receipt(encoded, kind):
    c = Cursor(encoded)
    ty = typed(c)
    prefix = c.data[:c.pos]
    digest = hashlib.sha256(b'conduit.structured-info.type.v1' + len(prefix).to_bytes(4, 'little') + prefix).hexdigest()
    assert kind == f'structured-info/profile-{digest}@1', 'numerical output Type differs'
    decoded = value(c, ty)
    assert c.pos == len(c.data)
    return decoded


def mathematical(data):
    c = Cursor(data)
    assert c.take(3) == bytes([1, 0, 1]), 'numerical source is not canonical rational'
    sign = c.integer(1)
    assert sign in [0, 1]
    n, d = c.integer(2), c.integer(2)
    numerator, denominator = c.integer(n), c.integer(d)
    assert c.pos == len(c.data) and denominator > 0
    assert n == (numerator.bit_length() + 7) // 8 and d == (denominator.bit_length() + 7) // 8
    result = Fraction(-numerator if sign else numerator, denominator)
    assert abs(result.numerator) == numerator and result.denominator == denominator
    assert numerator != 0 or sign == 0
    return result


def quantity(coordinate, unit):
    exponent = 0
    if coordinate:
        while coordinate % 10 == 0:
            coordinate //= 10
            exponent += 1
    return bytes([1, 0]) + unit + exponent.to_bytes(2, 'little', signed=True) + coordinate.to_bytes(16, 'little', signed=True)


def custody(root, status, row, play=None):
    prefix = f"sources/{row['case']}-{row['repetition']}"
    assert all(prefix + s in status['artifact_sha256'] for s in ['-plan.json', '-host.json'])
    plan = json.loads((root / (prefix + '-plan.json')).read_text())
    host = json.loads((root / (prefix + '-host.json')).read_text())
    assert len(plan['fragments']) == 1
    f = plan['fragments'][0]
    play = play or row['active_play']
    assert plan['plan_id'] == f['plan_id'] == play['plan_id'] and row['checked_plot_id'] == plan['checked_plot_id'], 'domain Plan custody'
    assert (host['host_id'], host['boot_id']) == (f['host_id'], f['boot_id']) == (play['host_id'], play['boot_id']), 'domain Host/Boot custody'
    assert f['offer_generation'] == host['offer_generation']
    return plan, f


def terminal(observations, play):
    ends = [o for o in observations if isinstance(o['kind'], dict) and 'PlanTerminal' in o['kind']]
    assert len(ends) == 1 and ends[0]['kind']['PlanTerminal']['disposition'] == 'Completed', 'domain terminal differs'
    for o in observations:
        assert (o['host_id'], o['boot_id']) == (play['host_id'], play['boot_id'])
        for field in ['plan_id', 'active_play_id']:
            assert o.get(field) in [None, play[field]], 'domain observation custody'


def output_times(o, count):
    offsets = o['acknowledgement_offsets_ns']
    assert len(offsets) == count and offsets == sorted(offsets) and all(type(n) is int and n >= 0 for n in offsets)
    assert len(o['outputs']) == count and o['completion_window_outputs'] == count - 1
    window = offsets[-1] - offsets[0]
    assert o['first_to_last_acknowledgement_ns'] == window
    if window:
        assert math.isclose(o['completion_window_useful_outputs_per_second'], (count - 1) * 1e9 / window, rel_tol=1e-12)
    else:
        assert o['completion_window_useful_outputs_per_second'] is None
    deltas = [b - a for a, b in zip(offsets, offsets[1:])]
    if len(deltas) > 1:
        distribution(deltas, o['inter_acknowledgement_distribution'])
    elif deltas:
        d = o['inter_acknowledgement_distribution']
        assert d['count'] == 1 and d['sample_standard_deviation_ns'] is None
        assert all(d[k] == deltas[0] for k in ['min_ns', 'max_ns', 'p50_ns', 'p95_ns', 'p99_ns', 'mean_ns'])
    else:
        assert o['inter_acknowledgement_distribution'] is None
    for v in o['outputs']:
        assert v['track'] == 'payload'
        assert hashlib.sha256(bytes(v['canonical'])).hexdigest() == v['sha256'], 'domain output digest differs'


def signs(events, count):
    assert [e['sequence'] for e in events] == sorted({e['sequence'] for e in events})
    assert sum(e['kind'] == 'RemoteValueDelivered' for e in events) == count, 'domain mandatory Signs differ'


def verify_extended(root, status, raw, metadata):
    domain = [r for r in raw if r.get('phase') in ['ordinary-domain-control', 'ordinary-numerical-control']]
    matrix = [r for r in raw if r.get('phase') == 'ordinary-value-matrix']
    if not domain and not matrix and metadata.get('hosted_extended_matrix_schema') is None:
        return {'domain': [], 'value_matrix': []}
    assert metadata['hosted_extended_matrix_schema'] == 'control-projection-value@1'
    reps = status['config']['repetitions']
    assert len(domain) == 2 * reps and {(r['case'], r['repetition']) for r in domain} == {(c, i) for c in ['theremin-pitch-control', 'numerical-projection-control'] for i in range(reps)}, 'domain matrix incomplete'
    assert len(matrix) == 6 * reps and {(r['gears'], r['mode'], r['repetition']) for r in matrix} == {(g, m, i) for g in [1, 8, 32] for m in ['value-reference', 'value-scalar-specialized'] for i in range(reps)}, 'Value matrix incomplete'
    identities = set()
    receipt = {'domain': [], 'value_matrix': []}
    for row in domain:
        _, f = custody(root, status, row)
        play = row['active_play']
        assert play['active_play_id'] not in identities
        identities.add(play['active_play_id'])
        terminal(row['observations'], play)
        assert row['value_capacity_before'] == row['value_capacity_after']
        outputs = row['output_observations']
        count = 1024 if row['phase'] == 'ordinary-domain-control' else 2
        output_times(outputs, count)
        signs(row['signs'], count)
        if count == 1024:
            assert row['mode'] == 'canonical-theremin-control' and row['control_updates'] == count
            instrument = (root / 'sources/theremin-pitch-control-instrument.plot').read_text()
            assert hashlib.sha256(instrument.encode()).hexdigest() == row['instrument_sha256']
            pitch = instrument.split('\n# The one canonical Theremin.')[0]
            expression = pitch.rsplit(') = ', 1)[1].strip()
            assert expression == '(distance <= 0cm ? 1760Hz : distance >= 30cm ? 110Hz : 1760Hz - (distance / 1cm) * 55Hz)', 'canonical control law changed'
            authored = pitch + '\nplot baseline (\n >> input: Distance...|\n output: Frequency...| >>\n) {\n input >> (' + expression.replace('distance', '.') + ') >> output\n}\n'
            assert (root / 'sources/theremin-pitch-control.plot').read_text() == authored
            assert hashlib.sha256(authored.encode()).hexdigest() == row['source_sha256']
            wire = json.loads((root / 'sources/theremin-pitch-control-inputs.json').read_text())
            units = {name: bytes(wire[name]) for name in UNIT_HASHES}
            assert all(hashlib.sha256(units[k]).hexdigest() == h for k, h in UNIT_HASHES.items()), 'control Unit fixture changed'
            assert wire['quantity_bytes'] == 788 and len(wire['inputs']) == count
            canonical = []
            for i, out in enumerate(outputs['outputs']):
                cm = i % 41 - 5
                encoded = quantity(cm, units['centimetre_unit']) if i % 2 == 0 else quantity(cm * 10, units['millimetre_unit'])
                assert bytes(wire['inputs'][i]) == encoded, 'control input differs'
                expected = quantity(1760 if cm <= 0 else 110 if cm >= 30 else 1760 - cm * 55, units['hertz_unit'])
                assert out['port'] == 'output' and out['sequence'] == i and bytes(out['canonical']) == expected, 'control Frequency differs'
                assert out['value_kind'] == next(p['value_kind'] for p in f['fore_ports'] if p['direction'] == 'Output')
                canonical.append(expected)
            assert row['ordered_output_sha256'] == hashlib.sha256(b''.join(canonical)).hexdigest()
            assert row['ingress']['accepted'] == count and row['ingress']['attempts'] == count + row['ingress']['full']
            assert row['canonical_input_bytes'] == row['canonical_output_bytes'] == 788 * count
            assert len(f['placements']) == 1
        else:
            assert row['mode'] == 'certified-numerical-projections' and row['zero_error_float_refused'] is True
            assert (root / 'sources/numerical-projection-control.plot').read_text() == NUMERICAL_SOURCE
            assert row['source_sha256'] == hashlib.sha256(NUMERICAL_SOURCE.encode()).hexdigest()
            assert len(f['placements']) == 2
            assert {p['implementation_id'] for p in f['placements']} == {'conduit.std/numeric-rational@1', 'conduit.std/numeric-binary64-nearest@1'}
            assert {v['port'] for v in outputs['outputs']} == {'exact', 'loose'}
            for v in outputs['outputs']:
                assert v['sequence'] == 0
                decoded = numerical_receipt(v['canonical'], v['value_kind'])
                assert mathematical(decoded['source']) == Fraction(3, 10), 'numerical source differs'
                target = Fraction(3, 10) if v['port'] == 'exact' else Fraction.from_float(.3)
                assert mathematical(decoded['lower']) == mathematical(decoded['upper']) == target, 'numerical target differs'
                assert mathematical(decoded['maximum-error']) == abs(Fraction(3, 10) - target), 'numerical error differs'
                rep, payload = decoded['representation']
                if v['port'] == 'exact':
                    assert rep == 'exact_rational' and payload == b''
                else:
                    assert rep == 'binary64' and payload['target'] == struct.pack('<d', .3)
                    assert payload['rounding'] == ('nearest_ties_even', b'')
                    assert payload['any-inexact'] == b'\x01' and payload['any-tiny-inexact'] == b'\x00'
                    assert payload['step-count'] == b'\x01' and payload['error-claim'] == ('exact_distance', b'')
            assert {p['port'] for p in row['isolated_back_preparation']} == {'exact', 'loose'}
            for p in row['isolated_back_preparation']:
                cert = p['certificate']
                assert 0 <= cert['prepared_limb_work'] <= cert['admitted_limb_work'] and cert['independent_validation_limb_work'] > 0
                assert p['receipt_sha256'] == next(v['sha256'] for v in outputs['outputs'] if v['port'] == p['port'])
        receipt['domain'].append({'case': row['case'], 'repetition': row['repetition'], 'active_play': play, 'full_host_call': row['full_host_call'], 'kernel_phase_wall': row['kernel_phase_wall']})
    by_workload = {}
    for row in matrix:
        g, mode = row['gears'], row['mode']
        assert row['case'] == f'value-chain-{g}-{mode}' and row['value_plays'] == 64
        authored = (root / f'sources/value-chain-{g}.plot').read_bytes()
        reference = (root / f'sources/hosted-pipeline-{g}-gears-32-adds.plot').read_bytes().replace(b'U64...|', b'U64')
        assert authored == reference and row['source_sha256'] == hashlib.sha256(authored).hexdigest()
        assert len(row['plays']) == len(row['per_play_phase_wall']) == 64
        _, f = custody(root, status, row, row['plays'][0]['active_play'])
        assert len(f['placements']) == g and not f['execution_fusions']
        if mode == 'value-scalar-specialized':
            assert all(p['implementation_id'] == 'std/kernel-pure-scalar-tape@1' for p in f['placements'])
        else:
            assert all(p['implementation_id'] != 'std/kernel-pure-scalar-tape@1' for p in f['placements'])
        e = row['eligibility']
        assert e['flow_specialization']['eligible'] is False and 'Value temporal contract' in e['flow_specialization']['refusal']
        assert e['value_combined_fusion']['eligible'] is False
        assert bool(e['value_combined_fusion']['refusal']) == (g > 1)
        canonical = []
        for i, p in enumerate(row['plays']):
            assert p['input_index'] == i and p['active_play']['plan_id'] == f['plan_id']
            play = p['active_play']
            assert play['active_play_id'] not in identities
            identities.add(play['active_play_id'])
            assert (play['host_id'], play['boot_id']) == (f['host_id'], f['boot_id'])
            assert p['value_capacity_before'] == p['value_capacity_after']
            terminal(p['observations'], play)
            signs(p['signs'], 1)
            output_times(p['output'], 1)
            v = p['output']['outputs'][0]
            expected = (i + 32).to_bytes(8, 'little')
            assert v['port'] == 'output' and v['value_kind'] == 'value/u64' and v['sequence'] == 0 and bytes(v['canonical']) == expected, 'Value matrix output differs'
            canonical.append(expected)
        digest = hashlib.sha256(b''.join(canonical)).hexdigest()
        assert row['ordered_output_sha256'] == digest
        by_workload.setdefault((g, row['repetition']), set()).add(digest)
        receipt['value_matrix'].append({'case': row['case'], 'mode': mode, 'repetition': row['repetition'], 'full_host_call': row['full_host_call'], 'ordered_output_sha256': digest, 'distinct_value_plays': 64, 'eligibility': e})
    assert all(len(v) == 1 for v in by_workload.values())
    return receipt
