import json, pathlib, struct, hashlib
class Reader:
 def __init__(s,b):s.b=b;s.i=0
 def n(s,n):v=s.b[s.i:s.i+n];s.i+=n;return v
 def u(s,n):return int.from_bytes(s.n(n),'little')
 def txt(s):return s.n(s.u(4)).decode()
 def ty(s):
  t=s.u(1)
  if t==0:return ('leaf',s.txt())
  if t==5:s.txt();return s.ty()
  if t==1:s.u(2);return ('seq',s.ty())
  if t==4:s.u(2);s.u(2);return ('seq',s.ty())
  if t in (2,3):
   s.txt();return ('record' if t==2 else 'variant',{s.txt():s.ty() for _ in range(s.u(4))})
  raise ValueError(t)
 def val(s,ty):
  t=s.u(1)
  if t==0:
   b=s.n(s.u(4));k=ty[1]
   if k in ('value/u64','value/u32','value/i64'):return int.from_bytes(b,'little',signed=k=='value/i64')
   if k=='value/bool':return bool(int.from_bytes(b,'little'))
   return b.decode(errors='replace')
  if t==1:return [s.val(ty[1]) for _ in range(s.u(4))]
  if t==2:
   d={}
   for _ in range(s.u(4)):
    k=s.txt();d[k]=s.val(ty[1][k])
   return d
  if t==3:
   k=s.txt();return {k:s.val(ty[1][k])}
 def decode(s):
  ty=s.ty();v=s.val(ty);assert s.i==len(s.b);return v
base=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907/outputs/native-admission-family/vocative-candidate-08425a-prepared-r2/authored_vocative/native_cached_clause_rows.json')
r=json.loads(base.read_text())[0]
dec=lambda x:Reader(bytes.fromhex(x)).decode()
source=dec(r['source_material_bytes']);tape=dec(r['lexical_tape_bytes'])
result={'row_id':r['id'],'source':source,'tokens':tape['tokens'],'epochs':[]}
for e in r['epochs']:
 beam=dec(e['beam_bytes']); result['epochs'].append({'epoch':e['epoch'],'candidates':[{k:v for k,v in beam[f'candidate{i}'].items() if k!='state'} for i in range(4)]})
out=pathlib.Path('work/window8-first-row-stability-decoded.json');out.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'source':source,'tokens':tape['tokens'],'last_epoch':result['epochs'][-1]},indent=2))
