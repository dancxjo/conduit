import json,pathlib
base=pathlib.Path('work/beam-training-candidate/candidate-epoch1');out={}
for split in ['dev','test']:
 rows=json.loads(pathlib.Path('semantics/language/training/ewt_proposal_v1/evaluation-inputs-v1',split+'.json').read_text())['rows'];pred=json.loads((base/(split+'-numerical-diagnostic.json')).read_text())['rows'];assert len(rows)==len(pred)
 groups={}
 for scope in ['all_original_rows','fullTRAIN_and_teaching_sequence_disjoint']:
  counts=dict(rows=0,tokens=0,complete=0,incomplete_search_rows=0,correct_heads=0,correct_base_las=0,correct_pos=0,exact_graph_and_pos=0,vocative_tp=0,vocative_fp=0,vocative_fn=0)
  for gold,actual in zip(rows,pred):
   assert gold['id']==actual['id']
   if scope!='all_original_rows' and(gold.get('full_train_form_sequence_overlap')or gold.get('authored_teaching_form_sequence_overlap')):continue
   counts['rows']+=1;counts['tokens']+=len(gold['forms']);counts['complete']+=actual['complete'];counts['incomplete_search_rows']+=not actual['complete']
   for key in ['correct_heads','correct_base_las','correct_pos']:counts[key]+=actual.get(key,0)
   counts['exact_graph_and_pos']+=actual.get('correct_base_las')==len(gold['forms'])and actual.get('correct_pos')==len(gold['forms'])
   for i,(head,rel)in enumerate(zip(gold['heads'],gold['relations'])):
    predicted=actual.get('relations',[]);ph=actual.get('heads',[]);pv=i<len(predicted)and predicted[i]=='vocative';gv=rel=='vocative';match=pv and gv and ph[i]==head
    counts['vocative_tp']+=match;counts['vocative_fp']+=pv and not match;counts['vocative_fn']+=gv and not match
  for key,num in [('uas','correct_heads'),('base_las','correct_base_las'),('pos_accuracy','correct_pos')]:counts[key]=counts[num]/counts['tokens']if counts['tokens']else None
  groups[scope]=counts
 out[split]=groups
out['scope']='External numerical reference diagnostic on frozen TRAIN-selected candidate; all original row/token denominators retained. Not actual Source/Native/model acceptance, stable-fact or public Session evidence. No tuning on these metrics.'
(base/'heldout-numerical-summary.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
