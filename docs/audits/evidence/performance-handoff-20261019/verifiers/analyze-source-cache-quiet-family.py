#!/usr/bin/env python3
"""Preregistered accounting; reject an incomplete or contaminated whole family."""
import argparse,hashlib,importlib.util,json,statistics
from pathlib import Path

def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def module(name):
    p=Path(__file__).with_name(name);s=importlib.util.spec_from_file_location(name,p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m

def account(root):
    rows=[json.loads(s) for s in (root/'raw.jsonl').read_text().splitlines()]
    def phases(names,field='wall_ns'):
        selected=[r['metrics'][field] for r in rows if r['phase'] in names];assert all(isinstance(n,int) and n>=0 for n in selected);return sum(selected)
    frames=[r for r in rows if r['phase']=='paired-fargan-frame'];assert len(frames)==96
    fresh={(r['repetition'],r['frame']):r for r in frames if r['profile']=='fresh'};candidate={(r['repetition'],r['frame']):r for r in frames if r['profile']=='prepared-matrix-source-cache'};assert set(fresh)==set(candidate) and len(fresh)==48
    common=phases({'cold-source-schema','cold-model-resource-admission','frame-input-encoding'})
    reference=common+phases({'cold-check-expand-plan','fresh-call-bindings'})+sum(r['metrics']['whole_call_wall_ns'] for r in fresh.values())
    retained=common+phases({'candidate-check-expand-plan','initial-input-encoding','cold-prepared-trajectory','prepared-rearm','previous-evidence-release','output-acceptance-state-advance','prepared-trajectory-release','paired-frame-result-release','source-cache-owner-creation','source-cache-owner-release'})+sum(r['metrics']['whole_call_wall_ns'] for r in candidate.values())
    ratios=[candidate[k]['metrics']['play']['wall_ns']/fresh[k]['metrics']['play']['wall_ns'] for k in sorted(fresh)]
    cold=next(r['metrics'] for r in rows if r['phase']=='candidate-cold-check-expand-plan' and r['repetition']==1);warm=next(r['metrics'] for r in rows if r['phase']=='candidate-check-expand-plan' and r['repetition']==1)
    return {'paired_frames':48,'reference_accounted_wall_ns':reference,'candidate_accounted_wall_ns':retained,'accounted_ratio':retained/reference,'play_ratios':ratios,'median_play_ratio':statistics.median(ratios),'play_ratio_range':[min(ratios),max(ratios)],'warm_source_plan_ratio':warm['wall_ns']/cold['wall_ns'],'warm_source_plan_cpu_ratio':warm['process_cpu_ns']/cold['process_cpu_ns'],'two_plan_source_cache_net_ratio':phases({'candidate-check-expand-plan','source-cache-owner-creation','source-cache-owner-release'})/phases({'candidate-cold-check-expand-plan'}),'separate_validation_and_inspection_wall_ns':phases({'candidate-cold-check-expand-plan','paired-evidence-reconstruction-and-write'})}

def verify_monitor(root,reg,family,launch):
    assert family['monitor_sha256']==sha(root/'monitor.jsonl')
    rows=[json.loads(s) for s in (root/'monitor.jsonl').read_text().splitlines()];assert rows
    monitor_pid=launch['monitor_pid'];pid_by_index={p['index']:p['pid'] for p in family['processes']};prior=None;prior_cpu={};hz=reg['monitor']['cpu_clock_ticks_per_second']
    blocked={'cargo','cargo-clippy','rustc','clippy-driver','rust-lld','ld','lld','rustdoc','cc','cc1','cc1plus','c++','clang','clang++','gcc','g++','collect2','ld.bfd','ld.gold','ld.lld','wasm-ld','wasm-opt','llvm-ar','make','ninja','cmake','meson','avr-gcc'}
    for row in rows:
        now=row['monotonic_ns'];assert prior is None or 0<now-prior<=2_000_000_000;prior=now
        assert not row['competing'];worker_pid=pid_by_index[row['index']] if row['stage'] in {'worker','worker-end'} else None
        for p in row['processes']:
            if p['state']=='Z':continue
            assert p['comm'] not in blocked and not p['comm'].startswith(('qemu','build-script','chrome','chromium','firefox'))
            if p['pid'] not in {monitor_pid,worker_pid} and p['pid'] in prior_cpu:assert p['cpu_ticks']-prior_cpu[p['pid']]<hz/10
        prior_cpu={p['pid']:p['cpu_ticks'] for p in row['processes']}
    for i in range(1,4):
        pre=[r for r in rows if r['stage']=='preflight' and r['index']==i];assert len(pre)==13 and pre[-1]['monotonic_ns']-pre[0]['monotonic_ns']>=6_000_000_000
        assert any(r['stage']=='worker' and r['index']==i for r in rows)
        assert sum(r['stage']=='worker-end' and r['index']==i for r in rows)==1
    return {'observations':len(rows),'all_forbidden_presence_and_other_cpu_checks_passed':True}

def analyze(root,registration,worker,launch_receipt):
    reg=json.loads(registration.read_text());family=json.loads((root/'family.json').read_text());launch=json.loads(launch_receipt.read_text())
    assert launch['schema']=='conduit.quiet-monitor-launch@1' and launch['launcher_sha256']==sha(Path(__file__).with_name('launch-source-cache-quiet-family.py'))
    assert family['complete'] is True and family['timing_excluded'] is False and not family['causes']
    assert family['source']==reg['source'] and family['worker_sha256']==reg['worker_sha256']==sha(worker)
    assert family['registration_sha256']==launch['registration_sha256']==sha(registration)
    assert family['controller_sha256']==launch['controller_sha256']==reg['controller_sha256']==sha(Path(__file__).with_name('run-source-cache-quiet-family.py'))
    assert reg['repeat_verifier_sha256']==sha(Path(__file__).with_name('verify-source-cache-repeat.py')) and reg['capture_verifier_sha256']==sha(Path(__file__).with_name('verify-source-cache-capture.py'))
    assert len(family['processes'])==3 and [p['index'] for p in family['processes']]==[1,2,3]
    monitor=verify_monitor(root,reg,family,launch);results=[];replay=module('verify-source-cache-repeat.py')
    for p in family['processes']:
        assert p['exit_code']==0 and p['source_custody'] is True and p['affinity_verified'] is True
        capture=root/p['capture'];assert p['status_sha256']==sha(capture/'status.json')
        proof=replay.verify(capture,registration,worker);metrics=account(capture);results.append({'index':p['index'],'conformance':proof,**metrics})
    return {'schema':'conduit.source-cache-quiet-family-analysis@1','source':reg['source'],'worker_sha256':reg['worker_sha256'],'registration_sha256':sha(registration),'monitor':monitor,'processes':results,'material_composite_lifetime_gain':all(p['accounted_ratio']<.8 for p in results),'play_regression_requires_investigation':any(p['median_play_ratio']>1.05 for p in results),'demonstrated_faster_play':all(p['median_play_ratio']<.95 for p in results),'material_warm_source_preparation_wall_gain':all(p['warm_source_plan_ratio']<.8 for p in results),'material_warm_source_preparation_cpu_gain':all(p['warm_source_plan_cpu_ratio']<.8 for p in results),'material_two_plan_cache_net_gain':all(p['two_plan_source_cache_net_ratio']<.9 for p in results),'scope':'preregistered3-process fixture/real-kernel measurements only; no total process speedup,hardware traffic,total RAM,realtime,ordinary Body adoption or stable release claim'}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('family',type=Path);p.add_argument('--registration',type=Path,required=True);p.add_argument('--worker',type=Path,required=True);p.add_argument('--launch-receipt',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();r=analyze(a.family,a.registration,a.worker,a.launch_receipt)
    with a.output.open('x') as f:json.dump(r,f,indent=2);f.write('\n')
