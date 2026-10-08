"""Numerical TRAIN-only structured correction. Never a runtime parser authority.

Warm-start exact pinned411 artifact; early update when complete gold trajectory
leaves width4, final update when surviving gold loses final ranking. Full fixed
Source/reference parity and actual runtime acceptance remain separate gates.
"""
import collections,copy,hashlib,json,pathlib,random,resource,struct,sys,time
BASE=pathlib.Path(__file__).resolve().parent.parent/'origin-corrected-v2'
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent));import reference as ref
BINS,CLASSES,LOOKUPS=411,76,27
PIN='400b304df9050715151878af0058620f7f04f471cd289e81c850a1b8e4211f15'
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def load(train,dictionary,teaching):
 if sha(train)!='d68e06122a702464c613076523d56740f047e5bbe89dd90ec32737e04d952143':raise ValueError('TRAIN pin')
 profile={e['surface']:e['pos_codes']for f in sorted(pathlib.Path(dictionary).glob('shard-*.json'))for e in json.loads(f.read_text())['entries']}
 if len(profile)!=8192:raise ValueError('dictionary extent')
 rows,skips=ref.legacy.read(pathlib.Path(train));covered=[];excluded=[]
 for r in rows:
  if all(p in profile.get(f,[])for f,p in zip(r['forms'],r['pos'])):covered.append(r)
  else:excluded.append(r['id'])
 extra=json.loads(pathlib.Path(teaching).read_text())['rows']
 if len(extra)!=22:raise ValueError('teaching extent')
 for r in covered+extra:
  if any(p not in profile.get(f,[])for f,p in zip(r['forms'],r['pos'])):raise ValueError('gold outside proposals')
  r['oracle']=ref.legacy.oracle(r)
 return profile,covered,extra,excluded,skips

def search(row,profile,weights):
 # Tuple: cumulative score, original reference state, selected POS, selection
 # frontier, stable proposal identity, complete action path, sparse feature path.
 n=len(row['forms']);beam=[(0,ref.legacy.initial(n),[16]*n,0,0,(),())];identity=4
 oracle=row['oracle'];goldpath=();goldstate=ref.legacy.initial(n);goldpos=[16]*n;goldfrontier=0;goldscore=0
 for epoch in range(1,33):
  proposals=[]
  for score,state,pos,frontier,prior,path,features in beam:
   if state['unread']==n and len(state['stack'])==1:
    proposals.append((score,state,pos,frontier,prior,path,features));continue
   current=state['unread'];choose=current<n and frontier==current
   for code in (profile[row['forms'][current]]if choose else[pos[current]if current<n else 17]):
    selected=pos.copy()
    if current<n:selected[current]=code
    indices=ref.features(state,selected,row,profile,[0]*n)
    scores=[sum(weights[c*BINS+i]for i in indices)for c in range(CLASSES)]
    for action,delta in enumerate(scores):
     if not ref.legacy.legal(state,action):continue
     child=copy.deepcopy(state);ref.legacy.apply(child,action)
     f=tuple(action*BINS+i for i in indices)
     proposals.append((score+delta,child,selected.copy(),frontier+int(choose),identity,path+(action,),features+f));identity+=1
  proposals.sort(key=lambda b:(-b[0],b[4]));beam=proposals[:4]
  if not beam:raise ValueError('empty legal beam')
  if epoch<=len(oracle):
   current=goldstate['unread']
   if current<n and goldfrontier==current:goldpos[current]=row['pos'][current];goldfrontier+=1
   indices=ref.features(goldstate,goldpos,row,profile,[0]*n);action=oracle[epoch-1]
   if not ref.legacy.legal(goldstate,action):raise ValueError('illegal gold')
   f=tuple(action*BINS+i for i in indices);goldpath+=f;goldscore+=sum(weights[i]for i in f)
   ref.legacy.apply(goldstate,action)
  gold=[b for b in beam if b[5]==tuple(oracle[:epoch])and b[2][:b[3]]==row['pos'][:b[3]]]
  if not gold:return goldpath,beam[0][6],'early',epoch
  if all(s['unread']==n and len(s['stack'])==1 for _,s,_,_,_,_,_ in beam):
   if beam[0] not in gold:return goldpath,beam[0][6],'final',epoch
   return (),(),'correct',epoch
 raise ValueError('epoch ceiling')

def run():
 if len(sys.argv)!=7:raise ValueError('TRAIN DICTIONARY TEACHING OUTPUT EPOCHS REPETITIONS')
 train,dictionary,teaching,dest,epochs,reps=sys.argv[1:];epochs=int(epochs);reps=int(reps)
 if not 1<=epochs<=8 or not 1<=reps<=32:raise ValueError('training bounds')
 resource.setrlimit(resource.RLIMIT_AS,(512*1024**2,512*1024**2));start=time.monotonic()
 dest=pathlib.Path(dest);dest.mkdir(exist_ok=False)
 profile,rows,extra,excluded,skips=load(train,dictionary,teaching)
 raw=(BASE/'proposal_window8.i16').read_bytes()
 if hashlib.sha256(raw).hexdigest()!=PIN or struct.unpack('<3I',raw[8:20])!=(BINS,CLASSES,LOOKUPS):raise ValueError('warm start pin')
 weights=list(struct.unpack('<'+str(BINS*CLASSES)+'h',raw[20:]));totals=[0]*len(weights);times=[0]*len(weights);steps=0
 data=rows+extra*reps
 if len(data)>5000:raise ValueError('row ceiling')
 randomizer=random.Random(4907);logs=[]
 for epoch in range(epochs):
  order=list(range(len(data)));randomizer.shuffle(order);stats=collections.Counter()
  for ordinal,index in enumerate(order):
   steps+=1;positive,negative,kind,depth=search(data[index],profile,weights);stats[kind]+=1
   changes=collections.Counter(positive);changes.subtract(negative)
   for i,count in changes.items():
    if not count:continue
    totals[i]+=(steps-times[i])*weights[i];times[i]=steps;weights[i]+=256*count
   if ordinal%250==0:print(json.dumps({'epoch':epoch+1,'rows_done':ordinal,'stats':dict(stats),'elapsed':time.monotonic()-start}),flush=True)
  logs.append(dict(stats));print(json.dumps({'epoch':epoch+1,'stats':dict(stats)}),flush=True)
 final=[]
 for i,w in enumerate(weights):
  totals[i]+=(steps-times[i])*w
  # Python round is exact ties-to-even for rational integers using divmod.
  sign=-1 if totals[i]<0 else 1;q,r=divmod(abs(totals[i]),steps);q+=int(2*r>steps or 2*r==steps and q%2==1);final.append(sign*q)
 if max(map(abs,final))>32767 or max(map(abs,final))*LOOKUPS>1000000:raise ValueError('original artifact/Source score bounds')
 artifact=b'CI16SUM1'+struct.pack('<3I',BINS,CLASSES,LOOKUPS)+struct.pack('<'+str(len(final))+'h',*final)
 (dest/'proposal_window8.i16').write_bytes(artifact)
 receipt={'scope':'TRAIN+22 reviewed teaching numerical warm-start beam correction only; no runtime selection, heldout metrics or Source equivalence claim','warm_start_sha256':PIN,'artifact_sha256':hashlib.sha256(artifact).hexdigest(),'train_sha256':sha(train),'dictionary_manifest_sha256':sha(pathlib.Path(dictionary)/'manifest.json'),'teaching_sha256':sha(teaching),'trainer_sha256':sha(__file__),'reference_sha256':sha(ref.__file__),'legal_reference_sha256':sha(ref.legacy.__file__),'train_rows':len(rows),'teaching_rows':len(extra),'teaching_repetitions':reps,'excluded_train_rows':excluded,'shape_skips':skips,'epochs':epochs,'seed':4907,'steps':steps,'epoch_updates':logs,'elapsed_seconds':time.monotonic()-start,'maximum_weight':max(map(abs,final)),'RLIMIT_AS_bytes':512*1024**2,'child_maxrss_KiB':resource.getrusage(resource.RUSAGE_SELF).ru_maxrss}
 (dest/'training-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt),flush=True)
if __name__=='__main__':run()
