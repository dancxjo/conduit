from pathlib import Path
from itertools import permutations
p=Path('semantics/language/parser.conduit');s=p.read_text()
def tree(xs,op):
 if len(xs)==1:return xs[0]
 mid=len(xs)//2;return '('+tree(xs[:mid],op)+op+tree(xs[mid:],op)+')'
terms=[]
for length in range(2,5):
 for path in permutations(range(4),length):
  comparisons=[f'.top == {path[0]}',f'.request.state.unread == {path[-1]}']+[f'.request.state.heads.{a} == {b}' for a,b in zip(path,path[1:])]
  terms.append('!('+tree(comparisons,' && ')+')')
replacement='(!variant/is(.request.action, "right_arc") || '+tree(terms,' && ')+')'
needle='!variant/is(.request.action, "right_arc")'
while needle in s:
 n=s.index(needle);start=n-1;assert s[start]=='(';end=start+1;depth=1
 while depth:
  if s[end]=='(':depth+=1
  if s[end]==')':depth-=1
  end+=1
 s=s[:start]+replacement.replace(needle,'variant/is(.request.action, "shift") || variant/is(.request.action, "reduce") || variant/is(.request.action, "left_arc")')+s[end:]
p.write_text(s)
