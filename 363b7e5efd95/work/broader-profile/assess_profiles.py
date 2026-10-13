import sys,pathlib,json,collections,hashlib
repo=pathlib.Path('/home/dancxjo/conduit-4907-native-admission');sys.path.insert(0,str(repo/'semantics/language/training'));import window8_reference as ref
project=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907');corpus=project/'work/window8/pinned-ewt';out=project/'work/window8/broader-profile-assessment';out.mkdir(exist_ok=True)
splits={};exc={}
for split in ['train','dev','test']:splits[split],exc[split]=ref.read(corpus/f'en_ewt-ud-{split}.conllu')
reviewed=json.loads((repo/'semantics/language/training/ewt_joint_v3_window8/reviewed_lexical_alternatives.json').read_text());reviewed.update({'old':[0,7],'man':[7,15],'boats':[7],',':[12],'Hello':[6],'Thanks':[6]})
observed=collections.defaultdict(set)
for r in splits['train']:
 for f,p in zip(r['forms'],r['pos']):observed[f].add(p)
clause=lambda r:any(x in ['ccomp','advcl','csubj','xcomp','conj','parataxis'] for x in r['relations']) and sum(p in [3,15] for p in r['pos'])>=2
trainseq=set()
for block in (corpus/'en_ewt-ud-train.conllu').read_text().strip().split('\n\n'):
 forms=tuple(f[1] for line in block.splitlines() if not line.startswith('#') and len(f:=line.split('\t'))==10 and f[0].isdigit())
 if forms:trainseq.add(forms)
teaching=json.loads((repo/'semantics/language/training/ewt_joint_v3_window8/vocative_training.json').read_text());trainseq.update(tuple(r['forms']) for r in teaching)
results={}
for option in ['ordinary_frequency','clause_frequency','clause_greedy']:
 pool=[r for r in splits['train'] if clause(r)] if option!='ordinary_frequency' else splits['train'];freq=collections.Counter(f for r in pool for f in r['forms']);profile=dict(reviewed)
 if option=='clause_greedy':
  remaining=[r for r in pool if all(f in profile or (f in observed and len(observed[f])<=4) for f in r['forms'])]
  while len(profile)<64:
   candidates=[r for r in remaining if 0<len(set(r['forms'])-profile.keys())<=64-len(profile)]
   if not candidates:break
   row=min(candidates,key=lambda r:(len(set(r['forms'])-profile.keys()),-len(r['forms']),r['id']))
   for f in sorted(set(row['forms'])-profile.keys()):profile[f]=sorted(observed[f])
 for f in sorted(freq,key=lambda f:(-freq[f],f)):
  if len(profile)>=64:break
  if f not in profile and len(observed[f])<=4:profile[f]=sorted(observed[f])
 report={}
 for split,rows in splits.items():
  covered=[r for r in rows if all(f in profile and p in profile[f] for f,p in zip(r['forms'],r['pos']))]
  independent=[r for r in covered if tuple(r['forms']) not in trainseq]
  clauses=[r for r in independent if clause(r)]
  report[split]={'covered':len(covered),'covered_clause_rows':sum(map(clause,covered)),'train_sequence_disjoint':len(independent),'independent_clause_rows':len(clauses),'independent_clause_examples':[{k:v for k,v in r.items() if k!='oracle'} for r in clauses[:12]]}
 results[option]={'profile':profile,'report':report};print(option,{k:{a:b for a,b in v.items() if a!='independent_clause_examples'} for k,v in report.items()})
(out/'options.json').write_text(json.dumps({'scope':'TRAIN-only profile selection; DEV/TEST observed once after choices fixed; no inference/accuracy; eligible<=8 projective profile only','reviewed_forms':reviewed,'corpus_sha256':{s:hashlib.sha256((corpus/f'en_ewt-ud-{s}.conllu').read_bytes()).hexdigest() for s in splits},'options':results},indent=2)+'\n')
