#!/usr/bin/env python3
"""Refuse internally rehashed corruptions of a real expanded runtime capture."""
import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', type=Path, required=True)
    parser.add_argument('--verifier', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert __debug__, 'do not disable proof assertions'
    assert not args.output.resolve().is_relative_to(args.capture.resolve())
    original_rows = [json.loads(line) for line in (args.capture/'raw.jsonl').read_text().splitlines()]
    original_report = json.loads((args.capture/'report.json').read_text())
    original_status = json.loads((args.capture/'status.json').read_text())
    checks=[]
    with tempfile.TemporaryDirectory(prefix='runtime-stream-negative-',dir=args.output.parent) as scratch:
        root=Path(scratch);copy=root/'capture';shutil.copytree(args.capture,copy)
        variants=['payload','sequence','pressure-count','pressure-witness','boot','terminal','latency','matrix']
        for name in variants:
            rows=json.loads(json.dumps(original_rows));report=json.loads(json.dumps(original_report));status=json.loads(json.dumps(original_status))
            r=next(r for r in rows if r.get('phase')=='bounded-ordinary-stream' and r['configuration']['queue']==1 and r['configuration']['batch']==8)
            expected=''
            if name=='payload':
                item=r['output_observations']['output_receipts'][0];item['sha256']=('1' if item['sha256'][0]=='0' else '0')+item['sha256'][1:];expected='stream output payload differs'
            elif name=='sequence':
                r['output_observations']['output_receipts'][0]['sequence']+=1;expected='stream output order/size differs'
            elif name=='pressure-count':
                r['ingress']['attempts']+=1
            elif name=='pressure-witness':
                r['ingress']['attempts']=r['input_items'];r['ingress']['full']=0;expected='stream pressure not witnessed'
            elif name=='boot':
                r['active_play']['boot_id']+='-other';expected='stream Host/Boot custody'
            elif name=='terminal':
                r['observations'][-1]['kind']['PlanTerminal']['disposition']='Failed';expected='stream terminal differs'
            elif name=='latency':
                r['output_observations']['inter_acknowledgement_distribution']['p95_ns']+=1;expected='stream latency percentile differs'
            elif name=='matrix':
                rows=[r for r in rows if r.get('phase')!='bounded-ordinary-stream'];report['runs']=[r for r in report['runs'] if r.get('mode')!='bounded-stream'];report['repeated_batches']=[r for r in report['repeated_batches'] if r['mode']!='bounded-stream'];report['raw_records']=len(rows);expected='stream matrix incomplete or repeated'
            (copy/'raw.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in rows));(copy/'report.json').write_text(json.dumps(report))
            for file in ['raw.jsonl','report.json']:status['artifact_sha256'][file]=hashlib.sha256((copy/file).read_bytes()).hexdigest()
            (copy/'status.json').write_text(json.dumps(status))
            output=root/(name+'.json')
            result=subprocess.run([sys.executable,str(args.verifier),'--capture',str(copy),'--output',str(output)],capture_output=True,text=True)
            assert result.returncode!=0 and not output.exists(),(name,result.stdout,result.stderr)
            if expected:assert expected in result.stderr,(name,result.stderr)
            checks.append({'mutation':name,'refused':True,'expected_reason':expected or 'ingress attempt accounting differs'})
    receipt={'source':original_status['git_sha'],'checks':checks,'capture_status_sha256':hashlib.sha256((args.capture/'status.json').read_bytes()).hexdigest(),'verifier_sha256':hashlib.sha256(args.verifier.read_bytes()).hexdigest(),'stream_verifier_sha256':hashlib.sha256(args.verifier.with_name('verify_runtime_stream.py').read_bytes()).hexdigest(),'proof_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scope':'Independent verifier against rehashed copied artifacts; original capture remains unchanged.'}
    with args.output.open('x') as f:json.dump(receipt,f,indent=2);f.write('\n')
    print(args.output)


if __name__=='__main__':main()
