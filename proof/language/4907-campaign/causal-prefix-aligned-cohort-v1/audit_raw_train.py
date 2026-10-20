#!/usr/bin/env python3
"""Independent raw annotation/path provenance only. No fitting or Source substitute."""
import collections,hashlib,json,pathlib
P=pathlib.Path;root=P(__file__).resolve().parent;campaign=root.parent.parent.parent
original=root.parent/'causal-prefix-paired-inputs'
manifest=json.loads((original/'manifest.json').read_text())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
raw_path=campaign/manifest['inputs']['raw_train']['path']
assert sha(raw_path)==manifest['inputs']['raw_train']['sha256']
POS='ADJ ADP ADV AUX CCONJ DET INTJ NOUN NUM PART PRON PROPN PUNCT SCONJ SYM VERB X'.split()
REL='acl advcl advmod amod appos aux case cc ccomp compound conj cop csubj dep det discourse dislocated expl fixed flat goeswith iobj list mark nmod nsubj nummod obj obl orphan parataxis punct reparandum root vocative xcomp'.split()
# Preserve the actual 37-class contract, including clf.
REL.insert(9,'clf');assert len(REL)==37
raw={}
for block in raw_path.read_text().strip().split('\n\n'):
 sid=next((line[12:] for line in block.splitlines() if line.startswith('# sent_id = ')),None)
 ts=[line.split('\t') for line in block.splitlines() if not line.startswith('#') and len(line.split('\t'))==10]
 if sid is not None:assert sid not in raw;raw[sid]=ts
rows=[json.loads(l) for l in (original/'paired-requests.jsonl').read_text().splitlines()]
seen={};actions=collections.Counter()
for row in rows:
 sid=row['sentence_id'];ts=raw[sid];n=len(ts)
 assert [t[0] for t in ts]==list(map(str,range(1,n+1))) and 1<=n<=4
 forms=[t[1] for t in ts];pos=[POS.index(t[3]) for t in ts]
 assert row['forms']==forms and row['gold_pos']==pos and row['full_count']==n
 path=row['gold_class_path'];assert len(path)<=12
 if sid in seen:assert seen[sid]==path;continue
 seen[sid]=path
 stack=[4];unread=0;heads=[5]*n;rels=[None]*n
 for cls in path:
  actions[cls]+=1;top=stack[-1]
  if cls==0:
   assert unread<n;stack.append(unread);unread+=1
  elif cls==1:
   assert top!=4 and heads[top]!=5;stack.pop()
  elif cls<39:
   assert 2<=cls and top!=4 and unread<n and heads[top]==5
   heads[top]=unread;rels[top]=REL[cls-2];stack.pop()
  else:
   assert 39<=cls<76 and unread<n and heads[unread]==5
   heads[unread]=top;rels[unread]=REL[cls-39];stack.append(unread);unread+=1
 assert unread==n and stack==[4]
 assert heads==[4 if t[6]=='0' else int(t[6])-1 for t in ts]
 assert rels==[t[7].split(':')[0] for t in ts]
report={'status':'RAW_TRAIN_FORMS_POS_GOLD_PATH_PROVENANCE_PASS_NOT_SOURCE_EXECUTION','raw_train_sha256':sha(raw_path),'requests_sha256':sha(original/'paired-requests.jsonl'),'audit_sha256':sha(P(__file__)),'sentences':len(seen),'pairs':len(rows),'unique_sentence_path_actions':dict(actions),'scope':'Independent raw FORM/UPOS/head/base-relation check and bounded path replay. No labels from DEV/TEST, no new Source execution, no fit or runtime acceptance.'}
assert len(seen)==394 and len(rows)==1620
out=root/'raw-train-review.json';assert not out.exists();out.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n');print(json.dumps(report,indent=2))
