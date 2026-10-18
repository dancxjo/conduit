#!/usr/bin/env python3
"""Verify retained full-frame FARGAN phases and exact before/after semantics."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import statistics


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    return json.loads(path.read_text())


def distribution(values):
    values = sorted(values)
    return {"count": len(values), "mean": statistics.mean(values),
            "sample_stddev": statistics.stdev(values) if len(values) > 1 else None,
            "min": values[0], "p50": values[math.ceil(.5*len(values))-1],
            "p95": values[math.ceil(.95*len(values))-1], "max": values[-1]}


def verify(root):
    status = read(root / "status.json")
    require(status["schema"] == "conduit.fargan-runtime-performance-command@1", "command schema")
    require(status["conformance_success"] and status["worker_success"] and
            status["source_custody_verified"] and status["worker_exit_code"] == 0, "command refused")
    sha = status["git_sha"]
    require(sha == status["final_git_sha"] == read(root / "metadata.json")["git_sha"], "source custody")
    manifest = status["artifact_sha256"]
    actual = {str(p.relative_to(root)): digest(p) for p in root.rglob('*')
              if p.is_file() and p.name != 'status.json'}
    require(actual == manifest, "artifact hash/coverage mismatch")
    model = read(root / "model-profile.json")
    require(model["f32_bundle_sha256"] == "d35f0510da476716183a33caafe45cc095e48d496f06de2db7655e780a385e47", "model pin")
    require(model["full_frame_oracle_sha256"] == "9c676f6c76aec71281ea097fe0c477c5a3b77f5022e639a0d82dbb4a5437fcc3", "oracle pin")
    require(model["parameters_published"] is False, "private parameters boundary")
    summary = read(root / "summary.json")
    require(summary["schema"] == "conduit.fargan-runtime-performance@1", "summary schema")
    require(summary["source_sha256"] == digest(root / "source.conduit"), "Source pin")
    repetitions = summary["repetitions"]
    require(2 <= repetitions <= 20 and repetitions == status["repetitions"], "repetition bound")
    records = [json.loads(line) for line in (root / 'raw.jsonl').read_text().splitlines()]
    require(len(records) == 3 + repetitions * 25, "raw phase coverage")
    controls = [r for r in records if r['phase'] == 'timer-control']
    require(len(controls) == 1 and len(controls[0]['samples_ns']) == 2000 and
            controls[0]['subtracted'] is False, "timer control")
    require(len([r for r in records if r['phase'] == 'cold-source-schema']) == 1, "cold schema")
    require(len([r for r in records if r["phase"] == "cold-model-resource-admission"]) == 1, "cold model admission")
    plans = [r for r in records if r['phase'] == 'cold-check-expand-plan']
    require(sorted(r['repetition'] for r in plans) == list(range(repetitions)), "cold plans")
    rows = [r for r in records if r['phase'] == 'fargan-frame-play']
    require(sorted((r['repetition'], r['frame']) for r in rows) ==
            [(r, f) for r in range(repetitions) for f in range(24)], "frame coverage")
    by_frame = {}
    for row in rows:
        require(row['nodes'] == 768 and row['cords'] == 1211 and row['useful_pcm_samples'] == 160, "real graph shape")
        require(row['structural_drain'] and row['pending_host_calls_at_end'] == 0, "terminal/call custody")
        require(row['allocator']['allocations'] == row['allocator']['reallocations'] == 0, "Play allocation")
        require(row['process_cpu_ns'] is not None and row['process_cpu_ns'] > 0, "observed CPU clock")
        require(row['numeric_back_wall_ns'] > 0 and row['step_remainder_ns'] >= 0 and
                row['step_wall_ns'] == row['numeric_back_wall_ns'] + row['step_remainder_ns'], "Step partition")
        require(row['wall_ns'] >= row['step_wall_ns'] + row['expression_host_wall_ns'], "whole Play partition")
        require(row['steps'] > 0 and row['numeric_steps'] > 0 and row['expression_calls'] > 0, "invocation coverage")
        require(row['pcm_max_abs_error'] < .002 and row['active_state_max_abs_error'] < .05 and
                row['conditioning_history_max_abs_error'] < .001, "oracle differential")
        plan = root / f"plan-repetition-{row['repetition']}.json"
        require(row['plan_sha256'] == hashlib.sha256(json.dumps(read(plan),separators=(',',':'),ensure_ascii=False).encode()).hexdigest(), "Plan pin")
        outcome = tuple(row[k] for k in ('plan_sha256', 'output_sha256', 'signs_sha256', 'signs', 'steps', 'numeric_steps', 'expression_calls'))
        prior = by_frame.setdefault(row['frame'], outcome)
        require(prior == outcome, "repeat semantic/Sign mismatch")
    first = next(r for r in rows if r['repetition'] == 0 and r['frame'] == 0)
    trace = root / 'signs-first-frame.txt'
    require(digest(trace) == first['signs_sha256'] and len(trace.read_text().splitlines()) == first['signs'], "Sign trace")
    fields = ('wall_ns','process_cpu_ns','owner_preparation_ns','step_remainder_ns','numeric_back_wall_ns','expression_host_wall_ns')
    return {'source':sha,'source_sha256':summary['source_sha256'],'model':model,'repetitions':repetitions,
            'rows':rows,'phases':{k:distribution([r[k] for r in rows]) for k in fields},
            'scheduler_inline_bytes':sorted({r['scheduler_inline_bytes'] for r in rows}),
            'verified_artifacts':len(actual)}


def compare(before, after):
    require(before['source_sha256'] == after['source_sha256'] and before['model'] == after['model'], "same workload/model required")
    require(before['repetitions'] == after['repetitions'], "same repetitions required")
    keys = ('plan_sha256','output_sha256','signs_sha256','signs','steps','numeric_steps','expression_calls','useful_pcm_samples','nodes','cords')
    baseline = {(r['repetition'],r['frame']):r for r in before['rows']}
    for row in after['rows']:
        prior = baseline[(row['repetition'],row['frame'])]
        require(all(prior[k] == row[k] for k in keys), "before/after semantic/Sign/work mismatch")
    ratios = {}
    for field in before['phases']:
        groups=[]
        for rep in range(before['repetitions']):
            old=sum(r[field] for r in before['rows'] if r['repetition']==rep)
            new=sum(r[field] for r in after['rows'] if r['repetition']==rep)
            groups.append((old,new))
        rng=random.Random(5231)
        bootstrap=[]
        for _ in range(10000):
            sample=rng.choices(groups,k=len(groups))
            bootstrap.append(sum(g[1] for g in sample)/sum(g[0] for g in sample))
        bootstrap.sort()
        ratios[field]={'after_over_before':after['phases'][field]['mean']/before['phases'][field]['mean'],
                       'repetition_cluster_bootstrap_95':[bootstrap[249],bootstrap[9749]]}
    return {'schema':'conduit.fargan-runtime-comparison@1','before_source':before['source'],
            'after_source':after['source'],'semantic_sign_work_pairs_verified':len(after['rows']),
            'ratios':ratios,'before_phases':before['phases'],'after_phases':after['phases'],
            'scheduler_inline_bytes':{'before':before['scheduler_inline_bytes'],'after':after['scheduler_inline_bytes']},
            'scope':'Source/model/Plan/output/Sign/work parity; descriptive shared-host timings, no realtime or universal regression threshold.'}


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('capture',type=Path)
    p.add_argument('--before',type=Path)
    args=p.parse_args()
    result=verify(args.capture)
    if args.before:
        result=compare(verify(args.before),result)
    else:
        result={k:v for k,v in result.items() if k not in ('rows','model')}
    print(json.dumps(result,indent=2,allow_nan=False))


if __name__ == '__main__':
    main()
