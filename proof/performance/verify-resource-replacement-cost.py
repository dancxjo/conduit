#!/usr/bin/env python3
"""Read-only custody, interval, reservation and reference checks for #6101.
The worker verifies actual provider bytes; this checks its retained records.
"""
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import statistics
import sys


def verify(root):
    status = json.loads((root / 'status.json').read_text())
    assert status['schema'] == 'conduit.resource-replacement-cost-command@1'
    assert status['worker_success'] and status['worker_exit_code'] == 0
    assert status['source_custody_verified'] and status['git_sha'] == status['final_git_sha']
    required = {'worker.json', 'plans.json', 'measurements.json', 'stdout.log', 'stderr.log'}
    assert required <= set(status['artifact_sha256'])
    for name, digest in status['artifact_sha256'].items():
        path = root / name
        assert path.resolve().is_relative_to(root.resolve())
        assert hashlib.sha256(path.read_bytes()).hexdigest() == digest, name
    worker = json.loads((root / 'worker.json').read_text())
    assert worker['profile'] == 'release' and worker['debug_assertions'] is False
    assert len(bytes.fromhex(worker['binary_sha256'])) == 32
    plans = json.loads((root / 'plans.json').read_text())
    host, old, new = plans['host'], plans['previous'], plans['candidate']
    assert old['plan_id'] != new['plan_id']
    for key in ['source_document_id', 'checked_plot_id', 'expanded_plot_id']:
        assert old[key] == new[key], key
    old_gear, new_gear = old['fragments'][0]['placements'][0], new['fragments'][0]['placements'][0]
    for key in ['gear_id', 'kind_id', 'kind_contract_revision', 'source_span', 'inputs', 'outputs', 'configuration', 'semantic_contract']:
        assert old_gear.get(key) == new_gear.get(key), key
    assert old_gear['resources'][0]['pool_id'] != new_gear['resources'][0]['pool_id']
    assert old_gear['base'] != new_gear['base']
    data = json.loads((root / 'measurements.json').read_text())
    assert data['schema'] == 'conduit.resource-replacement-cost@1'
    clock = data['clock']
    assert clock['host_id'] == host['host_id'] and clock['boot_id'] == host['boot_id']
    assert clock['scale'] == 'Nanoseconds' and clock['resolution_ticks'] > 0
    assert clock['uncertainty_ticks'] >= 0
    error = 2 * (clock['resolution_ticks'] + clock['uncertainty_ticks'])
    payload_bytes = data['payload_bytes']
    assert payload_bytes == 262144
    expected = hashlib.sha256(bytes(i % 251 for i in range(payload_bytes))).hexdigest()
    assert data['payload_sha256'] == expected
    def fraction(value):
        assert int(value['denominator']) > 0
        result = Fraction(int(value['numerator']), int(value['denominator']))
        assert result >= 0
        return result
    def interval(value):
        assert value['unit'] == 's' and value['role'] == 'linear'
        result = fraction(value['lower']), fraction(value['upper'])
        assert result[0] <= result[1]
        return result
    def phase(value):
        assert value['clock'] == clock
        started, completed = int(value['start_tick']), int(value['completed_tick'])
        elapsed = completed - started
        assert elapsed >= 0 and elapsed == value['elapsed_ns']
        assert interval(value['duration']) == (Fraction(max(0, elapsed - error), 10**9), Fraction(elapsed + error, 10**9))
        for key in ['allocations', 'reallocations', 'deallocations', 'requested_bytes', 'live_heap_bytes_at_end', 'phase_peak_heap_bytes']:
            assert value['metrics']['allocator'][key] >= 0
        return interval(value['duration'])
    def attribution(value, gear, completed):
        assert value['host_id'] == host['host_id'] and value['boot_id'] == host['boot_id']
        assert value['offer_generation'] == host['offer_generation']
        assert value['host_profile'] == host['profile'] and value['clock'] == clock
        assert int(value['observed_tick']) == completed < int(value['expires_tick'])
        assert value['calibration_revision'] == 1
        assert value['calibration_identity'] == 'std/resource-replacement-cost@1'
        assert value['capability'] == gear['capability_id']
        offer = next(c for c in host['capabilities'] if c['capability_id'] == gear['capability_id'])
        assert value['implementation'] == {k: offer[k] for k in ['execution_profile_id', 'implementation_id', 'artifact_id']}
        pool = next(p for p in host['resources'] if p['pool_id'] == gear['resources'][0]['pool_id'])
        assert value['resource'] == pool
        assert value['sign']
    phase(data['initial_resident_preparation'])
    phase(data['final_origin_release_and_disposal'])
    assert data['final_origin_release_and_disposal']['metrics']['allocator']['deallocations'] > 0
    rows = data['trials']
    assert 3 <= status['repetitions'] <= 20 and len(rows) == 2 * status['repetitions']
    assert len({r['trial'] for r in rows}) == len(rows)
    summary = {}
    target_handles = []
    for row in rows:
        moved = row['mode'] == 'prepared-local-copy'
        assert row['mode'] in ['retained-local', 'prepared-local-copy']
        candidate = new if moved else old
        candidate_gear = new_gear if moved else old_gear
        assert row['previous_plan'] == old['plan_id'] and row['candidate_plan'] == candidate['plan_id']
        assert row['origin_resident'] is True and row['payload_sha256'] == expected
        assert row['copied_payload_bytes'] == (payload_bytes if moved else 0)
        residences = row['residences']
        assert len(residences) == (2 if moved else 1)
        by_plan = {old['plan_id']: old_gear, new['plan_id']: new_gear}
        for residence in residences:
            gear = by_plan[residence['plan']]
            assert residence['placement'] == gear['placement_id']
            assert residence['binding'] == gear['resources'][0]
            assert residence['resident_bytes'] == payload_bytes and residence['handle']
            content = residence['binding']['content']['contract']
            assert content['maximum_bytes'] == payload_bytes and content['retention'] == 'Play'
            assert bytes(content['version']) == bytes.fromhex(expected)
            assert content['sharing'] == 'ImmutableReadMany' and content['generation_slots'] == 1
            match = [a for a in residence['reservation'] if a['plan_id'] == residence['plan'] and a['placement_id'] == residence['placement']]
            assert len(match) == 1 and any(i['binding'] == residence['binding'] for i in match[0]['items'])
            assert match[0]['host_id'] == host['host_id'] and match[0]['boot_id'] == host['boot_id']
            assert match[0]['offer_generation'] == host['offer_generation']
            attribution(residence['attribution'], gear, int(row['stages'][2]['observation']['completed_tick']))
        if moved:
            assert residences[0]['reference_bytes'] == residences[1]['reference_bytes']
            assert residences[0]['handle'] != residences[1]['handle']
            target_handles.append(residences[1]['handle'])
        assert [s['stage'] for s in row['stages']] == ['Preparation', 'Transfer', 'Residency', 'Migration', 'Continuity']
        assert len({s['sign'] for s in row['stages']}) == 5
        lower = upper = Fraction(0)
        for index, stage in enumerate(row['stages']):
            gear = old_gear if index == 4 else candidate_gear
            plan = old if index == 4 else candidate
            assert stage['plan'] == plan['plan_id'] and stage['sign'] == stage['attribution']['sign']
            lo, hi = phase(stage['observation'])
            lower, upper = lower + lo, upper + hi
            attribution(stage['attribution'], gear, int(stage['observation']['completed_tick']))
            residence = next(r for r in residences if r['plan'] == plan['plan_id'])
            assert stage['resource_reference_bytes'] == residence['reference_bytes']
        assert interval(row['sequential_overhead']) == (lower, upper)
        phase(row['cost_admission_and_report_construction'])
        phase(row['target_reservation_release_and_disposal'])
    assert len(set(target_handles)) == status['repetitions'], 'fresh targets reused an opaque handle'
    for mode in ['retained-local', 'prepared-local-copy']:
        selected = [r for r in rows if r['mode'] == mode]
        assert len(selected) == status['repetitions']
        summary[mode] = {
            'trials': len(selected),
            'stages_elapsed_ns_median': {s: statistics.median(r['stages'][i]['observation']['elapsed_ns'] for r in selected) for i, s in enumerate(['preparation', 'transfer', 'residency', 'migration', 'continuity'])},
            'admission_report_elapsed_ns_median': statistics.median(r['cost_admission_and_report_construction']['elapsed_ns'] for r in selected),
            'disposal_elapsed_ns_median': statistics.median(r['target_reservation_release_and_disposal']['elapsed_ns'] for r in selected),
            'copied_resource_payload_bytes': selected[0]['copied_payload_bytes'],
        }
    return {'source': status['git_sha'], 'verified_trials': len(rows), 'profiles': summary, 'scope': 'retained records from actual local providers; no Body migration, remote transport or hardware-copy-counter claim'}


if __name__ == '__main__':
    print(json.dumps(verify(Path(sys.argv[1])), indent=2))
