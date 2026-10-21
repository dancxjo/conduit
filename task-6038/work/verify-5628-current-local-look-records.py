from pathlib import Path
import json,hashlib
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');P=T/'outputs/5628-current-owner-journey/x86_64';raw=(P/'journey-serial.log').read_bytes();assert len(raw)<2*1024**2
records=[]
for index,line in enumerate(raw.decode().splitlines()):
 if line.startswith(('CONDUIT_NATIVE_FACE ','CONDUIT_NATIVE_COMPONENT_REALIZATION ','CONDUIT_CRECHE_CHANGE ')):
  tag,data=line.split(' ',1);records.append((index,tag,json.loads(data)))
r=json.loads((P/'journey-proof.json').read_text());transitions=r['look_selection']['transitions'];assert len(transitions)==4
faces={v['show']['show_id']:(i,v) for i,k,v in records if k=='CONDUIT_NATIVE_FACE'}
real=[(i,v) for i,k,v in records if k=='CONDUIT_NATIVE_COMPONENT_REALIZATION'];assert [v for _,v in real]==transitions
first,baseline=faces[transitions[0]['previous_show_id']];seen={baseline['show']['show_id']};previous=baseline['show']['show_id'];component=None;recipe=None
for i,t in real:
 assert t['previous_show_id']==previous and t['show_id'] not in seen;seen.add(t['show_id']);previous=t['show_id']
 fi,f=faces[t['show_id']];assert fi<i and f['face']==baseline['face'];assert t['face_id']==f['face']['identity'] and t['face_revision']==f['face']['revision']
 assert t['host_id']==r['host_id'] and t['boot_id']==r['boot_id'];assert t['local_scanout_acknowledged'] is True and t['owner_show_acknowledged'] is False;assert t['selection_entrance']=='local-creche' and t['fallback'] is None
 assert 0<t['render']['commands']<=48
 c=t['configuration']['component'];h=t['configuration']['recipe_canonical_sha256']
 if component is None:component=c;recipe=h
 assert c==component and h==recipe
 e=t['expression']['expression'];assert e['version']==2 and e['typography']=='editorial'
assert not [v for i,k,v in records if first<=i<=real[-1][0] and k=='CONDUIT_CRECHE_CHANGE']
result={'source':r['source_commit'],'raw_serial_sha256':hashlib.sha256(raw).hexdigest(),'independently_joined_local_transitions':4,'complete_faces_equal':True,'same_component_and_recipe':True,'fresh_linked_shows':True,'commands_within_48':True,'source_change_records_in_swap_interval':0,'typography':'Editorial on all four local roots; not independent Title-profile proof','owner_backed':False}
(P/'root-local-look-verification.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
