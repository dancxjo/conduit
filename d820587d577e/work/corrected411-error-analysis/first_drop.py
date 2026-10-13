"""Untrusted external width-four arithmetic diagnostic, never runtime admission."""
import pathlib,json,copy,itertools,time,hashlib
import sys,struct
base=pathlib.Path('semantics/language/training/ewt_proposal_v2/origin-corrected-v2')
sys.path.insert(0,str(base));import reference_v2_corrected as p
raw=(base/'proposal_window8.i16').read_bytes();assert struct.unpack('<3I',raw[8:20])==(411,76,27)
weights=struct.unpack('<'+str(411*76)+'h',raw[20:])
profile={e['surface']:e['pos_codes'] for f in sorted(pathlib.Path('/home/dancxjo/conduit-4907-evaluation-identity/work/lexical-proposer-next/train-dictionary-authored-overlay-v2').glob('shard-*.json'))for e in json.loads(f.read_text())['entries']}
rows=json.load(open('work/corrected411-runtime/diagnostic-rows.json'))
def scores(indices):return[sum(weights[c*411+i]for i in indices)for c in range(76)]
results=[]
for row in rows:
 started=time.monotonic();n=len(row['forms']);beam=[(0,p.legacy.initial(n),[16]*n,0,0,[])];calls=0;identity=4;first_drop=None;oracle=p.legacy.oracle(row)
 for epoch in range(1,33):
  proposals=[]
  for score,state,pos,selected_count,prior_identity,path in beam:
   if state['unread']==n and len(state['stack'])==1:
    proposals.append((score,state,pos,selected_count,prior_identity,path));continue
   current=state['unread']
   choose=current<n and selected_count==current
   for code in (profile[row['forms'][current]] if choose else [pos[current] if current<n else 17]):
    selected=pos.copy()
    if current<n:selected[current]=code
    ss=scores(p.features(state,selected,row,profile,[0]*n));calls+=1
    for action,delta in enumerate(ss):
     if not p.legacy.legal(state,action):continue
     new=copy.deepcopy(state);p.legacy.apply(new,action)
     proposals.append((score+delta,new,selected.copy(),selected_count+int(choose),identity,path+[action]));identity+=1
  # Original Source orders by score descending and identity ascending.
  proposals.sort(key=lambda b:(-b[0],b[4]));beam=proposals[:4]
  gold=[b for b in proposals if b[5]==oracle[:epoch] and b[2][:b[3]]==row['pos'][:b[3]]]
  if first_drop is None and not any(b in beam for b in gold):
   first_drop={'epoch':epoch,'oracle_action':oracle[epoch-1] if epoch<=len(oracle)else None,'gold_proposals':len(gold),'gold_score':gold[0][0]if gold else None,'cutoff_score':beam[-1][0]if beam else None,'best_action_prefix':beam[0][5]if beam else None}
  if not beam:break
  if all(s['unread']==n and len(s['stack'])==1 for _,s,_,_,_,_ in beam):break
 complete=[b for b in beam if b[1]['unread']==n and len(b[1]['stack'])==1]
 best=complete[0] if complete else (beam[0] if beam else None)
 result={'id':row['id'],'text':row.get('text'),'epochs':epoch,'first_gold_drop':first_drop,'final_gold_rank':next((i for i,b in enumerate(beam)if b[5]==oracle and b[2]==row['pos']),None),'final_scores':[b[0]for b in beam],'numeric_calls':calls,'complete':bool(complete),'elapsed_seconds':time.monotonic()-started}
 if best:
  _,state,pos,_,_,_=best;heads=state['heads'][:n];rels=state['relations'][:n]
  result.update(heads=heads,relations=rels,pos=pos,correct_heads=sum(a==b for a,b in zip(heads,row['heads'])),correct_base_las=sum(a==b and c.split(':')[0]==d.split(':')[0]for a,b,c,d in zip(heads,row['heads'],rels,row['relations'])),correct_pos=sum(a==b for a,b in zip(pos,row['pos'])),tokens=n)
 results.append(result)
out={'scope':'External numerical width-four diagnostic only; Source-shaped selected-frontier/score/identity policy; no checked Source equivalence. Not original Source/Native parity, admitted parser output, heldout accuracy or stability proof','artifact_sha256':hashlib.sha256(raw).hexdigest(),'rows':results}
pathlib.Path('work/corrected411-error-analysis/external-beam-first-drop.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({'rows':len(results),'complete':sum(r['complete']for r in results),'exact_graph_and_pos':sum(r.get('correct_base_las')==r.get('tokens')and r.get('correct_pos')==r.get('tokens')for r in results),'failures':[r['id']for r in results if r.get('correct_base_las')!=r.get('tokens')or r.get('correct_pos')!=r.get('tokens')]}))
