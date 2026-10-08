#!/usr/bin/env python3
"""Development-only comparison of captured Source rows to pinned scalar C output."""
import argparse, hashlib, json, math, pathlib, struct
p=argparse.ArgumentParser();p.add_argument('capture',type=pathlib.Path);p.add_argument('oracle',type=pathlib.Path);a=p.parse_args()
def fs(path):
 b=path.read_bytes();assert len(b)%4==0;return struct.unpack('<%df'%(len(b)//4),b)
def metric(left,right):
 assert len(left)==len(right) and all(math.isfinite(x) for x in left+right)
 d=[abs(x-y) for x,y in zip(left,right)];return {'count':len(d),'maximum_absolute':max(d,default=0),'rms':math.sqrt(sum(x*x for x in d)/len(d)) if d else 0}
b=a.oracle.read_bytes(); warm=3864;row=5792;assert len(b)>=warm and (len(b)-warm)%row==0
count=(len(b)-warm)//row;result={'rows':count,'scope':'same retained rows, scalar fullfloat oracle, not quality or boot acceptance','oracle_sha256':hashlib.sha256(b).hexdigest()}
condition=[];history=[];pcm=[];state=[];epochs=[]
for i in range(count):
 o=warm+i*row;epoch,period=struct.unpack_from('<Qi',b,o);epochs.append(epoch);v=struct.unpack_from('<1445f',b,o+12);condition.extend(v[:320]);history.extend(v[320:448]);pcm.extend(v[448:608]);state.extend(v[608:])
assert epochs==list(range(1,count+1))
for name,values in [('condition',condition),('history',history),('state',state)]:result[name]=metric(list(fs(a.capture/('source-'+name+'.f32le'))),values)
# Float values use the exact Source normalized saturation and nearest ties-away
# law. Upstream's floor(.5+x) and negative clipping differ and are reported apart.
f32=lambda x:struct.unpack('<f',struct.pack('<f',x))[0]
upper=struct.unpack('<f',bytes.fromhex('00fe7f3f'))[0]
source_q=[];upstream_q=[]
for x in pcm:
 y=f32(min(upper,max(-1.0,x))*32768.0);source_q.append(math.floor(y+.5) if y>=0 else math.ceil(y-.5))
 upstream_q.append(math.floor(.5+min(32767.,max(-32767.,32768.*x))))
actual_bytes=(a.capture/'source-pcm.i16le').read_bytes()
actual=list(struct.unpack('<%dh'%(len(actual_bytes)//2),actual_bytes));assert len(actual)==len(source_q)
result['pcm_source_rounding_profile']=metric(actual,source_q)
result['pcm_upstream_rounding_profile']=metric(actual,upstream_q)
result['source_profile_c_pcm_range']=[min(source_q),max(source_q)]
result['quantizer_profile_disagreement_samples']=sum(x!=y for x,y in zip(source_q,upstream_q))
actual_state=list(fs(a.capture/'source-state.f32le'))
result['state_components']={};offset=0
for name,width in [('conv',164),('gru1',160),('gru2',128),('gru3',128),('pitch_buffer',256),('deemphasis',1)]:
 left=[];right=[]
 for i in range(count):left.extend(actual_state[i*837+offset:i*837+offset+width]);right.extend(state[i*837+offset:i*837+offset+width])
 result['state_components'][name]=metric(left,right);offset+=width
result['per_epoch']=[{'epoch':epochs[i],'pcm':metric(actual[i*160:(i+1)*160],source_q[i*160:(i+1)*160]),'state':metric(actual_state[i*837:(i+1)*837],state[i*837:(i+1)*837])} for i in range(count)]

(a.capture/'oracle-comparison.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
