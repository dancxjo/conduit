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
assert receipt['schema'].endswith('@4')
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
capture=receipt['stale_show_capture']; guest=capture['guest']; png=target/capture['capture']['png']
assert guest['refusal']=='StaleShow' and guest['same_face'] and guest['same_draft'] and guest['current_show_preserved']
assert guest['old_show_id']!=guest['current_show_id'] and guest['owner_actions']==0 and guest['keyboard_available']
assert guest['mask_plan_id'] and guest['active_play_id']
assert guest['notice']=='Stale: refresh' and guest['pixels_written']>0
assert guest['width']==capture['capture']['width'] and guest['height']==capture['capture']['height']
assert capture['checkpoint_ack']=='real-ps2-enter-press-release-after-capture'
assert all(receipt['observations'][key] for key in ['initial_show_acknowledged','root_copy_failure','old_submit_refused','stale_input_preserves_semantic_revision','fresh_service_instance','no_show_or_keyboard_before_present','atomic_present_acknowledgement','fresh_show_and_play','held_input_refused_by_fresh_show','fresh_interaction_correlated','fresh_interaction_advances_semantics'])
assert capture['capture']['png_sha256']==sha(png)
assert capture['qmp_transcript_sha256']==sha(target/'stale-capture-qmp.log')
assert serial.count(b'CONDUIT_COMPOSITOR_STALE_CAPTURE_ACK\n')==1
print(json.dumps({'source_identity':source,'outer_command_exit':0,'guest_exit_required_by_collector':33,'iso_sha256':sha(target/'conduitos.iso'),'elf_sha256':sha(target/'conduitos'),'serial_sha256':hashlib.sha256(serial).hexdigest(),'scene_raster_pixels_written':scene['raster_pixels_written'],'surface_recompose_pixels_written':surfaces['recompose_pixels_written'],'ninth_guest_entries_before':surfaces['guest_entries_before_ninth'],'ninth_guest_entries_after':surfaces['guest_entries_after_ninth'],'independent_checks':'source, clean checkout, actual ELF and ISO bytes, unique complete serial receipts, Make binding and unchanged 256 MiB budget, actual capacity counters, semantic custody, explicit nonclaims','stale_show':guest,'capture':capture['capture'],'capture_acknowledgement':capture['checkpoint_ack'],'fixture':True,'ordinary_product':False},indent=2))
