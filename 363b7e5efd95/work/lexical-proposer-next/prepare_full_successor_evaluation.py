"""Read-only provenance verification and finite chunks; no model adaptation."""
from pathlib import Path
import json,hashlib,sys
base=Path('/home/dancxjo/conduit-4907-evaluation-identity')
sys.path.insert(0,str(base/'semantics/language/training'))
import window8_reference as reference
project=Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907')
archive=base/'semantics/language/training/ewt_proposal_v1'
corpus=project/'work/window8/pinned-ewt'
out=project/'work/lexical-proposer-next/full-411-evaluation-inputs-v1'
out.mkdir(parents=True,exist_ok=False)
manifest=json.loads((archive/'evaluation-inputs-v1/manifest.json').read_text())
paths={split:corpus/f'en_ewt-ud-{split}.conllu' for split in ('train','dev','test')}
for split,p in paths.items():assert hashlib.sha256(p.read_bytes()).hexdigest()==manifest['pins'][split]
train_sequences=set()
for block in paths['train'].read_text().strip().split('\n\n'):
 forms=tuple(f[1]for line in block.splitlines()if len(f:=line.split('\t'))==10 and f[0].isdigit())
 if forms:train_sequences.add(forms)
teaching=json.loads((archive/'authored-proposal-window8-teaching-v1.json').read_text())['rows']
teaching_sequences={tuple(row['forms'])for row in teaching}
receipt={'scope':'full original bounded UD denominators; all refusals remain counted; no inference or accuracy authority','corpus_pins':manifest['pins'],'maximum_rows_per_chunk':8,'splits':{}}
for split in ('dev','test'):
 source=archive/f'evaluation-inputs-v1/{split}.json';raw=source.read_bytes();rows=json.loads(raw)['rows'];expected=manifest['splits'][split]
 assert hashlib.sha256(raw).hexdigest()==expected['evaluation_sha256']
 parsed,skips=reference.read(paths[split]);assert len(rows)==len(parsed)==expected['rows'];assert skips==expected['corpus_shape_skips']
 texts={}
 for block in paths[split].read_text().strip().split('\n\n'):
  comments={line.split(' = ',1)[0]:line.split(' = ',1)[1]for line in block.splitlines()if line.startswith('# ')and' = 'in line}
  if '# sent_id'in comments and '# text'in comments:texts[comments['# sent_id']]=comments['# text']
 disjoint=[]
 for actual,original in zip(rows,parsed):
  for field in ('id','forms','pos','heads','relations','oracle'):assert actual[field]==original[field]
  assert actual['text']==texts[actual['id']]
  overlap=tuple(actual['forms'])in train_sequences;teaching_overlap=tuple(actual['forms'])in teaching_sequences
  assert actual['full_train_form_sequence_overlap']==overlap
  assert actual['authored_teaching_form_sequence_overlap']==teaching_overlap
  if not overlap and not teaching_overlap:disjoint.append(actual['id'])
 assert disjoint==expected['sequence_disjoint_ids']
 chunks=[];directory=out/split;directory.mkdir()
 for index,start in enumerate(range(0,len(rows),8)):
  group=rows[start:start+8];encoded=(json.dumps({'scope':receipt['scope'],'rows':group},ensure_ascii=False,indent=2)+'\n').encode();path=directory/f'chunk-{index:03}.json';path.write_bytes(encoded);chunks.append({'path':str(path),'sha256':hashlib.sha256(encoded).hexdigest(),'rows':len(group),'tokens':sum(len(r['forms'])for r in group),'ids':[r['id']for r in group]})
 assert [identity for chunk in chunks for identity in chunk['ids']]==[r['id']for r in rows]
 receipt['splits'][split]={'original_evaluation_sha256':hashlib.sha256(raw).hexdigest(),'rows':len(rows),'tokens':sum(len(r['forms'])for r in rows),'sequence_disjoint_rows':len(disjoint),'sequence_disjoint_ids':disjoint,'corpus_shape_skips':skips,'chunks':chunks}
(out/'manifest.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({split:{key:val for key,val in data.items()if key not in ('chunks','sequence_disjoint_ids')}for split,data in receipt['splits'].items()},indent=2))
