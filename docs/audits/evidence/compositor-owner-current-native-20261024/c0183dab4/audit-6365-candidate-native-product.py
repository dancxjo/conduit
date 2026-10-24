import pathlib,json,hashlib,struct,zlib,io,re
from PIL import Image
r=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit');p=r/'outputs/x86-product-40060819187/conduitos-x86_64';source='c0183dab41b13c80aa7eb8f4c5c94eba08654386'
def sha(b):return hashlib.sha256(b).hexdigest()
def read(n):return (p/n).read_bytes()
def load(n):return json.loads(read(n))
receipt=load('receipt.json');manifest=load('build-manifest.json');resolved=load('resolved-image.json');proof=load('evidence/x86_64-product-journey-proof.json');frames=load('evidence/journey-frames/manifest.json');download=json.loads((r/'outputs/x86-product-40060819187-download.json').read_text());assert sha((r/download['archive']).read_bytes())==download['archive_sha256'];assert receipt['sha']==manifest['source_identity']==proof['source_commit']==frames['context']['source_commit']==source
verified=[]
for f in receipt['files']:
 b=read(f['path']);assert len(b)==f['size'] and sha(b)==f['sha256'];verified.append(f)
assert len(verified)==49;kernel=read(manifest['kernel']['file']);iso=read(manifest['image']['file']);assert sha(kernel)==manifest['kernel']['sha256'];assert sha(iso)==manifest['image']['sha256']==proof['image_sha256'];assert manifest['resolved_build']==resolved['manifest']
life=proof['compositor_lifecycle'];assert life['artifact_custody']['unique_loaded_compositor_image_bytes_verified'] is True;serial=read('evidence/journey-serial.log');assert sha(serial)==life['serial_sha256'];assert sha(kernel)==life['root_elf_sha256'];assert sha(iso)==life['image_sha256'];assert life['artifact_custody']['actual_root_and_iso_hashes_verified'] and life['artifact_custody']['normal_manifest_and_boot_join_verified'];assert frames['status']=='complete' and frames['failure'] is None
# Independently parse ELF load ranges, complete section extent and symbol identity.
def n(b,o,w):assert 0<=o<=len(b)-w;return int.from_bytes(b[o:o+w],'little')
def loads(b):
 assert b[:7]==b'\x7fELF\x02\x01\x01' and n(b,16,2)==2 and n(b,18,2)==62 and n(b,52,2)==64
 off,count=n(b,32,8),n(b,56,2);assert n(b,54,2)==56 and 0<count<=64;out=[]
 for i in range(count):
  q=off+56*i
  if n(b,q,4)!=1:continue
  start,size,mem,address,flags=n(b,q+8,8),n(b,q+32,8),n(b,q+40,8),n(b,q+16,8),n(b,q+4,4);assert size<=mem and start+size<=len(b);out.append((start,size,address,mem,flags))
 assert out;return out
rootloads=loads(kernel);matches=[];at=0
while True:
 at=kernel.find(b'\x7fELF\x02\x01\x01',at)
 if at<0:break
 if sha(kernel[at:at+life['guest_elf_bytes']])==life['guest_image_sha256']:matches.append(at)
 at+=1
assert len(matches)>=1;at=matches[0];guest=kernel[at:at+life['guest_elf_bytes']];assert all(kernel[o:o+len(guest)]==guest for o in matches);assert all(any(o>=st and o+len(guest)<=st+sz for st,sz,*_ in rootloads) for o in matches);assert any(at>=s and at+len(guest)<=s+size for s,size,*_ in rootloads);gl=loads(guest);entry=n(guest,24,8);assert all(flags in [4,5] for _,_,_,_,flags in gl);assert any(flags==5 and addr<=entry<addr+mem for _,_,addr,mem,flags in gl)
off,count=n(guest,40,8),n(guest,60,2);assert n(guest,58,2)==64 and 0<count<=4096;extent=max(off+count*64,n(guest,32,8)+n(guest,56,2)*56,64);symbols=[]
for i in range(count):
 q=off+i*64;kind,start,size=n(guest,q+4,4),n(guest,q+24,8),n(guest,q+32,8)
 if kind!=8:extent=max(extent,start+size);assert start+size<=len(guest)
 if kind!=2:continue
 assert n(guest,q+56,8)==24 and size%24==0;link=n(guest,q+40,4);assert link<count;st=off+link*64;assert n(guest,st+4,4)==3;strings=guest[n(guest,st+24,8):n(guest,st+24,8)+n(guest,st+32,8)]
 for x in range(start,start+size,24):
  name=strings[n(guest,x,4):].split(b'\0',1)[0]
  if name==b'compositor_entry':assert n(guest,x+4,1)==18 and n(guest,x+6,2)!=0 and n(guest,x+8,8)==entry;symbols.append(entry)
assert len(symbols)==1 and extent==len(guest)
# Classify every ELF-magic candidate, not just matches of the reported digest.
def candidate_extent(b):
 ll=loads(b);en=n(b,24,8)
 assert all(fl in [4,5] for _,_,_,_,fl in ll) and any(fl==5 and a<=en<a+m for _,_,a,m,fl in ll)
 o,c=n(b,40,8),n(b,60,2);assert n(b,58,2)==64 and 0<c<=4096
 ext=max(o+c*64,n(b,32,8)+n(b,56,2)*56,64);assert ext<=len(b);found=0
 for i in range(c):
  q=o+i*64;kind,st,sz=n(b,q+4,4),n(b,q+24,8),n(b,q+32,8)
  if kind!=8:assert st+sz<=len(b);ext=max(ext,st+sz)
  if kind!=2:continue
  assert n(b,q+56,8)==24 and sz%24==0 and sz//24<=100000
  link=n(b,q+40,4);assert link<c;t=o+link*64;assert n(b,t+4,4)==3
  a,z=n(b,t+24,8),n(b,t+32,8);assert a+z<=len(b);text=b[a:a+z]
  for x in range(st,st+sz,24):
   name=n(b,x,4);assert name<len(text);tail=text[name:];assert b'\0' in tail
   if tail.split(b'\0',1)[0]==b'compositor_entry':assert n(b,x+4,1)==18 and n(b,x+6,2)!=0 and n(b,x+8,8)==en;found+=1
 assert found==1 and ext<=64*1024*1024
 for st,sz,*_ in ll:ext=max(ext,st+sz)
 return ext
valid=[];pos=0
while True:
 pos=kernel.find(b'\x7fELF\x02\x01\x01',pos)
 if pos<0:break
 try:
  size=candidate_extent(kernel[pos:]);assert any(pos>=st and pos+size<=st+sz for st,sz,*_ in rootloads)
 except (AssertionError,IndexError,ValueError):size=None
 if size is not None:
  assert kernel[pos:pos+size]==guest;valid.append({'offset':pos,'bytes':size,'sha256':sha(kernel[pos:pos+size])})
 pos+=1
assert [v['offset'] for v in valid]==matches
lines=serial.splitlines();lifecycle=[json.loads(l.split(b' ',1)[1]) for l in lines if l.startswith(b'CONDUIT_COMPOSITOR_LIFECYCLE ')];assert len(lifecycle)==life['lifecycle_records']==23;assert life['snapshots']==lifecycle
acks=[]
for selected in life['selected_checkpoints']:
 ack=selected['ack']
 if ack is None:continue
 expected={'service_instance':ack['instance'],'namespace':ack['namespace'],'frame_sequence':ack['frame_sequence'],'guest_sequence':ack['guest_sequence']};found=[]
 for l in lines:
  if not l.startswith(b'CONDUIT_COMPOSITOR_SERVICE_FRAME '):continue
  pairs={k.decode():int(v) for k,v in re.findall(rb'(\w+)=(\d+)',l)}
  if all(pairs[k]==v for k,v in expected.items()):found.append(pairs)
 assert len(found)==1;acks.append({'checkpoint':selected['checkpoint'],'ack':expected,'record':found[0]})
assert len(acks)==3
captures=[]
for row in frames['checkpoints']:
 f=row['frame'];b=read('evidence/journey-frames/'+f['png']);assert len(b)==f['png_bytes'] and sha(b)==f['png_sha256'];assert b[:8]==b'\x89PNG\r\n\x1a\n';o=8
 while o<len(b):
  length=int.from_bytes(b[o:o+4],'big');kind=b[o+4:o+8];data=b[o+8:o+8+length];crc=int.from_bytes(b[o+8+length:o+12+length],'big');assert zlib.crc32(kind+data)&0xffffffff==crc;o+=length+12
 assert o==len(b);img=Image.open(io.BytesIO(b)).convert('RGBA');assert img.size==(f['width'],f['height']) and sha(img.tobytes())==f['pixel_sha256'];end=row['serial_byte_end'];assert 0<end<=len(serial) and serial[end-1:end]==b'\n';prefix=serial[:end]
 boot=row['guest_boot_record'];assert boot['build_id']==manifest['build_id'] and boot['profile_id']==manifest['profile_id'] and boot['image_binding']==manifest['resolved_description_binding'];assert boot['host_id']==proof['host_id'] and boot['boot_id']==proof['boot_id'];assert boot in [json.loads(l.split(b' ',1)[1]) for l in prefix.splitlines() if l.startswith(b'CONDUIT_BOOT_SIGN ')]
 # Each recorded semantic observation must already exist in its exact serial prefix.
 records=[]
 for l in prefix.splitlines():
  if b' {' in l:
   try:records.append(json.loads(l.split(b' ',1)[1]))
   except (ValueError,IndexError):pass
 assert row['guest_record'] in records
 nr=row['native_receipts'];lastface=None;lastaction=None
 for line in prefix.splitlines():
  if line.startswith(b'CONDUIT_NATIVE_FACE '):
   rec=json.loads(line.split(b' ',1)[1]);lastface={'face_id':rec['face']['identity'],'face_revision':rec['face']['revision'],'basis':rec['face']['basis'],'context':rec['face']['interaction_context'],'show':rec['show']}
  if line.startswith((b'CONDUIT_NATIVE_ACTION ',b'CONDUIT_NATIVE_BIRTH ')):lastaction=json.loads(line.split(b' ',1)[1])
 assert nr['latest_acknowledged_face']==lastface and nr['latest_action']==lastaction
 captures.append({'checkpoint':row['checkpoint'],'png':'outputs/x86-product-40060819187/conduitos-x86_64/evidence/journey-frames/'+f['png'],'sha256':f['png_sha256'],'rgba_sha256':f['pixel_sha256'],'width':f['width'],'height':f['height'],'serial_byte_end':end})
assert len(captures)==27
custody=json.loads((r/'outputs/6365-actual-admission-proof-scope.json').read_bytes());identity=custody['quick_identity'];assert identity['tested_preview']==source and identity['head']=='295a7170ba0e4e952e281597817db5f2d9e6ad04';assert download['successful_x86_job']==120774171371 and download['full_gate_success'] is False
import subprocess
repo=r/'work/6267-review'
def git(*args):return subprocess.check_output(['git',*args],cwd=repo,text=True).strip()
assert git('rev-parse',source+'^{tree}')==identity['tested_tree'];feature=[]
for path in ['targets/conduitos/src/protected_compositor.rs','targets/conduitos/src/protected_compositor/request_owner.rs','targets/conduitos/src/protected_compositor/request_owner/tests.rs','targets/conduitos/tests/compositor_request_owner.rs']:
 blobs={label:git('rev-parse',ref+':'+path)for label,ref in [('submitted','295a7170ba0e4e952e281597817db5f2d9e6ad04'),('actual_executed',source),('root_B_unexecuted','ef5bba08ec65469a332c463b94d3a2be6210a841')]};assert len(set(blobs.values()))==1;feature.append({'path':path,**blobs})
assert len(proof['steps'])==24
result={'four_feature_blob_correspondence':feature,'only_completed_x86_job':120774171371,'whole_source_tree_equals_preview':False,'root_B_execution_claimed':False,'selected_showable_frames':[c for c in captures if c['checkpoint'] in ['patchbay-diagram','creche-ready']],'tree_bridge':identity,'source':source,'candidate':40060819187,'submitted_source':'295a7170ba0e4e952e281597817db5f2d9e6ad04','actual_executed_merge_preview':'c0183dab41b13c80aa7eb8f4c5c94eba08654386','full_candidate':'pending','dev_delivery':'pending','applicable_integration':'pending','artifact':12547519760,'mode':'Independent existingbytes audit only; no builds/network/newexecution.','archive_sha256':download['archive_sha256'],'receipt_files_all49_hash_size_verified':verified,'actual_root_sha256':sha(kernel),'actual_iso_sha256':sha(iso),'resolved_manifest_matches':True,'embedded_guest':{'offset':matches[0],'bytes':len(guest),'sha256':sha(guest),'occurrence_offsets':matches,'all_complete_duplicate_bytes_equal':True,'validated_loaded_candidates':valid,'actual_duplicate_copies_present':len(matches)>1,'complete_ELF_extent_load_permissions_entry_and_unique_defined_global_function_symbol_verified':True},'lifecycle_records_all23_exactrawserial':True,'ack_joins':acks,'frames_all27_crc_png_rgba_dimensions_prefix_and_boot_verified':captures,'normal_journey':{'status':frames['status'],'steps':proof['steps'],'host_id':proof['host_id'],'boot_id':proof['boot_id'],'body_id':proof['body_id'],'look_selection_status':proof['look_selection']['status'],'patchbay_actions_fields':list(proof['patchbay_actions']),'process_stop':frames['context']['process_stop']},'claims':'Successful sourcebound normalQEMUbaseline throughCrèche keyboard/pointer+Look switches,Birth/Wake/Plan/Play,protectedkeyboard/retainedtext,PatchbayFace/diagram/GearDetails/checkedsource/action/editrefusal/maskreplan,Stop/HomeReturn/reentry. RealQMP andexistingownerreceipts—notphysicalhuman/browsercomparison.','limits':'Actual6365mergepreviewcontainsNEWdeduplicatingcollectorandreceiptunique_loaded_compositor_image_bytes_verified. Thisestablishescurrentnativecollector/productcustodyatthisexactpreview,notfullCandidate/devdelivery/Integrationacceptance. One successfulPatchbayoperation doesnotestablishcausalboundedpublicationfixorfull6318closure. NoDMA/driverisolation,broadserviceconfinement,parent5356/human/physical/stableclaim. Requestowner11CPUtestsplusactualnormaljourneyestablishcurrentruntimepathbutnotselectedpolicy/cumulativeaccounting/fairness orfull6342/6317closure; RootBef5wasnotexecutedinthisrun. embeddedlifecyclepureparsercustodyfalse isseparatefromactualcollectorartifactcustodytrue.'}
(r/'outputs/6365-current-candidate-native-product-independent-review.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({'receipt_files':len(verified),'frames':len(captures),'lifecycle':len(lifecycle),'acks':len(acks),'guest_offset':at,'audit':'outputs/6365-current-candidate-native-product-independent-review.json'}))
