import sys
from pathlib import Path
class Decoder:
 def __init__(self,b):self.b=b;self.p=0
 def take(self,n):v=self.b[self.p:self.p+n];self.p+=n;assert len(v)==n;return v
 def num(self,n=4):return int.from_bytes(self.take(n),'little')
 def text(self):return self.take(self.num()).decode()
 def ty(self):
  k=self.num(1)
  if k==0:return(k,self.text())
  if k==1:return(k,self.num(2),self.ty())
  if k in (2,3):
   name=self.text();return(k,name,[(self.text(),self.ty()) for _ in range(self.num())])
  if k==4:return(k,self.num(2),self.num(2),self.ty())
  if k==5:return(k,self.text(),self.ty())
  raise ValueError(k)
 def val(self,t):
  if t[0]==5:return self.val(t[2])
  k=self.num(1)
  if k==0:return self.take(self.num())
  if k==1:return [self.val(t[-1]) for _ in range(self.num())]
  if k==2:
   assert self.num()==len(t[2]);out={}
   for name,ty in t[2]:assert self.text()==name;out[name]=self.val(ty)
   return out
  if k==3:
   tag=self.text();return{tag:self.val(dict(t[2])[tag])}
  raise ValueError(k)
def simplify(x):
 if isinstance(x,bytes):return int.from_bytes(x,'little') if len(x)<=8 else {'bytes':len(x)}
 if isinstance(x,dict):return{k:simplify(v) for k,v in x.items()}
 return x
lines=Path(sys.argv[1]).read_text().splitlines()
for i,line in enumerate(lines):
 if line.startswith('CONDUIT_PROTOCOL_OUTPUT'):
  d=Decoder(bytes.fromhex(lines[i+1]));t=d.ty();v=d.val(t);assert d.p==len(d.b);print(simplify(v))
