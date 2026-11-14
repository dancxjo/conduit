"""Read-only topology/stock opportunity measurement, never a lifetime certificate."""
from pathlib import Path
import ast,hashlib,json,re,collections
root=Path(__file__).resolve().parent.parent
p=root/'work/cache-backing/rp2040-source-fit-20261113/thumbv6m-none-eabi/release/build/conduit-rp2040-host-c79668900a21695b/out/workset_0_layout.rs'
s=p.read_text();data=bytes(ast.literal_eval(re.search(r'pub const PROJECTION_TEMPLATE:&\[u8\]=&(\[[^;]+\]);',s).group(1)))
assert data[:8]==b'CNDAP002' and int.from_bytes(data[10:12],'little')==len(data)
u16=lambda b,o:int.from_bytes(b[o:o+2],'little')
u32=lambda b,o:int.from_bytes(b[o:o+4],'little')
records=[];cursor=125
while cursor<len(data):
 tag=data[cursor];n=u16(data,cursor+1);payload=data[cursor+3:cursor+3+n];assert len(payload)==n;records.append((tag,payload));cursor+=3+n
assert cursor==len(data)
section=s.split('pub const CORD_VALUE_PROFILES:',1)[1].split('];',1)[0]
profiles=[]
for line in section.splitlines():
 if m:=re.search(r'::Canonical\((\d+)\)',line):profiles.append(('canonical',None,int(m[1])))
 elif m:=re.search(r'::Schema\{canonical:VALUE_SCHEMA_(\d+),maximum_bytes:(\d+)\}',line):profiles.append(('schema',int(m[1]),int(m[2])))
schemas={int(k):bytes(ast.literal_eval(v)) for k,v in re.findall(r'static VALUE_SCHEMA_(\d+):&\[u8\]=&(\[[^;]+\]);',s)}
cords=[];slot=0
for tag,b in records:
 if tag!=4:continue
 assert len(b)==42 and u16(b,0)==len(cords) and u16(b,28)==slot
 count=u16(b,30);stock=profiles[slot:slot+count];assert len(stock)==count and all(x==stock[0] for x in stock)
 cords.append({'cord':u16(b,0),'source':tuple(b[18:23]),'local_source':b[18]==0,'node':u16(b,19),'port':u16(b,21),'local_sink':b[23]==0,'capacity':count,'track':b[36],'pressure':b[41],'profile':stock[0]});slot+=count
assert slot==len(profiles)==97 and len(cords)==95
groups=collections.defaultdict(list)
for c in cords:groups[c['source']].append(c)
opportunities=[]
for siblings in groups.values():
 if len(siblings)<2:continue
 c=siblings[0];rep,schema,maximum=c['profile']
 if not(c['local_source'] and rep=='schema' and all(x['local_sink'] and x['track']==0 and x['pressure']==0 and x['profile']==c['profile'] for x in siblings)):continue
 old=sum(x['capacity'] for x in siblings);proposed=max(x['capacity'] for x in siblings);node_bytes=maximum-len(schemas[schema]);assert node_bytes>=0
 opportunities.append({'source_node':c['node'],'source_port':c['port'],'cords':[x['cord'] for x in siblings],'schema_index':schema,'full_maximum_bytes':maximum,'node_bytes':node_bytes,'original_queue_slots':old,'candidate_distinct_value_queue_stock':proposed,'duplicate_slot_reservations':old-proposed,'per_group_payload_estimate_bytes':(old-proposed)*node_bytes})
result={'assurance':'Read-only candidate opportunity, NOT a proved stock bound or measured runtime saving. No queue/resource/logical identity changes.','layout_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'projection_sha256':hashlib.sha256(data).hexdigest(),'cords':len(cords),'queue_slots':len(profiles),'schema_queue_slots':sum(x[0]=='schema' for x in profiles),'eligible_fanout_groups':len(opportunities),'candidate_duplicate_queue_reservations':sum(x['duplicate_slot_reservations'] for x in opportunities),'per_group_payload_estimate_bytes':sum(x['per_group_payload_estimate_bytes'] for x in opportunities),'groups':opportunities,'guards':['same local source output','all siblings included','schema/node profile identical across siblings','all sinks local','payload track only','PreserveOrder only'],'unproved':['suffix/lifetime invariant through commit/rollback/errors/cancellation and rearm','actual class-wide maximum capacity and retained-owner/native pin interaction','complete factory CPU/target measurements and full application fit'],'production_edits':False,'builds_run':False}
(root/'outputs/source-fanout-opportunity-20261114.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='groups'}))
