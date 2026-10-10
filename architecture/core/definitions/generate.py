#!/usr/bin/env python3
"""Regenerate immutable bootstrap capsules from physical.conduit.

The language compiler separately re-admits this source and checks equivalence.
This script is a deterministic bootstrap encoder, not a second definition source.
"""
import argparse
import hashlib
import json
import re
from fractions import Fraction
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCE = HERE / 'physical.conduit'
ARTIFACT = HERE / 'physical.generated.json'
DOMAIN = b'conduit.info.semantic.v1'

def digest(kind, data):
    tag = kind.encode()
    return hashlib.sha256(DOMAIN + len(tag).to_bytes(2, 'little') + tag + data).digest()

def integer(value, size):
    return int(value).to_bytes(size, 'little', signed=True)

def name(value, capacity=128):
    data = value.encode()
    if not 0 < len(data) <= capacity:
        raise ValueError('definition name exceeds bound: ' + value)
    return bytes([len(data)]) + data + bytes(capacity-len(data))

def members(body):
    result = {}; start = 0; depth = 0
    for index, character in enumerate(body + ','):
        depth += character in '{['
        depth -= character in '}]'
        if character == ',' and depth == 0:
            key, value = body[start:index].split(':', 1)
            result[key.strip()] = value.strip(); start = index+1
    return result

def dimension(expression):
    if expression.startswith('{'):
        entries = members(expression[1:-1]) if expression[1:-1].strip() else {}
    else:
        entries = {expression: '1'}
    terms = sorted((digest('physical/dimension-anchor@1', key.encode()), int(power)) for key, power in entries.items())
    if len(terms)>8 or any(power==0 or not -16<=power<=16 for _,power in terms):
        raise ValueError('dimension exceeds bounds')
    return bytes([len(terms)]) + b''.join(anchor+integer(power,1) for anchor,power in terms) + bytes((8-len(terms))*33)

def family(type_name, type_fields, points):
    delta_name = points.get(type_name)
    fields = type_fields[delta_name or type_name]
    delta = name(delta_name,64) if delta_name else bytes(65)
    body = b'\x01' + name(type_name) + dimension(fields['dimension']) + delta + bytes([bool(delta_name)])
    return body + digest('physical/quantity-family@1',body)

def rational(value):
    return Fraction(value)

def policy(fields, prefixes):
    enabled = fields.get('prefixes','none').strip('[]').replace(' ','').split(',')
    decimal = set(exponent for group,_,exponent,_ in prefixes if group=='si' and 'si' in enabled)
    binary = set(exponent for group,_,exponent,_ in prefixes if group=='binary' and 'binary' in enabled)
    decimal_mask = sum(1 << (-x-1 if x<0 else x+29) for x in decimal)
    binary_mask = sum(1 << (x-1) for x in binary)
    return decimal_mask.to_bytes(8,'little') + binary_mask.to_bytes(16,'little') + bytes([int(fields.get('power','1'))])

def canonical_scalar(value):
    n,d,e=value.numerator,value.denominator,0
    if n==0:return 0,1,0
    while n%10==0:n//=10;e+=1
    while d%10==0:d//=10;e-=1
    return n,d,e

def regenerate():
    source = SOURCE.read_text(); types = {}; points = {}; raw_units = []; prefixes=[]; binding=None; synthesized=[]
    for line in source.splitlines():
        line=line.strip()
        if line.startswith('# binding '):
            value=line[10:]
            if ' = ' in value:synthesized.append(tuple(value.split(' = ',1)))
            else:binding=value
            continue
        if not line or line.startswith('#') or line.startswith('dimension '): continue
        match=re.fullmatch(r'prefix (si|binary) (\S+) = \{ (.*) \}',line)
        if match:
            group,symbol,body=match.groups(); fields=members(body)
            prefixes.append((group,symbol,int(fields['exponent']),fields.get('alias'))); continue
        match=re.fullmatch(r'type (\w+) = quantity \{ (.*) \}',line)
        if match:
            type_name,body=match.groups(); fields=members(body); types[type_name]=fields
            if 'point' in fields:points[type_name]=fields['point']
            continue
        match=re.fullmatch(r'unit (\S+) : (\w+) = \{ (.*) \}',line)
        if match:
            symbol,type_name,body=match.groups();raw_units.append((binding,symbol,type_name,members(body)));binding=None;continue
        raise ValueError('unsupported source line: '+line)
    family_caps={type_name:family(type_name,types,points) for type_name in types if type_name not in points.values()}
    units={}; pending=list(raw_units); bindings={}
    while pending:
        count=len(pending)
        for item in pending[:]:
            binding,symbol,type_name,fields=item; ref=fields['reference']
            origin=ref=='origin' or ref.startswith('origin(')
            if not origin and ref not in units:continue
            family_bytes=family_caps[type_name]; scale=rational(fields['scale']);offset=rational(fields.get('offset','0'))
            if origin:anchor=None
            else:
                previous=units[ref];scale=scale*previous['scale'];offset=offset*previous['scale']+previous['offset'];anchor=previous['anchor']
            import math
            sn,sd,se=canonical_scalar(scale);on,od,oe=canonical_scalar(offset)
            common=math.lcm(sd,od);sn*=common//sd;on*=common//od
            reduction=math.gcd(math.gcd(sn,abs(on)),common);sn//=reduction;on//=reduction;common//=reduction
            if scale<=0 or (type_name not in points and offset):raise ValueError('invalid transform '+symbol)
            if type_name in points:
                delta_fields=members(fields['delta'][1:-1]);delta_ref=delta_fields['reference'];ds=rational(delta_fields['scale'])
                if delta_fields['quantity']!=points[type_name]:raise ValueError('wrong delta association')
                if delta_ref!='origin':
                    if delta_ref not in units:continue
                    ds*=units[delta_ref]['scale']
                if ds!=scale or rational(delta_fields.get('offset','0'))!=0:raise ValueError('point/delta scale mismatch')
            body=b'\x01'+family_bytes+name(symbol)+integer(sn,16)+integer(on,16)+integer(common,16)+integer(se,2)+integer(oe,2)+policy(fields,prefixes)+bytes(3)+bytes([1 if type_name in points else 0])
            if anchor is None:
                def scalar_identity(value):
                    if not value:return bytes(41)
                    n=abs(value.numerator);d=value.denominator;twos=0;fives=0
                    while n%2==0:n//=2;twos+=1
                    while d%2==0:d//=2;twos-=1
                    while n%5==0:n//=5;fives+=1
                    while d%5==0:d//=5;fives-=1
                    return bytes([2 if value<0 else 1])+n.to_bytes(16,'little')+d.to_bytes(16,'little')+integer(twos,4)+integer(fives,4)
                anchor=digest('physical/reference-origin@1',family_bytes[-32:]+name(symbol)+scalar_identity(scale)+scalar_identity(offset))
                if ref.startswith('origin('):
                    anchor=digest('physical/named-reference-origin@1',anchor+name(ref[7:-1]))
            body+=anchor;capsule=body+digest('physical/unit-definition@1',body)
            if len(capsule)!=768:raise ValueError('unexpected codec width')
            units[symbol]={'scale':scale,'offset':offset,'anchor':anchor,'capsule':capsule};bindings[binding]={'symbol':symbol,'capsule':capsule.hex()};pending.remove(item)
        if len(pending)==count:raise ValueError('cyclic or unresolved unit references')
    for binding,symbol in synthesized:
        candidates=[]
        for group,prefix_symbol,exponent,alias in prefixes:
            if alias is not None:continue
            if not symbol.startswith(prefix_symbol):continue
            base=symbol[len(prefix_symbol):]
            if base not in units:continue
            previous=units[base];capsule=previous['capsule'];pos=1+493+129+48
            dm=int.from_bytes(capsule[pos+4:pos+12],'little');bm=int.from_bytes(capsule[pos+12:pos+28],'little')
            enabled=(dm & (1 << (-exponent-1 if exponent<0 else exponent+29))) if group=='si' else (bm & (1 << (exponent-1)))
            if not enabled:continue
            body=bytearray(capsule[:-32]);body[1+493:1+493+129]=name(symbol);body[pos+29]=1 if group=='si' else 2;body[pos+30]=exponent&255
            derived=bytes(body)+digest('physical/unit-definition@1',body);candidates.append(derived)
        if len(candidates)!=1:raise ValueError('ambiguous or unresolved generated binding: '+symbol)
        bindings[binding]={'symbol':symbol,'capsule':candidates[0].hex()}
    defaults={type_name:binding for binding,symbol,type_name,fields in raw_units if fields['reference']=='origin'}
    artifact={'default_units':dict(sorted(defaults.items())),'source_sha256':hashlib.sha256(source.encode()).hexdigest(),'unit_encoded_len':768,'quantity_encoded_len':788,'families':{k:v.hex() for k,v in sorted(family_caps.items())},'prefixes':[{'group':g,'symbol':s,'exponent':e,'alias':a} for g,s,e,a in prefixes],'units':dict(sorted(bindings.items()))}
    return json.dumps(artifact,ensure_ascii=False,indent=2)+'\n'

def rust_artifact(encoded):
    data=json.loads(encoded);lines=['//! Generated from definitions/physical.conduit; do not edit.','use super::*;','pub const BUILTIN_PHYSICAL_SOURCE: &str = include_str!("../../definitions/physical.conduit");']
    identifiers=[]
    for binding,item in data['units'].items():
        identifier='BUILTIN_'+re.sub(r'(?<!^)(?=[A-Z])','_',binding).upper();identifiers.append((identifier,item['symbol']))
        lines+=['#[rustfmt::skip]',f'pub const {identifier}: UnitDefinition = UnitDefinition::from_generated(&['+','.join(str(x) for x in bytes.fromhex(item['capsule']))+']);']
    lines+=['#[rustfmt::skip]','pub const BUILTIN_UNIT_DEFINITIONS: &[(&str, UnitDefinition)] = &[']+[f'({json.dumps(symbol,ensure_ascii=False)}, {identifier}),' for identifier,symbol in identifiers]+['];']
    default_entries=[]
    for family_name,binding in data['default_units'].items():
        identifier='BUILTIN_'+re.sub(r'(?<!^)(?=[A-Z])','_',family_name).upper()+'_DEFAULT_UNIT'
        unit_identifier='BUILTIN_'+re.sub(r'(?<!^)(?=[A-Z])','_',binding).upper()
        lines += ['#[rustfmt::skip]',f'pub const {identifier}: UnitDefinition = {unit_identifier};']
        default_entries.append((family_name,identifier))
    lines += ['#[rustfmt::skip]','pub const BUILTIN_DEFAULT_UNIT_DEFINITIONS: &[(&str, UnitDefinition)] = &[']+[f'({json.dumps(family_name)}, {identifier}),' for family_name,identifier in default_entries]+['];']
    role_entries=[]
    for family_name,capsule_hex in data['families'].items():
        capsule=bytes.fromhex(capsule_hex);family_hash=capsule[-32:].hex();point=bool(capsule[-33]);role='point' if point else 'linear';identifier='BUILTIN_'+re.sub(r'(?<!^)(?=[A-Z])','_',family_name).upper()+'_INFO_ID'
        role_entries.append((family_name,identifier));lines.extend(['#[rustfmt::skip]','pub const '+identifier+': &str = '+json.dumps('quantity/role/'+family_hash+'/'+role+'@1')+';'])
        if point:
            delta_start=1+129+265;length=capsule[delta_start];delta_name=capsule[delta_start+1:delta_start+1+length].decode();identifier='BUILTIN_'+re.sub(r'(?<!^)(?=[A-Z])','_',delta_name).upper()+'_INFO_ID';role_entries.append((delta_name,identifier));lines.extend(['#[rustfmt::skip]','pub const '+identifier+': &str = '+json.dumps('quantity/role/'+family_hash+'/delta@1')+';'])
    lines+=['#[rustfmt::skip]','pub const BUILTIN_QUANTITY_ROLE_INFO_IDS: &[(&str,&str)] = &[']+[f'({json.dumps(name)}, {identifier}),' for name,identifier in role_entries]+['];']
    lines+=['#[rustfmt::skip]','pub const BUILTIN_PREFIX_DEFINITIONS: &[(&str, &str, i8, Option<&str>)] = &[']
    for prefix in data['prefixes']:
        alias='Some('+json.dumps(prefix['alias'],ensure_ascii=False)+')' if prefix['alias'] else 'None'
        lines.append('('+json.dumps(prefix['group'])+','+json.dumps(prefix['symbol'],ensure_ascii=False)+','+str(prefix['exponent'])+','+alias+'),')
    lines+= ['];']
    return '\n'.join(lines)+'\n'

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--check',action='store_true');args=parser.parse_args();expected=regenerate();rust=rust_artifact(expected);rust_path=HERE.parent/'src/physical_definition/generated.rs'
    if args.check:
        if not ARTIFACT.exists() or ARTIFACT.read_text()!=expected or not rust_path.exists() or rust_path.read_text()!=rust:raise SystemExit('physical definition bootstrap artifact differs')
    else:ARTIFACT.write_text(expected);rust_path.write_text(rust)
if __name__=='__main__':main()
