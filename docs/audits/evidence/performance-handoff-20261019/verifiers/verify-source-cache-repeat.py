#!/usr/bin/env python3
"""Frozen repeats require the pinned original successful command receipt."""
import argparse,hashlib,importlib.util,json,re
from pathlib import Path

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def verify(root,registration,worker):
    reg=json.loads(registration.read_text());source=reg['source'];expected=reg['worker_sha256']
    assert sha(worker)==expected and worker.stat().st_size==reg['worker_bytes']
    status=json.loads((root/'status.json').read_text());metadata=json.loads((root/'metadata.json').read_text())
    assert status['schema']=='conduit.fargan-frozen-source-cache-repeat-command@1'
    assert status['git_sha']==status['final_git_sha']==source and status['source_custody_verified'] is True
    assert status['conformance_success'] is True and status['worker_success'] is True and status['worker_exit_code']==0
    assert status['owners']==metadata['owners']=='prepared-matrix-source-cache' and status['repetitions']==reg['repetitions_per_process']==2
    assert metadata['git_sha']==source and metadata['source_status']=='clean' and metadata['profile']=='release' and metadata['debug_assertions'] is False
    assert metadata['worker_sha256']==expected and metadata['binary_bytes']==reg['worker_bytes']
    assert metadata['affinity'].split(':')[-1].strip()==str(reg['cpu'])
    assert status['conformance_reused_from_exact_source'] is True
    receipt_path=root/'conformance-reference-status.json';assert sha(receipt_path)==status['conformance_receipt_sha256']==reg['original_command_receipt_sha256']
    receipt=json.loads(receipt_path.read_text());original_metadata=root/'conformance-reference-metadata.json';om=json.loads(original_metadata.read_text())
    assert receipt['schema']=='conduit.fargan-runtime-performance-command@1' and not receipt.get('conformance_reused_from_exact_source',False)
    assert receipt['git_sha']==receipt['final_git_sha']==source and receipt['source_custody_verified'] is True and receipt['conformance_success'] is True
    assert receipt['worker_success'] is True and receipt['worker_exit_code']==0 and receipt['owners']=='prepared-matrix-source-cache'
    assert om['git_sha']==source and om['source_status']=='clean' and om['worker_sha256']==expected and om['binary_bytes']==reg['worker_bytes']
    assert om['profile']=='release' and om['debug_assertions'] is False and om['owners']==receipt['owners']
    assert sha(original_metadata)==receipt['artifact_sha256']['metadata.json']
    for name in ['conformance.stdout.log','conformance.stderr.log']:assert sha(root/name)==receipt['artifact_sha256'][name]
    for name,digest in status['artifact_sha256'].items():
        relative=Path(name);assert not relative.is_absolute() and '..' not in relative.parts
        assert sha(root/relative)==digest,name
    assert {str(p.relative_to(root)) for p in root.rglob('*') if p.is_file()}==set(status['artifact_sha256'])|{'status.json'}
    proof=(root/'conformance.stdout.log').read_text();counts=re.findall(r'test result: ok\. (\d+) passed; 0 failed;',proof)
    assert len(counts)==2 and all(int(c)>0 for c in counts)
    assert 'full_frame_check_and_expansion_reuse_match_cold_plans_and_execution ... ok' in proof
    sibling=Path(__file__).with_name('verify-source-cache-capture.py');spec=importlib.util.spec_from_file_location('replay',sibling);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
    summary=json.loads((root/'summary.json').read_text());assert summary['source_sha256']==sha(root/'source.conduit') and summary['repetitions']==2
    result=m.verify(root);result.update({'classification':'independent frozen-repeat conformance, exact pinned original command and worker; quiet timing requires separate family monitor acceptance','source':source,'worker_sha256':expected,'original_receipt_sha256':sha(receipt_path),'status_sha256':sha(root/'status.json'),'canonical_conformance_counts':[int(c) for c in counts]});return result
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('capture',type=Path);p.add_argument('--registration',type=Path,required=True);p.add_argument('--worker',type=Path,required=True);p.add_argument('--output',type=Path);a=p.parse_args();result=json.dumps(verify(a.capture,a.registration,a.worker),indent=2)+'\n'
    if a.output:
        with a.output.open('x') as f:f.write(result)
    else:print(result,end='')
