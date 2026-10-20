#!/usr/bin/env python3
"""Verify supported-command custody and replay; does not certify quiet timing."""
import argparse, hashlib, importlib.util, json, re
from pathlib import Path

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def verify_conformance_receipt(root,status,source,worker):
    assert status['schema']=='conduit.fargan-runtime-performance-command@1'
    assert not status.get('conformance_reused_from_exact_source',False)
    assert 'conformance_receipt_sha256' not in status
    return 'canonical-command'

def verify(root,source,worker):
    sibling=Path(__file__).with_name('verify-source-cache-capture.py')
    spec=importlib.util.spec_from_file_location('compact_replay',sibling);replay=importlib.util.module_from_spec(spec);spec.loader.exec_module(replay)
    status=json.loads((root/'status.json').read_text());metadata=json.loads((root/'metadata.json').read_text())
    assert status['git_sha']==status['final_git_sha']==source
    assert status['source_custody_verified'] and status['conformance_success'] and status['worker_success'] and status['worker_exit_code']==0
    assert status['owners']=='prepared-matrix-source-cache'
    assert metadata['git_sha']==source and metadata['source_status']=='clean' and metadata['profile']=='release' and not metadata['debug_assertions']
    assert metadata['owners']==status['owners'] and metadata['worker_sha256']==sha(worker)
    assert metadata['binary_bytes']==worker.stat().st_size
    for name,digest in status['artifact_sha256'].items():
        relative=Path(name);assert not relative.is_absolute() and '..' not in relative.parts
        assert sha(root/relative)==digest,name
    assert {str(p.relative_to(root)) for p in root.rglob('*') if p.is_file()}==set(status['artifact_sha256'])|{'status.json'}
    receipt_kind=verify_conformance_receipt(root,status,source,worker)
    proof=(root/'conformance.stdout.log').read_text()
    counts=re.findall(r'test result: ok\. (\d+) passed; 0 failed;',proof);assert len(counts)==2 and all(int(n)>0 for n in counts)
    assert 'matrix_local_profile_preserves_source_outputs_and_both_evidence_boundaries ... ok' in proof
    assert 'full_frame_check_and_expansion_reuse_match_cold_plans_and_execution ... ok' in proof
    summary=json.loads((root/'summary.json').read_text());assert summary['source_sha256']==sha(root/'source.conduit') and summary['repetitions']==status['repetitions']
    result=replay.verify(root);result.update({'classification':'source-bound supported release command and independent conformance replay; no quiet timing, throughput, realtime or release acceptance claim','source':source,'worker_sha256':sha(worker),'status_sha256':sha(root/'status.json'),'canonical_conformance_counts':[int(n) for n in counts],'conformance_receipt_kind':receipt_kind})
    return result
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('capture',type=Path);p.add_argument('--source',required=True);p.add_argument('--worker',type=Path,required=True);p.add_argument('--output',type=Path);a=p.parse_args();text=json.dumps(verify(a.capture,a.source,a.worker),indent=2)+'\n'
    if a.output:
        with a.output.open('x') as f:f.write(text)
    else:print(text,end='')
