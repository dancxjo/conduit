#!/usr/bin/env python3
"""Verify hosted runtime capture hashes, raw statistics and output custody."""
import argparse
import collections
import hashlib
import json
import math
import statistics
from pathlib import Path


def verify_pipeline_work(root, status, row, fragment):
    gears = row['gears']
    assert type(gears) is int and gears in [1, 8, 32]
    placements = {p['placement_id'] for p in fragment['placements']}
    assert len(placements) == len(fragment['placements']) == gears, 'pipeline Gear count/Plan placements'
    connections = fragment['connections']
    assert len(connections) == gears - 1, 'pipeline internal Cord count'
    successors = {}
    sinks = set()
    for cord in connections:
        source, sink = cord['source_placement_id'], cord['sink_placement_id']
        assert source in placements and sink in placements
        assert source not in successors and sink not in sinks, 'pipeline is not one chain'
        assert cord['track'] == 'payload' and cord['value_kind'] == 'value/u64'
        successors[source] = sink
        sinks.add(sink)
    starts = placements - sinks
    assert len(starts) == 1
    current = next(iter(starts))
    start = current
    visited = set()
    while current not in visited:
        visited.add(current)
        if current not in successors:
            break
        current = successors[current]
    assert visited == placements and current not in successors, 'pipeline chain disconnected or cyclic'
    fronts = {f['direction']: f for f in fragment['fore_ports']}
    assert len(fragment['fore_ports']) == len(fronts) == 2
    assert fronts['Input']['placement_id'] == start and fronts['Output']['placement_id'] == current
    assert all(f['track'] == 'payload' and f['value_kind'] == 'value/u64' for f in fronts.values())
    name = f'sources/hosted-pipeline-{gears}-gears-32-adds.plot'
    assert name in status['artifact_sha256']
    source = (root / name).read_bytes()
    assert hashlib.sha256(source).hexdigest() == row['source_sha256'], 'pipeline source custody'
    # Lexical counts apply only to this frozen, literal arithmetic fixture.
    assert source.count(b'+') == 32 and source.count(b' >> (') == gears
    if row['phase'] == 'finite-ordinary-hosted-pipeline':
        assert row['adds_per_value'] == 32
        assert row['case'] == f'hosted-pipeline-{gears}-gears-32-adds'
    else:
        assert row['mode'] in ['fused', 'local-unfused']
        assert row['case'] == f"pure-chain-{gears}-{row['mode']}"
        assert row['logical_gear_count'] == gears and row['logical_internal_cord_count'] == gears - 1
        assert row['physical_regions'] == len(fragment['execution_fusions'])
        work = row['work_accounting']
        assert work['logical_evaluations'] == gears * 64, 'pipeline logical work count'
        assert work['intermediate_materializations'] == (gears - 1) * 64, 'pipeline intermediate materializations'
        assert work['intermediate_canonical_bytes'] == (gears - 1) * 64 * 8, 'pipeline intermediate bytes'
        assert work['internal_copy_counter'] is None
        assert len(fragment['execution_fusions']) == int(row['mode'] == 'fused' and gears > 1)
        for fusion in fragment['execution_fusions']:
            assert len(fusion['preserved_placements']) == gears and set(fusion['preserved_placements']) == placements
            ids = {c['connection_id'] for c in connections}
            assert len(ids) == len(connections)
            assert len(fusion['preserved_connections']) == gears - 1 and set(fusion['preserved_connections']) == ids, 'fusion logical connection preservation'


def verify_phase_wall(row):
    phase = row['kernel_phase_wall']
    fields = ['call_wall_ns', 'before_preparation_ns', 'back_preparation_ns', 'play_ns', 'after_play_ns']
    assert all(type(phase[name]) is int and phase[name] >= 0 for name in fields), 'invalid phase duration'
    offsets = phase['marker_offsets_ns']
    assert len(offsets) == 3 and all(type(v) is int and v >= 0 for v in offsets)
    before, start, finish = offsets
    assert 0 <= before <= start <= finish <= phase['call_wall_ns'], 'phase markers not monotonic'
    assert [phase[name] for name in fields[1:]] == [before, start - before, finish - start, phase['call_wall_ns'] - finish], 'phase partition differs'
    assert phase['call_wall_ns'] <= row['full_host_call']['wall_ns'], 'phase call exceeds measured outer call'
    return phase

def verify(root, output):
    root = root.resolve()
    assert not output.resolve().is_relative_to(root), "receipt must be outside immutable capture"
    status = json.loads((root / 'status.json').read_text())
    assert status['worker_success'] and status['source_custody_verified'] and status['conformance_success']
    assert status['git_sha'] == status['final_git_sha']
    assert status['config']['hosted_pipeline'], 'hosted-pipeline capture required'
    files = {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file()}
    assert files == set(status['artifact_sha256']) | {'status.json'}, 'capture manifest incomplete'
    for name, digest in status['artifact_sha256'].items():
        p = (root / name).resolve()
        assert p.is_relative_to(root)
        assert hashlib.sha256(p.read_bytes()).hexdigest() == digest, name
    conformance = json.loads((root / 'conformance.json').read_text())
    assert conformance['success'] and conformance['git_sha'] == status['git_sha']
    metadata = json.loads((root / 'metadata.json').read_text())
    assert metadata['git_sha'] == status['git_sha'] and metadata['source_status'] == 'clean'
    assert metadata['profile'] == 'release' and (not metadata['debug_assertions'])
    report = json.loads((root / 'report.json').read_text())
    raw = [json.loads(line) for line in (root / 'raw.jsonl').read_text().splitlines() if line]
    assert len(raw) == report['raw_records']
    repetitions = status['config']['repetitions']
    source_hashes = {hashlib.sha256(p.read_bytes()).hexdigest() for p in (root / 'sources').glob('*.plot')}
    expected = {}
    checksums = collections.defaultdict(set)
    pipelines = []
    for row in raw:
        phase = row.get('phase')
        mode = None
        metrics = {}
        if 'source_sha256' in row:
            assert row['source_sha256'] in source_hashes
        if phase == 'warm-expression':
            run = row['run']
            assert run['values'] == status['config']['values']
            mode = run['mode']
            metrics = {'bulk_wall_ns': run['metrics']['wall_ns'], 'bulk_cpu_ns': run['metrics']['process_cpu_ns']}
            checksums[row['case']].add((run['validation_sum'], run['validation_sha256']))
        elif phase in ['pool-selection', 'typed-cost-selection']:
            mode = 'production-selection-isolation' if phase == 'pool-selection' else 'production-typed-cost-selection-isolation'
            metrics = {'selection_wall_ns': row['run']['summed_selection_wall_ns']}
        elif phase in ['finite-ordinary-hosted-pipeline', 'finite-ordinary-fusion-pipeline']:
            mode = row.get('mode', 'finite-ordinary-hosted-pipeline')
            metrics = {'full_host_call': row['full_host_call']}
            pipelines.append(row)
            if 'kernel_phase_wall' in row:
                metrics['kernel_phase_wall'] = verify_phase_wall(row)
        if mode is not None:
            key = (row['case'], mode, row['repetition'])
            assert key not in expected, key
            expected[key] = metrics
    summaries = {}
    for row in report['runs']:
        key = (row['case'], row.get('mode', 'finite-ordinary-hosted-pipeline'), row['repetition'])
        assert key not in summaries
        summaries[key] = row
        assert key in expected, key
        for name, value in expected[key].items():
            assert row[name] == value, (key, name)
    assert summaries.keys() == expected.keys()
    assert len(checksums) == 12 and all(len(digests) == 1 for digests in checksums.values())
    # Derive batch samples from raw records, independently of report order.
    groups = collections.defaultdict(list)
    for (case, mode, _), metrics in expected.items():
        for name, value in metrics.items():
            if name == 'full_host_call':
                for field in ['wall_ns', 'process_cpu_ns']:
                    if value.get(field) is not None:
                        groups[case, mode, name + '.' + field].append(value[field])
            elif name == 'kernel_phase_wall':
                for field in ['call_wall_ns', 'before_preparation_ns', 'back_preparation_ns', 'play_ns', 'after_play_ns']:
                    groups[case, mode, name + '.' + field].append(value[field])
            elif value is not None:
                groups[case, mode, name].append(value)
    seen = set()
    for row in report['repeated_batches']:
        key = (row['case'], row['mode'], row['metric'])
        assert key not in seen
        seen.add(key)
        samples = groups[key]
        assert samples == row['samples_ns']
        assert len(samples) == repetitions
        ordered = sorted(samples)
        d = row['distribution']
        assert d['count'] == len(samples)
        for field, expected_value in [('min_ns', min(samples)), ('max_ns', max(samples)), ('p50_ns', ordered[math.ceil(len(samples) * 0.5) - 1]), ('p95_ns', ordered[math.ceil(len(samples) * 0.95) - 1]), ('p99_ns', ordered[math.ceil(len(samples) * 0.99) - 1])]:
            assert d[field] == expected_value
        assert math.isclose(d['mean_ns'], statistics.mean(samples), rel_tol=1e-12, abs_tol=1e-06), 'repeated batch mean differs from raw samples'
        assert math.isclose(d['sample_standard_deviation_ns'], statistics.stdev(samples), rel_tol=1e-12, abs_tol=1e-06)
    assert seen == groups.keys()
    expected_digest = hashlib.sha256(b''.join(((i + 32).to_bytes(8, 'little') for i in range(64)))).hexdigest()
    ordinary = []
    identities = []
    boots = set()
    plays = set()
    for row in pipelines:
        assert row['values'] == 64 and row['ordered_output_sha256'] == expected_digest
        assert row['value_capacity_before'] == row['value_capacity_after']
        assert sum((e['kind'] == 'RemoteValueDelivered' for e in row['signs'])) == 64, 'mandatory output Sign count'
        sequences = [e['sequence'] for e in row['signs']]
        assert sequences == sorted(set(sequences))
        # Dynamic names must identify files already covered by the manifest.
        prefix = f"sources/{row['case']}-{row['repetition']}"
        assert prefix + '-plan.json' in status['artifact_sha256']
        assert prefix + '-host.json' in status['artifact_sha256']
        plan = json.loads((root / (prefix + '-plan.json')).read_text())
        host = json.loads((root / (prefix + '-host.json')).read_text())
        play = row['active_play']
        assert plan['plan_id'] == play['plan_id'], 'pipeline Plan custody'
        assert row.get('plan_id', play['plan_id']) == play['plan_id'], 'pipeline Plan custody'
        assert row.get('checked_plot_id', plan['checked_plot_id']) == plan['checked_plot_id']
        assert (host['host_id'], host['boot_id']) == (play['host_id'], play['boot_id']), 'pipeline Host/Boot custody'
        assert len(plan['fragments']) == 1
        fragment = plan['fragments'][0]
        verify_pipeline_work(root, status, row, fragment)
        assert (fragment['host_id'], fragment['boot_id'], fragment['plan_id']) == (play['host_id'], play['boot_id'], play['plan_id']), 'pipeline fragment custody'
        assert fragment['offer_generation'] == host['offer_generation']
        boot = (play['host_id'], play['boot_id'])
        assert boot not in boots and play['active_play_id'] not in plays, 'pipeline execution identity reused'
        boots.add(boot)
        plays.add(play['active_play_id'])
        identities.append({'case': row['case'], 'repetition': row['repetition'], 'active_play': play})
        if row['phase'] == 'finite-ordinary-hosted-pipeline':
            for observation in row['observations']:
                assert observation['host_id'] == play['host_id'] and observation['boot_id'] == play['boot_id']
                for key in ['plan_id', 'active_play_id']:
                    assert observation.get(key) in [None, play[key]]
            ordinary.append({
                'case': row['case'],
                'repetition': row['repetition'],
                'gears': row['gears'],
                'plan_id': row['plan_id'],
                'active_play': play,
                'full_host_call': row['full_host_call'],
                'kernel_decisions': row['kernel_decisions'],
                'kernel_logical_invocations': row['kernel_logical_invocations'],
                'output_sha256': expected_digest,
            })
    assert {r['gears'] for r in ordinary} == {1, 8, 32} and len(ordinary) == 3 * repetitions
    phase_count = sum('kernel_phase_wall' in row for row in pipelines)
    assert phase_count in [0, len(pipelines)], 'phase observation missing from some pipeline trials'
    if metadata.get('hosted_phase_wall_schema') is not None:
        assert metadata['hosted_phase_wall_schema'] == 'installed-boundaries-wall@1'
        assert phase_count == len(pipelines), 'declared phase observation absent'
    receipt = {
        'source': status['git_sha'],
        'verified_artifact_hashes': len(status['artifact_sha256']),
        'raw_records': len(raw),
        'verified_repeated_distributions': len(groups),
        'expression_cases_with_cross_mode_output_parity': len(checksums),
        'expression_outputs': {case: {'validation_sum': next(iter(digests))[0],
                                      'validation_sha256': next(iter(digests))[1]}
                               for case, digests in sorted(checksums.items())},
        'hosted_rows': ordinary,
        'all_pipeline_rows': len(pipelines),
        'verified_phase_wall_partitions': phase_count,
        'verified_pipeline_execution_identities': identities,
        'ordered_pipeline_output_sha256': expected_digest,
        'repeated_batches': report['repeated_batches'],
        'metadata': metadata,
        'artifact_sha256': status['artifact_sha256'],
        'process_resident_high_water': report['process_resident_high_water'],
        'verifier_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'scope': report['scope'],
        'limits': report['confidence'],
    }
    with output.open('x') as stream:
        json.dump(receipt, stream, indent=2)
        stream.write('\n')
    print(output)

def main():
    if not __debug__:
        raise SystemExit('verification requires Python assertions; do not use -O')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    verify(args.capture, args.output)

if __name__ == "__main__":
    main()
