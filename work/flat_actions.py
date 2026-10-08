from pathlib import Path
p=Path('semantics/language/parser.conduit');s=p.read_text()
def tree(xs,op):
 if len(xs)==1:return xs[0]
 mid=len(xs)//2;return '('+tree(xs[:mid],op)+op+tree(xs[mid:],op)+')'
st='.request.state';base='variant/is(.request.relation.base, "root")'
shift=tree([f'{st}.unread < {st}.token_count',f'{st}.depth < 5'],' && ')
reduce=tree([f'{st}.depth > 1','.top < 4',f'sequence/at({st}.heads, .top) < 5'],' && ')
left=tree([f'{st}.unread < {st}.token_count',f'{st}.depth > 1','.top < 4',f'.top >= {st}.committed',f'sequence/at({st}.heads, .top) == 5',f'!{base}'],' && ')
root=tree(['.top == 4',base]+[f'{st}.heads.{i} != 4' for i in range(4)],' && ')
normal=tree(['.top < 4',f'!{base}'],' && ')
right=tree([f'{st}.unread < {st}.token_count',f'{st}.depth < 5',f'{st}.unread >= {st}.committed',f'sequence/at({st}.heads, {st}.unread) == 5',f'({root} || {normal})'],' && ')
actions=tree([f'(variant/is(.request.action, "{name}") && {value})' for name,value in [('shift',shift),('reduce',reduce),('left_arc',left),('right_arc',right)]],' || ')
needle='variant/is(.request.action, "shift") ?'
while needle in s:
 n=s.index(needle);start=n-1;assert s[start]=='(';end=start+1;depth=1
 while depth:
  if s[end]=='(':depth+=1
  if s[end]==')':depth-=1
  end+=1
 s=s[:start]+actions+s[end:]
p.write_text(s)
