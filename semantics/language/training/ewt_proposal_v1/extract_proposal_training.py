"""External supervision extraction; never runtime parser/linguistic authority."""
import sys,pathlib,json,struct,hashlib,collections
import proposal_window8_training_reference as proposal
reference=proposal.legacy
train=pathlib.Path(sys.argv[1]);dictionary=pathlib.Path(sys.argv[2]);teaching=pathlib.Path(sys.argv[3]);destination=pathlib.Path(sys.argv[4]);repetitions=int(sys.argv[5])
if not 1<=repetitions<=128:raise SystemExit('teaching repetition bound')
if hashlib.sha256(train.read_bytes()).hexdigest()!='d68e06122a702464c613076523d56740f047e5bbe89dd90ec32737e04d952143':raise SystemExit('TRAIN pin')
profile={}
for p in sorted(dictionary.glob('shard-*.json')):
 for e in json.loads(p.read_text())['entries']:
  if e['surface'] in profile:raise SystemExit('duplicate dictionary surface')
  profile[e['surface']]=e['pos_codes']
if len(profile)!=8192:raise SystemExit('dictionary extent')
rows,skipped=reference.read(train);covered=[];excluded=[]
for row in rows:
 missing=[{'ordinal':i,'surface':form,'gold_pos':code,'candidates':profile.get(form,[])}for i,(form,code)in enumerate(zip(row['forms'],row['pos']))if code not in profile.get(form,[])]
 if missing:excluded.append({'id':row['id'],'missing':missing})
 else:covered.append(row)
extra=json.loads(teaching.read_text())['rows']
for row in extra:
 if any(code not in profile.get(form,[])for form,code in zip(row['forms'],row['pos'])):raise SystemExit('authored gold outside explicit dictionary overlay: '+row['id'])
 row['oracle']=reference.oracle(row)
destination.mkdir(parents=True,exist_ok=False);sample_count=0
with (destination/'samples.bin').open('wb')as out:
 out.write(b'C27TRAIN')
 for row in covered+extra*repetitions:
  state=reference.initial(len(row['pos']));origins=[0]*len(row['pos'])
  for target in row['oracle']:
   gold=proposal.features(state,row['pos'],row,profile,origins)
   alternatives=sorted(set(tuple(i)for i in proposal.alternatives(state,row,profile,origins,row['pos'][:state['unread']])))
   if tuple(gold)not in alternatives or not 1<=len(alternatives)<=16:raise SystemExit('supervision candidate extent')
   sample_count+=1
   if sample_count>1_000_000:raise SystemExit('sample bound')
   out.write(struct.pack('<27H2H',*gold,target,len(alternatives)))
   for indices in alternatives:out.write(struct.pack('<27H',*indices))
   reference.apply(state,target)
manifest={'scope':'TRAIN+explicit authored supervised feature tuples; no native runtime/heldout/linguistic acceptance proof','train_sha256':hashlib.sha256(train.read_bytes()).hexdigest(),'dictionary_manifest_sha256':hashlib.sha256((dictionary/'manifest.json').read_bytes()).hexdigest(),'teaching_sha256':hashlib.sha256(teaching.read_bytes()).hexdigest(),'extractor_sha256':hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),'feature_reference_sha256':hashlib.sha256(pathlib.Path(proposal.__file__).read_bytes()).hexdigest(),'legacy_reference_sha256':hashlib.sha256(pathlib.Path(reference.__file__).read_bytes()).hexdigest(),'train_eligible_rows':len(rows),'train_covered_rows':len(covered),'train_exclusions':excluded,'train_corpus_shape_skips':skipped,'authored_rows':len(extra),'authored_repetitions':repetitions,'samples':sample_count,'samples_sha256':hashlib.sha256((destination/'samples.bin').read_bytes()).hexdigest(),'origin_training':'all selected boundedTRAIN/authored surfaces in dictionary: reviewed-origin0; unknown-origin1 not observed by this training, origin remains explicit at runtime','selected_history':'gold TRAIN/authored oracle trajectory, consumed POS retained, current alternatives varied; no runtime gold constraints'}
(destination/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(json.dumps({'samples':sample_count,'train_covered':len(covered),'train_excluded':len(excluded),'authored_rows':len(extra)}))
