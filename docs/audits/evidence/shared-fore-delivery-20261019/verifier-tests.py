#!/usr/bin/env python3
"""Exercise independent semantic rejection after deliberately bypassing hashes."""
import argparse,copy,importlib.util,json
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('before',type=Path);p.add_argument('after',type=Path);p.add_argument('--verifier',type=Path,default=Path(__file__).with_name('verify.py'));args=p.parse_args()
spec=importlib.util.spec_from_file_location('fore_verify',args.verifier);v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
b,bm,br=v.load(args.before.resolve(),'before');a,am,ar=v.load(args.after.resolve(),'after');original=v.load;refusals=[]
try:
 for field in v.FIELDS:
  key=next(iter(a));bad=copy.deepcopy(a[key]);value=bad[field]
  if isinstance(value,int):bad[field]+=1
  elif isinstance(value,str):bad[field]+='tampered'
  elif isinstance(value,list):bad[field]=value+['tampered']
  else:raise AssertionError(field)
  changed=dict(a);changed[key]=bad
  v.load=lambda path,side:(b,bm,br) if side=='before' else (changed,am,ar)
  try:v.compare(args.before.resolve(),args.after.resolve())
  except AssertionError:refusals.append(field)
  else:raise AssertionError('accepted '+field)
finally:v.load=original
original_digest=v.digest
try:
 v.digest=lambda path:'0'*64 if path.name=='raw.jsonl' else original_digest(path)
 try:v.load(args.before.resolve(),'before')
 except AssertionError:hash_refused=True
 else:raise AssertionError('accepted wrong raw digest')
finally:v.digest=original_digest
print(json.dumps({'verifier_sha256':v.digest(args.verifier),'semantic_mutation_refusals':refusals,'count':len(refusals),'raw_checksum_mismatch_refused':hash_refused,'scope':'Semantic mutation comparison deliberately bypasses envelope validation so each field must refuse independently; separate wrong-digest test verifies the raw artifact check. No capture files were modified.'},indent=2))
