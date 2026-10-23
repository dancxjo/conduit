import hashlib,json,subprocess,sys
from pathlib import Path
root=Path(sys.argv[1]); target=root/'target/conduitos/x86_64'
receipt=json.loads((target/'compositor-semantic-copy-recovery-fixture-proof.json').read_text())
serial=(target/'compositor-semantic-recovery-proof.log').read_bytes()
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True).strip()
assert receipt['build']['base_commit']==source
assert receipt['build']['elf_sha256']==sha(target/'conduitos')
assert receipt['transcript_sha256']==hashlib.sha256(serial).hexdigest()
assert receipt['schema'].endswith('@3')
assert receipt['build']['artifact_role']=='architecture-proof-appliance'
for prefix,key in [('CONDUIT_COMPOSITOR_PRESSURE_PROOF ','pressure_observations'),('CONDUIT_COMPOSITOR_SEMANTIC_RECOVERY_PROOF ','observations'),('CONDUIT_COMPOSITOR_SEMANTIC_RECOVERY_PROFILE ','admitted_profile')]:
    lines=[line[len(prefix):] for line in serial.decode().splitlines(keepends=True) if line.startswith(prefix)]
    assert len(lines)==1 and lines[0].endswith('\n')
    assert json.loads(lines[0])==receipt[key]
p=receipt['admitted_profile']; manifest=receipt['native_make_manifest']
assert p['runtime_arena_bytes']==p['runtime_arena_ceiling']==268435456
assert p['profile_id']==manifest['profile_id'] and p['build_id']==manifest['build_id'] and p['image_binding']==manifest['image_id']
c=receipt['pressure_observations']; scene=c['scene']; surfaces=c['surfaces']
assert scene['commands']==scene['raster_commands']==48 and scene['text_bytes_per_command']==192
assert scene['command49_refusal']=='TooManyCommands' and scene['payload193_refusal']=='PayloadTooLong'
assert scene['scene_unchanged_after_refusals'] and scene['raster_pixels_written']>0
assert surfaces['extra_admitted']==surfaces['extra_uploaded']==surfaces['extra_removed']==7
assert surfaces['mask_surfaces']==1 and surfaces['retained_total']==8
assert surfaces['guest_entries_before_ninth']==surfaces['guest_entries_after_ninth']>0
for field in ['same_session','same_service_instance','source_face_unchanged','show_unchanged','draft_revision_unchanged','keyboard_after_recompose']: assert surfaces[field]
assert surfaces['recompose_pixels_written']>0
assert not any([scene['diagnostic_show'],scene['protected_scene_interpretation'],surfaces['mask_plan_reserves_eight'],c['physical_scanout'],c['ordinary_product']])
print(json.dumps({'source_identity':source,'outer_command_exit':0,'guest_exit_required_by_collector':33,'iso_sha256':sha(target/'conduitos.iso'),'elf_sha256':sha(target/'conduitos'),'serial_sha256':hashlib.sha256(serial).hexdigest(),'scene_raster_pixels_written':scene['raster_pixels_written'],'surface_recompose_pixels_written':surfaces['recompose_pixels_written'],'ninth_guest_entries_before':surfaces['guest_entries_before_ninth'],'ninth_guest_entries_after':surfaces['guest_entries_after_ninth'],'independent_checks':'source, clean checkout, actual ELF and ISO bytes, unique complete serial receipts, Make binding and unchanged 256 MiB budget, actual capacity counters, semantic custody, explicit nonclaims','fixture':True,'ordinary_product':False},indent=2))
