from pathlib import Path

def whole(s):
 if not s.startswith('(') or not s.endswith(')'):return False
 depth=0
 for i,ch in enumerate(s):
  if ch=='(':depth+=1
  if ch==')':depth-=1
  if depth==0 and i!=len(s)-1:return False
 return True

def prune(s):
 out='';i=0
 while i<len(s):
  if s[i]=='(':
   depth=1;j=i+1
   while depth:
    if s[j]=='(':depth+=1
    if s[j]==')':depth-=1
    j+=1
   content=prune(s[i+1:j-1]).strip()
   while whole(content):content=content[1:-1].strip()
   out+='('+content+')';i=j
  else:out+=s[i];i+=1
 return out
p=Path('semantics/language/parser.conduit');p.write_text('\n'.join(prune(l) if l.startswith(') = ') else l for l in p.read_text().splitlines())+'\n')
