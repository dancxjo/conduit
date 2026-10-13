"""Diagnostic TRAIN+reviewed teaching oracle vectors; no model fitting or admission."""
import pathlib,json,hashlib,sys,itertools
sys.path.insert(0,str(pathlib.Path(__file__).parent))
import reference_v2_corrected as ref
old=pathlib.Path('/home/dancxjo/conduit-4907-evaluation-identity/work/lexical-proposer-next')
train=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907/work/window8/pinned-ewt/en_ewt-ud-train.conllu')
assert hashlib.sha256(train.read_bytes()).hexdigest()=='d68e06122a702464c613076523d56740f047e5bbe89dd90ec32737e04d952143'
profile={e['surface']:e['pos_codes'] for p in sorted((old/'train-dictionary-authored-overlay-v2').glob('shard-*.json')) for e in json.loads(p.read_text())['entries']}
rows,skips=ref.legacy.read(train)
rows=[r for r in rows if all(p in profile.get(f,[]) for f,p in zip(r['forms'],r['pos']))]
extra=json.loads((old/'authored-proposal-window8-teaching-v1.json').read_text())['rows']
for r in extra:r['oracle']=ref.legacy.oracle(r)
count=states=0
with pathlib.Path(__file__).with_name('vectors-full-context.jsonl').open('w') as out:
 for row in rows+extra:
  state=ref.legacy.initial(len(row['pos']))
  # Include post-final state: boundary/last-consumed origin is a distinct case.
  for target in row['oracle']+[None]:
   states+=1;n=state['n'];current=state['unread'];top=state['stack'][-1]
   positions=[i for i in (top,current) if i<n]
   variants=list(itertools.product(*(profile[row['forms'][i]] for i in positions))) or [()]
   for chosen in variants:
    pos=list(row['pos'])
    if any(i<current and pos[i]!=code for i,code in zip(positions,chosen)):continue
    for i,code in zip(positions,chosen):pos[i]=code
    for origin_policy in ('reviewed','unknown','alternating'):
     origins=[0 if origin_policy=='reviewed' else 1 if origin_policy=='unknown' else i%2 for i in range(n)]
     future=profile[row['forms'][current+1]] if current+1<n else []
     v={'id':row['id'],'state':state,'pos':pos,'future':future,'profiles':[profile[f] for f in row['forms']],'origins':origins,'expected':ref.features(state,pos,row,profile,origins),'origin_policy':origin_policy}
     out.write(json.dumps(v,separators=(',',':'))+'\n');count+=1
   if target is not None:ref.legacy.apply(state,target)
receipt={'scope':'All covered TRAIN and22reviewed teaching oracle states including terminal; all permitted top/current candidate variants preserving selected history; synthetic unknown/alternating origins are arithmetic parity only, not lexical admission','train_rows':len(rows),'teaching_rows':len(extra),'states':states,'vectors':count,'train_sha256':hashlib.sha256(train.read_bytes()).hexdigest(),'vectors_sha256':hashlib.sha256(pathlib.Path(__file__).with_name('vectors-full-context.jsonl').read_bytes()).hexdigest()}
pathlib.Path(__file__).with_name('export-full-context-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(receipt)
