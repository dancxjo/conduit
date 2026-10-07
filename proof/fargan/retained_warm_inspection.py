"""Development artifact inspection only; never grants Native-law admission."""
import hashlib,json,math,pathlib,re,struct,sys
if len(sys.argv)!=3:raise SystemExit('usage: retained_warm_inspection.py ARTIFACT_DIRECTORY SCRATCH_DIRECTORY')
OUT=pathlib.Path(sys.argv[1]);WORK=pathlib.Path(sys.argv[2]);WORK.mkdir(parents=True,exist_ok=True)
class Cursor:
 def __init__(self,data,offset=0):self.data=data;self.offset=offset;self.nodes=0
 def take(self,n):
  if n<0 or self.offset+n>len(self.data):raise ValueError('truncated')
  b=self.data[self.offset:self.offset+n];self.offset+=n;return b
 def num(self,fmt):return struct.unpack(fmt,self.take(struct.calcsize(fmt)))[0]
 def text(self):
  n=self.num('<I')
  if n>4096:raise ValueError('text bound')
  return self.take(n).decode()
 def ty(self,depth=0):
  self.nodes+=1
  if depth>64 or self.nodes>8192:raise ValueError('type bound')
  tag=self.num('B')
  if tag==0:return ('leaf',self.text())
  if tag==5:return ('nominal',self.text(),self.ty(depth+1))
  if tag==1:return ('collection',self.num('<H'),self.ty(depth+1))
  if tag==4:return ('sequence',self.num('<H'),self.num('<H'),self.ty(depth+1))
  if tag in (2,3):
   schema=self.text();n=self.num('<I')
   if n>1024:raise ValueError('type fields')
   return ('record' if tag==2 else 'variant',schema,[(self.text(),self.ty(depth+1)) for _ in range(n)])
  raise ValueError('type tag')
 def value(self,ty,depth=0):
  self.nodes+=1
  if depth>64 or self.nodes>16384:raise ValueError('value bound')
  if ty[0]=='nominal':return self.value(ty[2],depth+1)
  tag=self.num('B');kind=ty[0]
  if kind=='leaf' and tag==0:
   n=self.num('<I')
   if n>4096:raise ValueError('leaf bound')
   return self.take(n)
  if kind in ('collection','sequence') and tag==1:
   n=self.num('<I');lo=hi=ty[1] if kind=='collection' else None
   if kind=='sequence':lo,hi=ty[1:3]
   if not lo<=n<=hi:raise ValueError('collection length')
   return [self.value(ty[-1],depth+1) for _ in range(n)]
  if kind=='record' and tag==2:
   n=self.num('<I')
   if n!=len(ty[2]):raise ValueError('record count')
   result={}
   for name,field in ty[2]:
    if self.text()!=name:raise ValueError('field order')
    result[name]=self.value(field,depth+1)
   return result
  if kind=='variant' and tag==3:
   name=self.text();fields=dict(ty[2])
   if name not in fields:raise ValueError('variant tag')
   return (name,self.value(fields[name],depth+1))
  raise ValueError('shape')
 def canonical(self):
  start=self.offset;self.nodes=0;ty=self.ty();value=self.value(ty)
  if self.offset-start>16384:raise ValueError('canonical frame bound')
  return ty,value,self.data[start:self.offset]
def floats(value):
 values=[struct.unpack('<f',x)[0] for x in value]
 if not all(math.isfinite(x) for x in values):raise ValueError('nonfinite')
 return values
def state(value):
 signal=value['signal'];return sum([floats(signal[k]) for k in ('conv_history','gru1','gru2','gru3')],[])+floats(value['pitch_history'])+floats(value['deemphasis_state'])
def save(name,values):
 (WORK/name).write_bytes(struct.pack('<'+'f'*len(values),*values))
def main():
 manifest=json.loads((OUT/'manifest.json').read_text());raw=(OUT/'raw-epochs.json').read_bytes()
 assert hashlib.sha256(raw).hexdigest()==manifest['raw_epochs_sha256']
 epochs=json.loads(raw);states=[];periods=[];pcm=[]
 for i,row in enumerate(epochs,1):
  c=Cursor(bytes(row));ty,v,_=c.canonical();assert c.offset==len(row)
  assert ty[0]=='record' and ty[1].startswith('type/FarganPcm16EpochResult@')
  assert struct.unpack('<Q',v['epoch'])[0]==i
  states+=state(v['next_state']);periods.append(struct.unpack('<H',v['next_period'])[0]);pcm += [struct.unpack('<h',x)[0] for x in v['pcm_i16']]
 save('actual-next-states.f32le',states);(WORK/'actual-raw-pcm.i16le').write_bytes(struct.pack('<'+'h'*len(pcm),*pcm))
 basis=(OUT/'session-basis.bin').read_bytes();assert hashlib.sha256(basis).hexdigest()==manifest['session_basis_sha256']
 c=Cursor(basis);handoff=None
 while c.offset<len(basis):
  n=c.num('<Q');name=c.take(n).decode();n=c.num('<Q');value=c.take(n)
  if name=='retained_native_handoff_and_linguistic_receipts':handoff=value
 assert handoff is not None
 hits=[m.start() for m in re.finditer(b'type/FarganFeatureProposalEpoch',handoff)];assert len(hits)==1
 c=Cursor(handoff,hits[0]-5);ty,proposal,canonical=c.canonical();assert ty[0]=='record' and ty[1].startswith('type/FarganFeatureProposalEpoch@')
 (WORK/'actual-first-proposal.canonical').write_bytes(canonical);feature=floats(proposal['features']);assert len(feature)==20
 save('actual-first-feature.f32le',feature);save('actual-warm-five.f32le',feature*5)
 warm={}
 while c.offset<len(handoff):
  ty,value,canonical=c.canonical();name=ty[1].split('@')[0];assert name not in warm;warm[name]=value
  (WORK/(name.split('/')[-1]+'.canonical')).write_bytes(canonical)
 assert len(warm)==3
 save('actual-warm-state.f32le',state(warm['type/FarganSubframeState']))
 history=warm['type/NumericHistory2x64'];save('actual-warm-history.f32le',sum([floats(row) for row in history],[]))
 report={'scope':'retained artifact inspection only; no new Native admission or utterance coverage claim','raw_epochs':len(epochs),'actual_next_state_values':len(states),'actual_raw_pcm_samples':len(pcm),'periods':periods,'retained_feature_rows':1,'retained_warm_feature_calls':5,'complete_feature_row_trace':False,'basis_sha256':manifest['session_basis_sha256'],'raw_epochs_sha256':manifest['raw_epochs_sha256'],'first_feature':feature,'first_proposal_sha256':hashlib.sha256((WORK/'actual-first-proposal.canonical').read_bytes()).hexdigest(),'warm_period':struct.unpack('<H',warm['type/FarganPeriod'])[0],'warm_state_values':837,'warm_history_values':128}
 (WORK/'retained-inspection.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:report[k] for k in ('raw_epochs','retained_feature_rows','complete_feature_row_trace','first_proposal_sha256')}))
if __name__=='__main__':main()
