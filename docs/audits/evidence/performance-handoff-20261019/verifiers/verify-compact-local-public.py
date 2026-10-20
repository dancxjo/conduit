#!/usr/bin/env python3
"""Verify supported-command custody and replay; does not certify quiet timing."""
import argparse, hashlib, importlib.util, json, re
from pathlib import Path

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def verify_conformance_receipt(root, status, source, worker):
    """Bind reused proof to its original successful command and exact worker."""
    reused = status.get('conformance_reused_from_exact_source', False)
    if status.get('schema') == 'conduit.fargan-frozen-repeat-command@1':
        assert reused is True
    if not reused:
        assert 'conformance_receipt_sha256' not in status
        return 'canonical-command'
    assert status['schema'] == 'conduit.fargan-frozen-repeat-command@1'
    receipt_path = root/'conformance-reference-status.json'
    assert sha(receipt_path) == status['conformance_receipt_sha256']
    receipt = json.loads(receipt_path.read_text())
    original_metadata = root/'conformance-reference-metadata.json'
    metadata = json.loads(original_metadata.read_text())
    assert not receipt.get('conformance_reused_from_exact_source', False)
    assert receipt['git_sha'] == receipt['final_git_sha'] == source
    assert receipt['source_custody_verified'] is True
    assert receipt['conformance_success'] is True
    assert receipt['worker_success'] is True and receipt['worker_exit_code'] == 0
    assert receipt['owners'] == status['owners'] == 'prepared-compact-local'
    assert metadata['git_sha'] == source and metadata['source_status'] == 'clean'
    assert metadata['profile'] == 'release' and metadata['debug_assertions'] is False
    assert metadata['worker_sha256'] == sha(worker)
    assert metadata['binary_bytes'] == worker.stat().st_size
    assert metadata['owners'] == receipt['owners']
    assert sha(original_metadata) == receipt['artifact_sha256']['metadata.json']
    for name in ['conformance.stdout.log', 'conformance.stderr.log']:
        assert sha(root/name) == receipt['artifact_sha256'][name]
    return 'reused-exact-source-worker-command'

def verify(root,source,worker):
    sibling=Path(__file__).with_name('verify-compact-local-capture.py')
    spec=importlib.util.spec_from_file_location('compact_replay',sibling);replay=importlib.util.module_from_spec(spec);spec.loader.exec_module(replay)
    status=json.loads((root/'status.json').read_text());metadata=json.loads((root/'metadata.json').read_text())
    assert status['git_sha']==status['final_git_sha']==source
    assert status['source_custody_verified'] and status['conformance_success'] and status['worker_success'] and status['worker_exit_code']==0
    assert status['owners']=='prepared-compact-local'
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
    assert 'full_frame_local_expression_profile_preserves_source_and_all_outputs ... ok' in proof
    summary=json.loads((root/'summary.json').read_text());assert summary['source_sha256']==sha(root/'source.conduit') and summary['repetitions']==status['repetitions']
    result=replay.verify(root);result.update({'classification':'source-bound supported release command and independent conformance replay; no quiet timing, throughput, realtime or release acceptance claim','source':source,'worker_sha256':sha(worker),'status_sha256':sha(root/'status.json'),'canonical_conformance_counts':[int(n) for n in counts],'conformance_receipt_kind':receipt_kind})
    return result
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('capture',type=Path);p.add_argument('--source',required=True);p.add_argument('--worker',type=Path,required=True);p.add_argument('--output',type=Path);a=p.parse_args();text=json.dumps(verify(a.capture,a.source,a.worker),indent=2)+'\n'
    if a.output:
        with a.output.open('x') as f:f.write(text)
    else:print(text,end='')
