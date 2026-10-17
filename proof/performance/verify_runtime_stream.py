"""Independent exact source, output, Plan/Play and timing custody for std streams."""
import collections
import hashlib
import math
import statistics


def cases():
    configurations = [dict(branches=b, queue=q, batch=a, payload_bytes=8, text=False, slow_sink_us=20)
                      for b in [1, 4] for q in [1, 4] for a in [1, 8]]
    configurations += [dict(branches=b, queue=1, batch=8, payload_bytes=8, text=False, slow_sink_us=0) for b in [1, 4]]
    configurations += [dict(branches=4, queue=1, batch=8, payload_bytes=n, text=True, slow_sink_us=20) for n in [32, 256]]
    for c in configurations:
        c['chain_gears'] = 0
    configurations += [dict(branches=1, chain_gears=g, queue=1, batch=8, payload_bytes=8, text=False, slow_sink_us=0) for g in [8, 32]]
    return {f"stream-{c['branches']}-branches-chain{c['chain_gears']}-q{c['queue']}-batch{c['batch']}-bytes{c['payload_bytes']}-slow{c['slow_sink_us']}": c for c in configurations}


def authored(c):
    def ones(n):
        return '1' if n == 1 else f'({ones(n // 2)} + {ones(n - n // 2)})'
    if c['chain_gears']:
        expression = '. + ' + ones(32 // c['chain_gears'])
        return ('plot baseline (\n >> input: U64...|\n output0: U64...| >>\n) {\n input' +
                f' >> ({expression})' * c['chain_gears'] + ' >> output0\n}\n').encode()
    ty = 'Text' if c['text'] else 'U64' 
    expression = '.' if c['text'] else '. + ' + ones(32)
    return (f'plot baseline (\n >> input: {ty}...|\n' +
            ''.join(f' output{b}: {ty}...| >>\n' for b in range(c['branches'])) + ') {\n' +
            ''.join(f' input >> ({expression}) >> output{b}\n' for b in range(c['branches'])) + '}\n').encode()


def output_bytes(c, i):
    return (f'{i:08}' + 'x' * (c['payload_bytes'] - 8)).encode() if c['text'] else (i + 32).to_bytes(8, 'little')


def distribution(samples, d):
    ordered = sorted(samples)
    assert d['count'] == len(samples)
    for name, expected in [('min_ns', min(samples)), ('max_ns', max(samples)),
                           ('p50_ns', ordered[math.ceil(len(samples) * .5) - 1]),
                           ('p95_ns', ordered[math.ceil(len(samples) * .95) - 1]),
                           ('p99_ns', ordered[math.ceil(len(samples) * .99) - 1])]:
        assert d[name] == expected, 'stream latency percentile differs'
    assert math.isclose(d['mean_ns'], statistics.mean(samples), rel_tol=1e-12, abs_tol=1e-6)
    assert math.isclose(d['sample_standard_deviation_ns'], statistics.stdev(samples), rel_tol=1e-12, abs_tol=1e-6)


def verify_streams(root, status, rows, metadata):
    import json
    streams = [r for r in rows if r.get('phase') == 'bounded-ordinary-stream']
    declared = metadata.get('hosted_stream_schema')
    if not streams and declared is None:
        return []
    assert declared == 'bounded-overlapping-std@1', 'stream schema missing or unsupported'
    configurations = cases()
    required = {(case, i) for case in configurations for i in range(status['config']['repetitions'])}
    assert {(r['case'], r['repetition']) for r in streams} == required and len(streams) == len(required), 'stream matrix incomplete or repeated'
    identities = set()
    receipt = []
    for r in streams:
        c = configurations[r['case']]
        assert r['configuration'] == c and r['mode'] == 'bounded-stream'
        n = r['input_items']
        assert n == 1024 and r['useful_outputs'] == n * c['branches']
        source = authored(c)
        assert (root / f"sources/{r['case']}.plot").read_bytes() == source, 'stream authored workload differs'
        assert r['source_sha256'] == hashlib.sha256(source).hexdigest()
        prefix = f"sources/{r['case']}-{r['repetition']}"
        assert all(prefix + suffix in status['artifact_sha256'] for suffix in ['-plan.json', '-host.json'])
        plan = json.loads((root / (prefix + '-plan.json')).read_text())
        host = json.loads((root / (prefix + '-host.json')).read_text())
        play = r['active_play']
        assert len(plan['fragments']) == 1 and r['checked_plot_id'] == plan['checked_plot_id']
        fragment = plan['fragments'][0]
        assert plan['plan_id'] == fragment['plan_id'] == play['plan_id'], 'stream Plan custody'
        assert (host['host_id'], host['boot_id']) == (fragment['host_id'], fragment['boot_id']) == (play['host_id'], play['boot_id']), 'stream Host/Boot custody'
        assert fragment['offer_generation'] == host['offer_generation']
        assert play['active_play_id'] not in identities, 'stream Play reused'
        identities.add(play['active_play_id'])
        gears = c['chain_gears'] or c['branches']
        assert len(fragment['placements']) == gears and not fragment['execution_fusions']
        cords = fragment['connections']
        assert len(cords) == (gears - 1 if c['chain_gears'] else 0)
        if c['chain_gears']:
            placements = {p['placement_id'] for p in fragment['placements']}
            successors = {cord['source_placement_id']: cord['sink_placement_id'] for cord in cords}
            assert len(successors) == gears - 1 and len(set(successors.values())) == gears - 1
            starts = placements - set(successors.values())
            assert len(starts) == 1
            visited = set()
            current = next(iter(starts))
            while current not in visited:
                visited.add(current)
                if current not in successors:
                    break
                current = successors[current]
            assert visited == placements and current not in successors
            assert all(cord['track'] == 'payload' and cord['value_kind'] == 'value/u64' and cord['item_capacity'] == c['queue'] for cord in cords)
        assert len(fragment['fore_ports']) == c['branches'] * 2
        fronts = fragment['fore_ports']
        assert sum(p['direction'] == 'Input' and p['front_port_id'] == 'input' for p in fronts) == c['branches']
        assert {p['front_port_id'] for p in fronts if p['direction'] == 'Output'} == {f'output{b}' for b in range(c['branches'])}
        assert all(p['track'] == 'payload' and p['item_capacity'] == c['queue'] and p['byte_capacity'] == c['payload_bytes'] * c['queue'] for p in fronts)
        assert r['value_capacity_before'] == r['value_capacity_after']
        ingress = r['ingress']
        assert ingress['admission_batch'] == c['batch'] and ingress['accepted'] == n
        assert ingress['attempts'] == n + ingress['full'] and ingress['batches'] > 0
        if c['queue'] == 1 and c['batch'] == 8:
            assert ingress['full'] > 0, 'stream pressure not witnessed'
        o = r['output_observations']
        assert o['per_branch_counts'] == [n] * c['branches']
        expected = [output_bytes(c, i) for i in range(n)]
        digest = hashlib.sha256(b''.join(expected) * c['branches']).hexdigest()
        assert o['ordered_output_sha256'] == digest, 'stream ordered output digest differs'
        counts = [0] * c['branches']
        last = [None] * c['branches']
        gaps = [0] * c['branches']
        records = o['output_receipts']
        assert len(records) == n * c['branches']
        for index, item in enumerate(records):
            b = item['branch']
            assert type(b) is int and 0 <= b < c['branches']
            assert item['sequence'] == counts[b] and item['bytes'] == c['payload_bytes'], 'stream output order/size differs'
            assert item['sha256'] == hashlib.sha256(expected[counts[b]]).hexdigest(), 'stream output payload differs'
            if last[b] is not None:
                gaps[b] = max(gaps[b], index - last[b] - 1)
            last[b] = index
            counts[b] += 1
        assert counts == [n] * c['branches'] and gaps == o['max_other_acknowledgements_between_branch_outputs']
        offsets = o['acknowledgement_offsets_ns']
        assert len(offsets) == len(records) and offsets == sorted(offsets) and all(type(v) is int and v >= 0 for v in offsets)
        interval = offsets[-1] - offsets[0]
        assert interval > 0 and o['first_to_last_acknowledgement_ns'] == interval and o['completion_window_outputs'] == len(records) - 1
        assert math.isclose(o['completion_window_useful_outputs_per_second'], (len(records) - 1) * 1e9 / interval, rel_tol=1e-12)
        distribution([b - a for a, b in zip(offsets, offsets[1:])], o['inter_acknowledgement_distribution'])
        assert {v['mode'] for v in r['native_controls']} == {'native-fused-loop', 'native-library-pipeline'} and len(r['native_controls']) == 2
        assert all(v['ordered_output_sha256'] == digest for v in r['native_controls'])
        signs = r['signs']
        assert [e['sequence'] for e in signs] == sorted({e['sequence'] for e in signs})
        assert sum(e['kind'] == 'RemoteValueDelivered' for e in signs) == len(records), 'stream mandatory output Signs differ'
        assert r['canonical_input_bytes'] == n * c['payload_bytes'] and r['canonical_output_bytes'] == n * c['branches'] * c['payload_bytes']
        assert r['internal_copy_counter'] is None
        terminal = [o for o in r['observations'] if isinstance(o['kind'], dict) and o['kind'].get('PlanTerminal') is not None]
        assert len(terminal) == 1 and terminal[0]['kind']['PlanTerminal']['disposition'] == 'Completed', 'stream terminal differs'
        for observation in r['observations']:
            assert (observation['host_id'], observation['boot_id']) == (play['host_id'], play['boot_id'])
            for key in ['plan_id', 'active_play_id']:
                assert observation.get(key) in [None, play[key]], 'stream observation custody'
        receipt.append({'case': r['case'], 'repetition': r['repetition'], 'active_play': play,
                        'full_host_call': r['full_host_call'], 'kernel_phase_wall': r['kernel_phase_wall'],
                        'ordered_output_sha256': digest, 'ingress': ingress,
                        'useful_outputs': len(records), 'max_other_acknowledgements_between_branch_outputs': gaps,
                        'completion_window_useful_outputs_per_second': o['completion_window_useful_outputs_per_second']})
    return receipt
