#!/usr/bin/env python3
"""TRAIN-only candidate dictionary; JSON material is not Native admission."""
import collections, hashlib, json, pathlib, sys
ROOT = pathlib.Path(__file__).resolve().parents[2]
TRAINING = ROOT / 'semantics/language/training'
sys.path.insert(0, str(TRAINING))
from window8_reference import read
from train_ewt import POS
source = pathlib.Path(sys.argv[1]); destination = pathlib.Path(sys.argv[2])
expected = 'd68e06122a702464c613076523d56740f047e5bbe89dd90ec32737e04d952143'
data = source.read_bytes()
if hashlib.sha256(data).hexdigest() != expected: raise SystemExit('TRAIN digest mismatch')
rows, skipped = read(source)
all_pos = collections.defaultdict(collections.Counter)
bounded_pos = collections.defaultdict(collections.Counter)
for line in data.decode().splitlines():
    f = line.split('\t')
    if len(f) == 10 and f[0].isdigit() and f[3] in POS: all_pos[f[1]][POS.index(f[3])] += 1
for row in rows:
    for form, code in zip(row['forms'], row['pos']): bounded_pos[form][code] += 1
surfaces = sorted(bounded_pos)
remaining = sorted((s for s in all_pos if s not in bounded_pos), key=lambda s:(-sum(all_pos[s].values()), s))
surfaces += remaining[:8192-len(surfaces)]
assert len(surfaces) == 8192
entries=[]; omissions=[]
for form in sorted(surfaces):
    selected = sorted(bounded_pos.get(form, {}), key=lambda c:(-bounded_pos[form][c],c))[:4]
    selected += [c for c in sorted(all_pos[form], key=lambda c:(-all_pos[form][c],c)) if c not in selected][:4-len(selected)]
    entries.append({'surface':form,'pos_codes':selected,'origin':'reviewed TRAIN dictionary candidate'})
    omitted = sorted(set(all_pos[form])-set(selected))
    if omitted: omissions.append({'surface':form,'omitted_pos_codes':omitted,'full_train_counts':dict(all_pos[form])})
destination.mkdir(parents=True, exist_ok=False)
shards=[]
for i in range(128):
    content=json.dumps({'language':'language/en','entries':entries[i*64:(i+1)*64]},ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()
    name=f'shard-{i:03}.json'; (destination/name).write_bytes(content)
    shards.append({'path':name,'sha256':hashlib.sha256(content).hexdigest(),'entries':64})
manifest={'scope':'TRAIN-only finite candidate export; not canonical Native shards, no learned model or acceptance proof','train_sha256':expected,'policy':'all bounded TRAIN forms, then full TRAIN frequency; bounded POS frequency/code then full TRAIN frequency/code; max4','bounded_train_rows':len(rows),'bounded_train_forms':len(bounded_pos),'skipped':skipped,'entries':len(entries),'shards':shards,'omitted_alternatives':omissions,'unknown_policy':'not selected by this dictionary export','script_sha256':hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest()}
(destination/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({'entries':len(entries),'shards':len(shards),'bounded_train_forms':len(bounded_pos),'omitted_forms':len(omissions)}))
