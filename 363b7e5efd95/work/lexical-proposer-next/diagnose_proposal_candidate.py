"""External numerical teaching diagnostic; no Native/Source acceptance authority."""
import pathlib,json,struct,hashlib,time,copy
import proposal_window8_training_reference as p
base=pathlib.Path(__file__).resolve().parent
artifact=base/'proposal-446-training-v2/proposal_window8.i16'
raw=artifact.read_bytes()
assert raw[:8]==b'CI16SUM1' and struct.unpack('<3I',raw[8:20])==(446,76,27)
weights=struct.unpack('<33896h',raw[20:])
profile={e['surface']:e['pos_codes'] for f in sorted((base/'train-dictionary-authored-overlay-v2').glob('shard-*.json')) for e in json.loads(f.read_text())['entries']}
rows=json.loads((base/'authored-proposal-window8-teaching-v1.json').read_text())['rows']
def scores(indices):return [sum(weights[c*446+i] for i in indices) for c in range(76)]
results=[]
for row in rows:
 started=time.monotonic();state=p.legacy.initial(len(row['forms']));history=[];correct=0
 for target in p.legacy.oracle(row):
  alternatives=[]
  for indices in p.alternatives(state,row,profile,[0]*len(row['forms']),history):
   ss=scores(indices)
   alternatives.extend((score,-code,indices) for code,score in enumerate(ss) if p.legacy.legal(state,code))
  chosen=max(alternatives,key=lambda a:(a[0],a[1],tuple(-i for i in a[2])))
  correct+=(-chosen[1]==target)
  # Teacher forcing is explicit: this diagnoses class separation, not free parsing.
  p.legacy.apply(state,target);history=row['pos'][:state['unread']]
 results.append({'id':row['id'],'text':row.get('text'),'oracle_steps':len(p.legacy.oracle(row)),'correct_teacher_forced_class_choices':correct,'elapsed_seconds':time.monotonic()-started})
manifest={'scope':'External numerical teacher-forced class diagnostic only; not free-running parse, Native acceptance, heldout accuracy or stability proof','artifact_sha256':hashlib.sha256(artifact.read_bytes()).hexdigest(),'teaching_rows':len(rows),'rows':results,'correct_steps':sum(r['correct_teacher_forced_class_choices'] for r in results),'total_steps':sum(r['oracle_steps'] for r in results)}
(base/'proposal-446-training-v2/teacher-forced-diagnostic.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({k:v for k,v in manifest.items() if k!='rows'}))
