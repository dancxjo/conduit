"""Evaluation inputs/provenance only; no parser or metric authority."""
import pathlib,json,hashlib,sys
import proposal_window8_training_reference as p
base=pathlib.Path(__file__).resolve().parent
project=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907')
corpus=project/'work/window8/pinned-ewt';out=base/'proposal-446-evaluation-inputs-v1';out.mkdir(exist_ok=True)
pins={'train':'d68e06122a702464c613076523d56740f047e5bbe89dd90ec32737e04d952143','dev':'39239e0a60db3ae68f4b7036189f11b6692741d10ff8240dd91f74f2760d90f8','test':'fa024f43dc5da3c5ac02563bc9bd0e974f46cbb1560823976a8f342a37dc494a'}
paths={split:next(corpus.glob(f'*{split}.conllu'))for split in pins}
for split,path in paths.items():assert hashlib.sha256(path.read_bytes()).hexdigest()==pins[split]
# Full TRAIN, including rows outside the parser window, determines exact form-sequence overlap.
train_sequences=set()
for block in paths['train'].read_text().strip().split('\n\n'):
 forms=tuple(f[1]for line in block.splitlines()if len(f:=line.split('\t'))==10 and f[0].isdigit())
 if forms:train_sequences.add(forms)
teaching=json.loads((base/'authored-proposal-window8-teaching-v1.json').read_text())['rows'];teach_sequences={tuple(r['forms'])for r in teaching}
manifest={'scope':'Pinned full bounded UD evaluation input denominators and exact fullTRAIN/authored-teaching overlap; no parser/Native/runtime/heldout accuracy claim','pins':pins,'splits':{}}
for split in ['dev','test']:
 rows,skips=p.legacy.read(paths[split]);disjoint=[]
 originals={}
 for block in paths[split].read_text().strip().split('\n\n'):
  comments={line.split(' = ',1)[0]:line.split(' = ',1)[1]for line in block.splitlines()if line.startswith('# ')and' = 'in line}
  if '# sent_id'in comments and'# text'in comments:originals[comments['# sent_id']]=comments['# text']
 for row in rows:
  row['text']=originals[row['id']] # Exact original UD text; actual producer alignment refusals remain counted.
  row['full_train_form_sequence_overlap']=tuple(row['forms'])in train_sequences
  row['authored_teaching_form_sequence_overlap']=tuple(row['forms'])in teach_sequences
  if not row['full_train_form_sequence_overlap']and not row['authored_teaching_form_sequence_overlap']:disjoint.append(row['id'])
 data={'scope':manifest['scope'],'transport':'exact original UD # text; actual lexical reconstruction/alignment must be checked, failures remain in denominator','rows':rows}
 raw=(json.dumps(data,ensure_ascii=False,indent=2)+'\n').encode();(out/f'{split}.json').write_bytes(raw)
 manifest['splits'][split]={'rows':len(rows),'tokens':sum(len(r['forms'])for r in rows),'corpus_shape_skips':skips,'fullTRAIN_and_teaching_sequence_disjoint_rows':len(disjoint),'sequence_disjoint_ids':disjoint,'evaluation_sha256':hashlib.sha256(raw).hexdigest()}
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(json.dumps({s:{k:v for k,v in d.items()if k!='sequence_disjoint_ids'}for s,d in manifest['splits'].items()}))
