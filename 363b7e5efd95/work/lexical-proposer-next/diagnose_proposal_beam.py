"""Untrusted external width-four arithmetic diagnostic, never runtime admission."""
import pathlib,json,copy,itertools,time,hashlib
import diagnose_proposal_candidate as diagnostic
p=diagnostic.p;profile=diagnostic.profile
results=[]
for row in diagnostic.rows:
 started=time.monotonic();n=len(row['forms']);beam=[(0,p.legacy.initial(n),[16]*n,0,0)];calls=0;identity=4
 for epoch in range(1,33):
  proposals=[]
  for score,state,pos,selected_count,prior_identity in beam:
   if state['unread']==n and len(state['stack'])==1:
    proposals.append((score,state,pos,selected_count,prior_identity));continue
   current=state['unread']
   choose=current<n and selected_count==current
   for code in (profile[row['forms'][current]] if choose else [pos[current] if current<n else 17]):
    selected=pos.copy()
    if current<n:selected[current]=code
    ss=diagnostic.scores(p.features(state,selected,row,profile,[0]*n));calls+=1
    for action,delta in enumerate(ss):
     if not p.legacy.legal(state,action):continue
     new=copy.deepcopy(state);p.legacy.apply(new,action)
     proposals.append((score+delta,new,selected.copy(),selected_count+int(choose),identity));identity+=1
  # Original Source orders by score descending and identity ascending.
  proposals.sort(key=lambda b:(-b[0],b[4]));beam=proposals[:4]
  if not beam:break
  if all(s['unread']==n and len(s['stack'])==1 for _,s,_,_,_ in beam):break
 complete=[b for b in beam if b[1]['unread']==n and len(b[1]['stack'])==1]
 best=complete[0] if complete else (beam[0] if beam else None)
 result={'id':row['id'],'text':row.get('text'),'epochs':epoch,'numeric_calls':calls,'complete':bool(complete),'elapsed_seconds':time.monotonic()-started}
 if best:
  _,state,pos,_,_=best;heads=state['heads'][:n];rels=state['relations'][:n]
  result.update(heads=heads,relations=rels,pos=pos,correct_heads=sum(a==b for a,b in zip(heads,row['heads'])),correct_base_las=sum(a==b and c.split(':')[0]==d.split(':')[0]for a,b,c,d in zip(heads,row['heads'],rels,row['relations'])),correct_pos=sum(a==b for a,b in zip(pos,row['pos'])),tokens=n)
 results.append(result)
out={'scope':'External numerical width-four diagnostic only; Source-shaped selected-frontier/score/identity policy; no checked Source equivalence. Not original Source/Native parity, admitted parser output, heldout accuracy or stability proof','artifact_sha256':diagnostic.manifest['artifact_sha256'],'rows':results}
(diagnostic.base/'proposal-446-training-v2/external-beam-diagnostic.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({'rows':len(results),'complete':sum(r['complete']for r in results),'exact_graph_and_pos':sum(r.get('correct_base_las')==r.get('tokens')and r.get('correct_pos')==r.get('tokens')for r in results),'failures':[r['id']for r in results if r.get('correct_base_las')!=r.get('tokens')or r.get('correct_pos')!=r.get('tokens')]}))
