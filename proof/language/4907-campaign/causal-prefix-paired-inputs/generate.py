#!/usr/bin/env python3
"""Offline replay REQUESTS and expected features; never Source admission/model input approval."""
import argparse, ast, collections, hashlib, itertools, json, pathlib
P=pathlib.Path;OUT=P(__file__).resolve().parent
POS='ADJ ADP ADV AUX CCONJ DET INTJ NOUN NUM PART PRON PROPN PUNCT SCONJ SYM VERB X'.split()
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--corpus',type=P,required=True);ap.add_argument('--training',type=P,required=True);a=ap.parse_args()
 pins={'raw_train':(a.corpus/'en_ewt-ud-train.conllu','d68e06122a702464c613076523d56740f047e5bbe89dd90ec32737e04d952143'),'base':(a.training/'train_ewt.py','7e738a0e3656fa2b90b6e94d199894dba84de5a343dd1cb1b1e83dbf09b23530'),'trainer':(a.training/'train_ewt_punctuation_v3.py','16790f2d71ffee98ed32a0ab0a31aa1e4e55b47c03fd381c629291da318ac85b'),'profile':(a.training/'ewt_punctuation_v3/lexical_profile.json','045a51b00224785f77983a7c8127f690a1ca8e3c471eb40f6339ceec60c5251b')}
 for p,h in pins.values():assert sha(p)==h
 profile=json.loads(pins['profile'][0].read_text())
 tree=ast.parse(pins['base'][0].read_text());names={'read','initial','features','action','apply','oracle'}
 nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in names or isinstance(n,ast.Assign) and all(isinstance(t,ast.Name) for t in n.targets)]
 env={'collections':collections};exec(compile(ast.Module(body=nodes,type_ignores=[]),'<pinned metadata oracle>','exec'),env)
 raw={}
 for block in pins['raw_train'][0].read_text().strip().split('\n\n'):
  sid=next((l[12:] for l in block.splitlines() if l.startswith('# sent_id = ')),None)
  raw[sid]=[f[1] for l in block.splitlines() if len(f:=l.split('\t'))==10 and f[0].isdigit()]
 cohort=OUT.parent/'train-causal-role-preparation/membership-and-proposed-roles.jsonl'
 assert sha(cohort)=='16be8f69d28429e7e471eb03d0539b4aabc87792f5c6d7ef557cf4a6d81b4837'
 ids={json.loads(l)['sentence_id'] for l in cohort.read_text().splitlines()};assert len(ids)==394
 rows,_=env['read'](pins['raw_train'][0]);rows=sorted([r for r in rows if r['id'] in ids],key=lambda r:r['id']);assert len(rows)==394
 def state(n,path):
  s=env['initial'](n)
  for cls in path:env['apply'](s,cls)
  return s
 def features(s,forms,choice):
  # This is a parity expectation, NOT accepted Source execution.
  n=s['n'];unread=s['unread']
  visible=[profile[w][choice[i]] if i<=unread else 16 for i,w in enumerate(forms)]
  result=env['features'](s,visible)+[374+n]
  support=profile[forms[unread+1]] if unread+1<n else []
  result += [379+2*code+int(code in support) for code in range(17)]
  assert len(result)==25 and all(0<=i<413 for i in result)
  return result
 def arm(r,n,path):
  forms=raw[r['id']][:n];s=state(n,path);representatives={}
  for choices in itertools.product(*(range(len(profile[w])) for w in forms)):
   padded=list(choices)+[0]*(4-n);f=features(s,forms,padded)
   representatives.setdefault(tuple(f),padded)
  gold=[profile[w].index(pos) for w,pos in zip(forms,r['pos'][:n])]+[0]*(4-n)
  entries=sorted(representatives.items())
  return {'available_count':n,'choice_vectors':[v for k,v in entries],'expected25':[list(k) for k,v in entries],'gold_choice_vector':gold,'gold_expected25':features(s,forms,gold)}
 paired=[];stops=[]
 for r in rows:
  forms=raw[r['id']];N=len(forms);pathfull=[cls for _,cls in r['oracle']]
  assert all(w in profile and pos in profile[w] for w,pos in zip(forms,r['pos']))
  for p in range(1,N+1):
   path=[];s=state(p,path)
   for cls in pathfull:
    kind,_=env['action'](cls)
    if kind in {'shift','left_arc','right_arc'} and s['unread']>=p:
     stops.append({'sentence_id':r['id'],'prefix_count':p,'replay_classes':path.copy(),'reason':'no_arrived_buffer_token' if kind=='shift' else 'future_endpoint_not_arrived','next_gold_class':cls});break
    # Offline expected reachability only. Actual Native/Source may reject and quarantine.
    pair_id=hashlib.sha256(json.dumps([r['id'],p,len(path),cls],separators=(',',':')).encode()).hexdigest()
    paired.append({'schema':'causal-train-pair@1','admission_status':'UNVALIDATED_REPLAY_REQUEST_NOT_FIT_ELIGIBLE','pair_id':pair_id,'sentence_id':r['id'],'prefix_count':p,'full_count':N,'forms':forms,'gold_pos':r['pos'],'gold_class_path':pathfull,'replay_classes':path.copy(),'target_class':cls,'arms':{'causal':arm(r,p,path),'full':arm(r,N,path)}})
    env['apply'](s,cls);path.append(cls)
   else:stops.append({'sentence_id':r['id'],'prefix_count':p,'replay_classes':path.copy(),'reason':'teacher_complete','next_gold_class':None})
 def write(name,items):
  p=OUT/name;p.write_text(''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n' for x in items));return sha(p)
 outputs={'paired-requests.jsonl':write('paired-requests.jsonl',paired),'stops.jsonl':write('stops.jsonl',stops)}
 manifest={'status':'UNVALIDATED_OFFLINE_EXPECTATIONS_NO_TRAINING_INFERENCE','schema':'causal-train-pair@1','cohort_count':len(rows),'cohort_manifest_sha256':sha(cohort),'paired_target_count':len(paired),'arm_samples_each':len(paired),'prefix_schedule_count':len(stops),'stop_counts':dict(collections.Counter(r['reason'] for r in stops)),'alternative_query_counts':{arm:sum(len(r['arms'][arm]['choice_vectors']) for r in paired) for arm in ['causal','full']},'fixed_profile':profile,'inputs':{k:{'path':str(p),'sha256':h} for k,(p,h) in pins.items()},'outputs':outputs,'generator_sha256':sha(P(__file__)),'multiplicity':'one lexicographically smallest choicevector per distinct ordered25indices per arm; goldchoice+gold25 separately retained','future_shuffle_proposal':'Pair IDs lexicographic initial order; one Random(4907) pair-order shuffle per epoch shared by both arms,12epochs; no sample fitting executed','independent_partition':'unchanged QUARANTINED;394previously fitted TRAIN reused explicitly; no blind calibration/quality proof'}
 (OUT/'manifest.json').write_text(json.dumps(manifest,indent=2,sort_keys=True)+'\n')
 print(json.dumps({k:manifest[k] for k in ['cohort_count','paired_target_count','prefix_schedule_count','stop_counts','alternative_query_counts','outputs']},indent=2))
if __name__=='__main__':main()
