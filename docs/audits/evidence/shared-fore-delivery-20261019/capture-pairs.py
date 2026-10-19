#!/usr/bin/env python3
"""Run frozen release workers in three alternating fresh-process pairs."""
import hashlib,json,os,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parent
SOURCES={'before':('conduit-flow-current-reference','99dfe6eceeb9f8e318e388a06beb98382746c647'),
         'after':('conduit','f2bb3b0b9640c8a9f2ccd2626af817538706c7e5')}
def digest(path):
    h=hashlib.sha256()
    with path.open('rb') as f:
        for b in iter(lambda:f.read(1024*1024),b''): h.update(b)
    return h.hexdigest()
def git(cwd,*args):
    return subprocess.check_output(['git',*args],cwd=cwd,text=True).strip()
def write(path,x):
    with path.open('x') as f: json.dump(x,f,indent=2); f.write('\n')
def main():
    assert 4 in os.sched_getaffinity(0),'CPU4 unavailable'
    for side,(directory,sha) in SOURCES.items():
        manifest=json.loads((ROOT/f'shared-fore-{side}-worker.json').read_text())
        assert manifest['source']==sha
        assert digest(Path(manifest['path']))==manifest['sha256']
        assert git(ROOT/directory,'rev-parse','HEAD')==sha
        assert not git(ROOT/directory,'status','--porcelain')
    for pair in range(1,4):
        for side in (['before','after'] if pair%2 else ['after','before']):
            directory,sha=SOURCES[side]; cwd=ROOT/directory
            manifest=json.loads((ROOT/f'shared-fore-{side}-worker.json').read_text())
            binary=Path(manifest['path']); output=ROOT/f'shared-fore-pair-{pair}-{side}'
            output.mkdir()
            command=['taskset','-c','4',str(binary),'--output',str(output),'--values','1000000',
                     '--repetitions','3','--decisions','1000','--samples','1000','--hosted-pipeline','true']
            assert git(cwd,'rev-parse','HEAD')==sha and not git(cwd,'status','--porcelain')
            assert digest(binary)==manifest['sha256']
            started=time.time_ns()
            print(f'pair {pair} {side}: starting',flush=True)
            with (output/'stdout.log').open('xb') as out,(output/'stderr.log').open('xb') as err:
                result=subprocess.run(command,cwd=cwd,env={**os.environ,'RUST_TEST_THREADS':'1'},stdout=out,stderr=err)
            custody=git(cwd,'rev-parse','HEAD')==sha and not git(cwd,'status','--porcelain')
            metadata=json.loads((output/'metadata.json').read_text())
            receipt={'schema':'conduit.shared-fore-frozen-worker-capture@1','pair':pair,'side':side,
                'source':sha,'source_custody_verified':custody,'exit_code':result.returncode,
                'worker_sha256':digest(binary),'command':command,'cwd':str(cwd),
                'started_unix_ns':started,'finished_unix_ns':time.time_ns(),
                'artifact_sha256':{str(p.relative_to(output)):digest(p) for p in sorted(output.rglob('*')) if p.is_file()},
                'scope':'fresh process, CPU4; full original worker matrix; descriptive shared-host timings, no hardware-counter traffic claim'}
            write(output/'capture-status.json',receipt)
            assert result.returncode==0 and custody and receipt['worker_sha256']==manifest['sha256']
            assert metadata['git_sha']==sha and metadata['source_status']=='clean'
            assert metadata['affinity'].split(':',1)[1].strip()=='4'
            print(f'pair {pair} {side}: passed and retained',flush=True)
if __name__=='__main__': main()
