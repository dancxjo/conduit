#!/usr/bin/env python3
"""Internal repeat controller for an already proved, frozen xtask worker."""
import argparse, hashlib, importlib.util, json, os, shutil, subprocess, time
from pathlib import Path

def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def write(path,value): path.write_text(json.dumps(value,indent=2)+'\n')
def git(root,*args): return subprocess.check_output(['git',*args],cwd=root,text=True).strip()
def inventory():
    rows=[]
    for p in Path('/proc').iterdir():
        if not p.name.isdigit(): continue
        try:
            comm=(p/'comm').read_text().strip(); fields=(p/'stat').read_text().rsplit(')',1)[1].split()
            rows.append({'pid':int(p.name),'comm':comm,'state':fields[0],'cpu_ticks':int(fields[11])+int(fields[12])})
        except (OSError,ValueError,IndexError): continue
    return rows

def heavy(rows):
    names={'cargo','cargo-clippy','rustc','clippy-driver','rust-lld','ld','lld','rustdoc'}
    return [r for r in rows if r['state']!='Z' and (r['comm'] in names or r['comm'].startswith(('qemu','build-script','chrome','chromium','firefox')))]

def main():
    p=argparse.ArgumentParser();p.add_argument('--root',type=Path,required=True);p.add_argument('--worker',type=Path,required=True);p.add_argument('--fixture',type=Path,required=True);p.add_argument('--public',type=Path,required=True);p.add_argument('--registration',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    a.root=a.root.resolve();a.worker=a.worker.resolve();a.fixture=a.fixture.resolve();a.public=a.public.resolve();a.output=a.output.resolve()
    registration=json.loads(a.registration.read_text());source=registration['source'];expected=registration['worker_sha256'];cpu=registration['cpu']
    assert registration['processes']==3 and registration['repetitions_per_process']==2
    assert cpu in os.sched_getaffinity(0) and sha(a.worker)==expected
    reference=json.loads((a.public/'status.json').read_text());reference_metadata=json.loads((a.public/'metadata.json').read_text())
    assert reference['source_custody_verified'] and reference['conformance_success'] and reference['worker_success']
    assert reference['git_sha']==reference['final_git_sha']==source and reference_metadata['worker_sha256']==expected
    for name in ['metadata.json','conformance.stdout.log','conformance.stderr.log']:
        assert sha(a.public/name)==reference['artifact_sha256'][name]
    # Reuse only an independently replayed original supported command. The quiet
    # gate starts afterwards, so verification work is outside all timed samples.
    assert not reference.get('conformance_reused_from_exact_source', False)
    verifier_path=Path(__file__).with_name('verify-compact-local-public.py')
    spec=importlib.util.spec_from_file_location('public_verify',verifier_path)
    verifier=importlib.util.module_from_spec(spec);spec.loader.exec_module(verifier)
    verified_reference=verifier.verify(a.public,source,a.worker)
    a.output.mkdir(parents=True,exist_ok=False)
    report={'schema':'conduit.fargan-quiet-repeat-family@1','controller_sha256':sha(Path(__file__)),'registration_sha256':sha(a.registration),'source':source,'worker_sha256':expected,'processes':[],'timing_excluded':False,'causes':[]}
    def save(): write(a.output/'family.json',report)
    report['reference_verifier_sha256']=sha(verifier_path)
    report['verified_reference']=verified_reference
    save()
    with (a.output/'monitor.jsonl').open('x') as monitor:
        previous=None
        def sample(stage,index):
            nonlocal previous
            now=time.monotonic_ns();rows=inventory();bad=heavy(rows)
            if previous is not None and now-previous>2_000_000_000:
                report['timing_excluded']=True;report['causes'].append({'cause':'monitor-gap','ns':now-previous})
            previous=now
            monitor.write(json.dumps({'monotonic_ns':now,'stage':stage,'index':index,'processes':rows,'competing':bad})+'\n');monitor.flush()
            return bad
        for index in range(1,4):
            assert git(a.root,'rev-parse','HEAD')==source and not git(a.root,'status','--porcelain') and sha(a.worker)==expected
            if shutil.disk_usage(a.output).free<512*1024**2:
                report['timing_excluded']=True;report['causes'].append({'cause':'output-space-preflight','index':index});save();return 2
            # A quiet gate precedes every process. No busy sample is selected away.
            for tick in range(13):
                bad=sample('preflight',index)
                if bad:
                    report['timing_excluded']=True;report['causes'].append({'cause':'busy-preflight','index':index,'processes':bad});save();return 2
                if tick<12: time.sleep(.5)
            capture=a.output/f'capture-{index}';capture.mkdir()
            for name in ['conformance.stdout.log','conformance.stderr.log']:
                shutil.copy2(a.public/name,capture/name)
            shutil.copy2(a.public/'status.json',capture/'conformance-reference-status.json')
            shutil.copy2(a.public/'metadata.json',capture/'conformance-reference-metadata.json')
            with (capture/'stdout.log').open('xb') as stdout,(capture/'stderr.log').open('xb') as stderr:
                command=['taskset','-c',str(cpu),str(a.worker),str(a.fixture),str(capture),'2','prepared-compact-local']
                process=subprocess.Popen(command,cwd=a.root,stdout=stdout,stderr=stderr)
                while process.poll() is None:
                    bad=sample('worker',index)
                    if bad:
                        report['timing_excluded']=True;report['causes'].append({'cause':'competing-process','index':index,'processes':bad})
                    time.sleep(.5)
                code=process.wait()
            bad=sample('worker-end',index)
            if bad:
                report['timing_excluded']=True;report['causes'].append({'cause':'competing-at-worker-end','index':index,'processes':bad})
            custody=git(a.root,'rev-parse','HEAD')==source and not git(a.root,'status','--porcelain') and sha(a.worker)==expected
            metadata=json.loads((capture/'metadata.json').read_text()) if (capture/'metadata.json').exists() else {}
            affinity=metadata.get('affinity','').split(':')[-1].strip()==str(cpu)
            if code!=0 or not custody or not affinity:
                report['timing_excluded']=True;report['causes'].append({'cause':'worker-or-custody','index':index,'code':code,'custody':custody,'affinity':affinity})
            manifest={str(f.relative_to(capture)):sha(f) for f in capture.rglob('*') if f.is_file()}
            write(capture/'status.json',{'schema':'conduit.fargan-frozen-repeat-command@1','git_sha':source,'final_git_sha':git(a.root,'rev-parse','HEAD'),'source_custody_verified':custody,'conformance_success':True,'conformance_receipt_sha256':sha(a.public/'status.json'),'conformance_reused_from_exact_source':True,'worker_success':code==0,'worker_exit_code':code,'repetitions':2,'owners':'prepared-compact-local','artifact_sha256':manifest})
            report['processes'].append({'index':index,'pid':process.pid,'exit_code':code,'source_custody':custody,'affinity_verified':affinity,'capture':capture.name,'status_sha256':sha(capture/'status.json')});save()
            print(f'completed process {index}/3, exit={code}, timing_excluded={report["timing_excluded"]}',flush=True)
    report['monitor_sha256']=sha(a.output/'monitor.jsonl')
    report['complete']=len(report['processes'])==3
    report['scope']='Raw matched observations only; independent replay and preregistered analysis still required. No automatic speedup, realtime or release claim.'
    save();return 0 if report['complete'] and not report['timing_excluded'] else 2
if __name__=='__main__': raise SystemExit(main())
