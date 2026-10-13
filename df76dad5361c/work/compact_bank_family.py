from pathlib import Path
import re,hashlib
src=Path('work/bank-family-generated.rs').read_text()
out=Path('work/bank-family-compiled');out.mkdir(exist_ok=True)
count=0;unique={}
def replace(match):
    global count
    vals=[v.strip() for v in match.group(1).split(',') if v.strip()]
    data=bytes(int(v,16) if v.startswith('0x') else int(v) for v in vals)
    digest=hashlib.sha256(data).hexdigest();name=digest+'.bin'
    if digest not in unique:
        (out/name).write_bytes(data);unique[digest]=len(data)
    count+=1
    return 'include_bytes!("'+str((out/name).resolve())+'")'
src=re.sub(r'&\[((?:(?:0x[0-9a-fA-F]+|\d+)\s*,?\s*)+)\]',replace,src)
(out/'semantic_types.rs').write_text(src)
print(count,'arrays;',len(unique),'unique;',sum(unique.values()),'bytes;',len(src),'source bytes')
