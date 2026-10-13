"""Bounded bootstrap mathematical encoder; full compiler readmission is the gate."""
import ast
from dataclasses import dataclass
from fractions import Fraction
import hashlib
DOMAIN=b'conduit.info.semantic.v1'
def digest(kind,data):
    tag=kind.encode();return hashlib.sha256(DOMAIN+len(tag).to_bytes(2,'little')+tag+data).digest()
@dataclass(frozen=True)
class Exact:
    coefficient: Fraction
    pi_power: int=0
    def __post_init__(self):
        if not self.coefficient:object.__setattr__(self,'pi_power',0)
    def __bool__(self):return bool(self.coefficient)
    def __mul__(self,other):return Exact(self.coefficient*other.coefficient,self.pi_power+other.pi_power)
    def __truediv__(self,other):
        if not other:raise ValueError('zero division')
        return Exact(self.coefficient/other.coefficient,self.pi_power-other.pi_power)
    def __add__(self,other):
        if not self:return other
        if not other:return self
        if self.pi_power!=other.pi_power:raise ValueError('bootstrap sum outside declared builtin profile')
        return Exact(self.coefficient+other.coefficient,self.pi_power)
    def __le__(self,other):
        if other!=0:raise ValueError('only positivity is inspected')
        return self.coefficient<=0
    def __pow__(self,power):return Exact(self.coefficient**power,self.pi_power*power)
def scalar(source):
    parsed=ast.parse(source,mode='eval')
    def visit(node):
        if isinstance(node,ast.Constant) and isinstance(node.value,(int,float)):
            return Exact(Fraction(ast.get_source_segment(source,node)))
        if isinstance(node,ast.Name) and node.id=='pi':return Exact(Fraction(1),1)
        if isinstance(node,ast.UnaryOp) and isinstance(node.op,ast.USub):
            value=visit(node.operand);return Exact(-value.coefficient,value.pi_power)
        if isinstance(node,ast.BinOp):
            left,right=visit(node.left),visit(node.right)
            if isinstance(node.op,ast.Mult):return left*right
            if isinstance(node.op,ast.Div):return left/right
            if isinstance(node.op,ast.Add):return left+right
            if isinstance(node.op,ast.Sub):return left+Exact(-right.coefficient,right.pi_power)
        raise ValueError('unsupported builtin bootstrap expression')
    return visit(parsed.body)
def magnitude(n):
    limbs=[]
    while n: n,limb=divmod(n,1_000_000_000);limbs.append(limb)
    if len(limbs)>128:raise ValueError('integer limb budget exceeded')
    return len(limbs).to_bytes(2,'little')+b''.join(limb.to_bytes(4,'little') for limb in limbs),len(limbs)
def atom(value):
    n,d=value.numerator,value.denominator
    if d==1 and -(1<<63)<=n<(1<<63):return b'\x01\x01'+n.to_bytes(8,'little',signed=True),1,max(magnitude(abs(n))[1],1)
    numerator,nl=magnitude(abs(n));denominator,dl=magnitude(d)
    return b'\x01\x02'+bytes([n<0])+numerator+denominator,1 if d==1 else 2,max(nl,dl,1)
def wire(value):
    coefficient=value.coefficient
    literal,form,limbs=atom(coefficient)
    if value.pi_power==0:return literal,(form,limbs,0,0,len(literal))
    if value.pi_power not in (-1,1):raise ValueError('builtin symbolic profile exceeded')
    pi=('op',1,0,())
    if value.pi_power==1:
        if coefficient==1:root=pi
        else:
            rational=('literal',literal)
            children=sorted((rational,pi),key=node_hash)
            root=('op',5,0,tuple(children))
    else:root=('op',6,0,(('literal',literal),pi))
    nodes=[];indices={};depths={}
    def visit(node):
        if node in indices:return
        children=() if node[0]=='literal' else node[3]
        for child in children:visit(child)
        depths[node]=1+max((depths[child] for child in children),default=0)
        indices[node]=len(nodes);nodes.append(node)
    visit(root)
    encoded=b'\x01\x03'+len(nodes).to_bytes(2,'little')
    for node in nodes:
        if node[0]=='literal':encoded+=b'\0'+len(node[1]).to_bytes(2,'little')+node[1]
        else:encoded+=bytes(node[1:3])+b''.join(indices[child].to_bytes(2,'little') for child in node[3])
    return encoded,(4,limbs,len(nodes),depths[root],len(encoded))
def node_hash(node):
    if node[0]=='literal':payload=b'\0'+node[1]
    else:payload=bytes(node[1:3])+b''.join(node_hash(child) for child in node[3])
    return digest('numeric/node@1',payload)
def profile(scale,offset):
    result=(scale[0]|offset[0],max(scale[1],offset[1]),max(scale[2],offset[2]),max(scale[3],offset[3]),max(scale[4],offset[4],10))
    form,limbs,nodes,depth,length=result
    return bytes([form])+limbs.to_bytes(2,'little')+nodes.to_bytes(2,'little')+bytes([depth])+length.to_bytes(4,'little')
def encode_unit(family,name,scale,offset,policy,prefix,role,anchor):
    sw,ss=wire(scale);ow,os=wire(offset)
    profile_bytes=profile(ss,os)
    body=b'\x02'+family+name+policy+prefix+bytes([role])+anchor+profile_bytes+len(sw).to_bytes(4,'little')+len(ow).to_bytes(4,'little')+sw+ow
    profile_start=1+493+129+25+3+1+32
    meaning=body[:profile_start]+body[profile_start+10:]
    return body+digest('physical/unit-definition@2',meaning)
def origin(family,symbol,scale,offset,named=None):
    sw,_=wire(scale);ow,_=wire(offset)
    root=family+symbol.encode()+len(sw).to_bytes(4,'little')+sw+len(ow).to_bytes(4,'little')+ow
    anchor=digest('physical/reference-origin@2',root)
    if named is not None:anchor=digest('physical/named-reference-origin@2',anchor+named.encode())
    return anchor
