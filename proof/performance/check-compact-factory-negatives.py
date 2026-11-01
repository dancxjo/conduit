from pathlib import Path
import importlib.util,json,tempfile,re
import argparse
if not __debug__:
    raise RuntimeError('factory log replay requires assertions; optimized Python is refused')
p=argparse.ArgumentParser();p.add_argument('log',type=Path);p.add_argument('--source',required=True);p.add_argument('--worker',type=Path,required=True);p.add_argument('--output',type=Path);args=p.parse_args()
script=Path(__file__).with_name('verify-compact-linear-factory-controls.py');spec=importlib.util.spec_from_file_location('factory',script);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
log=args.log.resolve();worker=args.worker;source=args.source;positive=m.verify(log,worker,source)
lines=log.read_text().splitlines();indices=[i for i,s in enumerate(lines)if 'COMPACT_LINEAR_CONTROL ' in s]
parsed=[json.loads(lines[i].split('COMPACT_LINEAR_CONTROL ',1)[1])for i in indices]
def change(rows,kind):
 normal=next(r for r in rows if r['fault']=='None' and r['mode']==2);fault=next(r for r in rows if r['fault']=='Overflow' and r['mode']==2)
 if kind=='allocation':normal['play_allocations']=1
 elif kind=='wrong-output-bit':normal['output_bits'][0]^=1
 elif kind=='wrong-byte-count':normal['raw_bytes']+=4
 elif kind=='fault-publishes-output':fault['output_bits']=[0,0,0,0]
 elif kind=='wrong-role':normal['roles'][0][0]='invented-role'
 elif kind=='missing-mode':rows.pop()
 elif kind=='duplicate-mode':rows.append(rows[0].copy())
 elif kind=='wrong-event-sequence':normal['events'][0]['sequence']=17
 elif kind=='wrong-failure-detail':fault['failure']='BackFailed detail: 0'
results=[]
for kind in ['allocation','wrong-output-bit','wrong-byte-count','fault-publishes-output','wrong-role','missing-mode','duplicate-mode','wrong-event-sequence','wrong-failure-detail']:
 rows=json.loads(json.dumps(parsed));change(rows,kind)
 # Keep ordinary test transcript, then append only the mutated control records.
 text='\n'.join(s for s in lines if 'COMPACT_LINEAR_CONTROL ' not in s)+'\n'+''.join('COMPACT_LINEAR_CONTROL '+json.dumps(row)+'\n'for row in rows)
 with tempfile.TemporaryDirectory(prefix='factory-refusal-',dir=log.parent)as d:
  p=Path(d)/'log';p.write_text(text)
  try:m.verify(p,worker,source)
  except (AssertionError,KeyError,ValueError,FileNotFoundError)as e:results.append({'case':kind,'refused':True,'error_class':type(e).__name__})
  else:raise AssertionError(kind)
report={'positive':positive,'refusals':results,'limits':'Raw transcript semantic replay. Source/worker fields are caller provenance annotations; no log-to-binary authenticity, independent Source-to-Plan check, timing or release acceptance.'}
text=json.dumps(report,indent=2)+'\n'
if args.output:
 with args.output.open('x') as output: output.write(text)
else: print(text,end='')
