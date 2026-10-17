#!/usr/bin/env python3
"""Reject rehashed corruptions of actual domain and temporal-matrix receipts."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture',type=Path,required=True)
    parser.add_argument('--verifier',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();assert __debug__
    assert not args.output.resolve().is_relative_to(args.capture.resolve())
    original_rows=[json.loads(line) for line in (args.capture/'raw.jsonl').read_text().splitlines()]
    original_report=json.loads((args.capture/'report.json').read_text());original_status=json.loads((args.capture/'status.json').read_text())
    checks=[]
    with tempfile.TemporaryDirectory(prefix='domain-custody-',dir=args.output.parent) as scratch:
        root=Path(scratch);copy=root/'capture';shutil.copytree(args.capture,copy)
        for name in ['control-output','control-input','numerical-source','numerical-error','numerical-type','value-play','value-implementation','value-eligibility','domain-matrix','value-matrix']:
            rows=json.loads(json.dumps(original_rows));report=json.loads(json.dumps(original_report));status=json.loads(json.dumps(original_status))
            extra={};expected=''
            control=next(r for r in rows if r.get('phase')=='ordinary-domain-control')
            number=next(r for r in rows if r.get('phase')=='ordinary-numerical-control')
            matrix=next(r for r in rows if r.get('phase')=='ordinary-value-matrix' and r['mode']=='value-scalar-specialized' and r['gears']==8)
            if name=='control-output':
                output=control['output_observations']['outputs'][0];output['canonical'][-1]^=1;output['sha256']=hashlib.sha256(bytes(output['canonical'])).hexdigest();expected='control Frequency differs'
            elif name=='control-input':
                file='sources/theremin-pitch-control-inputs.json';data=json.loads((args.capture/file).read_text());data['inputs'][0][-1]^=1;extra[file]=json.dumps(data);expected='control input differs'
            elif name in ['numerical-source','numerical-error','numerical-type']:
                output=next(v for v in number['output_observations']['outputs'] if v['port']=='loose');frame=bytearray(output['canonical'])
                if name=='numerical-type':
                    frame[5]^=1;expected='numerical output Type differs'
                else:
                    field=b'source' if name=='numerical-source' else b'maximum-error';position=frame.rfind(field)+len(field);assert frame[position]==0;payload=position+5;assert frame[payload:payload+3]==bytes([1,0,1]);frame[payload+8]=7 if name=='numerical-source' else 3;expected='numerical source differs' if name=='numerical-source' else 'numerical error differs'
                output['canonical']=list(frame);output['sha256']=hashlib.sha256(frame).hexdigest()
            elif name=='value-play':
                matrix['plays'][1]['active_play']['active_play_id']=matrix['plays'][0]['active_play']['active_play_id']
            elif name=='value-implementation':
                file=f"sources/{matrix['case']}-{matrix['repetition']}-plan.json";plan=json.loads((args.capture/file).read_text());plan['fragments'][0]['placements'][0]['implementation_id']='foreign/scalar-tape';extra[file]=json.dumps(plan)
            elif name=='value-eligibility':
                matrix['eligibility']['flow_specialization']['eligible']=True
            else:
                modes={'canonical-theremin-control','certified-numerical-projections'} if name=='domain-matrix' else {'value-reference','value-scalar-specialized'}
                rows=[r for r in rows if r.get('mode') not in modes];report['runs']=[r for r in report['runs'] if r.get('mode') not in modes];report['repeated_batches']=[r for r in report['repeated_batches'] if r['mode'] not in modes];report['raw_records']=len(rows);expected='domain matrix incomplete' if name=='domain-matrix' else 'Value matrix incomplete'
            extra['raw.jsonl']=''.join(json.dumps(r)+'\n' for r in rows);extra['report.json']=json.dumps(report)
            # Restore the only source artifacts mutated by a preceding variant.
            for file in ['sources/theremin-pitch-control-inputs.json',f"sources/{matrix['case']}-{matrix['repetition']}-plan.json"]:
                if file not in extra:shutil.copy2(args.capture/file,copy/file)
            for file,data in extra.items():(copy/file).write_text(data);status['artifact_sha256'][file]=hashlib.sha256((copy/file).read_bytes()).hexdigest()
            (copy/'status.json').write_text(json.dumps(status));output=root/(name+'.json')
            result=subprocess.run([sys.executable,str(args.verifier),'--capture',str(copy),'--output',str(output)],capture_output=True,text=True)
            assert result.returncode!=0 and not output.exists(),(name,result.stdout,result.stderr)
            if expected:assert expected in result.stderr,(name,result.stderr)
            checks.append({'mutation':name,'refused':True,'expected_reason':expected or 'identity, implementation or eligibility invariant'})
    receipt={'source':original_status['git_sha'],'checks':checks,'capture_status_sha256':hashlib.sha256((args.capture/'status.json').read_bytes()).hexdigest(),'verifier_sha256':hashlib.sha256(args.verifier.read_bytes()).hexdigest(),'domain_verifier_sha256':hashlib.sha256(args.verifier.with_name('verify_runtime_domain.py').read_bytes()).hexdigest(),'proof_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scope':'Rehashed copies only; original immutable data unchanged.'}
    with args.output.open('x') as f:json.dump(receipt,f,indent=2);f.write('\n')
    print(args.output)


if __name__=='__main__':main()
