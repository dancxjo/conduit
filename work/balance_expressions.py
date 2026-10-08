from pathlib import Path

def balance(expr):
 out='';i=0
 while i<len(expr):
  if expr[i]=='(':
   end=i+1;depth=1
   while depth:
    if expr[end]=='(':depth+=1
    if expr[end]==')':depth-=1
    end+=1
   out+='('+balance(expr[i+1:end-1])+')';i=end
  else:out+=expr[i];i+=1
 # Do not split operators across record or ternary boundaries.
 depth=0;has_boundary=False
 for c in out:
  if c in '([{':depth+=1
  elif c in ')]}':depth-=1
  elif depth==0 and c in '?:':has_boundary=True
 if has_boundary:return out
 for op in (' || ',' && '):
  chunks=[];depth=0;start=0;i=0
  while i<len(out):
   if out[i] in '([{':depth+=1
   elif out[i] in ')]}':depth-=1
   if depth==0 and out.startswith(op,i):chunks.append(out[start:i]);i+=len(op);start=i;continue
   i+=1
  if chunks:
   chunks.append(out[start:])
   def tree(xs):
    if len(xs)==1:return xs[0]
    mid=len(xs)//2;return '('+tree(xs[:mid])+op+tree(xs[mid:])+')'
   return tree(chunks)
 return out
p=Path('semantics/language/parser.conduit');p.write_text('\n'.join(l[:4]+balance(l[4:]) if l.startswith(') = ') else l for l in p.read_text().splitlines())+'\n')
