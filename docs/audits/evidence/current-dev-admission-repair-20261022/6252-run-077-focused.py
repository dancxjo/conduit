import os,json,subprocess,time
from pathlib import Path
base=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit')
root=base/'work/6252-native-fixtures'
source='077a2a535b4ec30d1f09ec6c2884c22bbffd1c57'
env=os.environ.copy();env.pop('RUST_MIN_STACK',None)
env.update(CARGO_TARGET_DIR='/home/dancxjo/Documents/Codex/2026-10-21/goal-find-the-first-available-github/work/lint-target',CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
tests=[('cycles','native_face_scene::connected_layout::tests::cycles_and_self_links_preserve_every_declared_cord_and_bounded_pages'),('large-graph','native_face_scene::ports::tests::large_graphs_keep_every_identity_reachable_and_report_omitted_geometry'),('choices','native_face_scene::tests::primary_action_names_bounded_text_choices_from_current_face_contract'),('cold','native_workset::preparation::tests::every_native_source_reuses_exact_cold_meaning_with_finite_entries'),('release','product_front_door::workspace_input::tests::tab_and_captured_release_preserve_two_plot_state_and_the_same_play'),('service-release','product_front_door::workspace_input::service_tests::held_release_after_selection_is_observed_without_repainting_the_new_plot')]
native_tests=tests
tests=[(label,name,['cargo','+stable','test','--locked','-p','conduitos','--lib','--features','native-compositor',name,'--','--exact']) for label,name in native_tests]
for label,name in [('membership','installed_browser::membership_offer::tests::membership_preserves_every_installed_offer_within_admission_bounds'),('prebirth-cancel','owner_face_mask::prebirth_tests::cancelling_prebirth_mask_refuses_late_host_completion_before_show_delivery')]:
 tests.append((label,name,['cargo','+stable','test','--locked','-p','conduit-browser-runtime','--lib',name,'--','--exact']))
name='wrong_protocol_and_oversized_claim_collections_do_not_enter_inventory'
tests.append(('candidate-boundary',name,['cargo','+stable','test','--locked','-p','conduit-body','--test','candidate',name,'--','--exact']))
rows=[]
for label,name,cmd in tests:
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True)
 log=base/f'work/6252-077a2a535-{label}.log';start=time.time()
 with log.open('w') as f:r=subprocess.run(cmd,cwd=root,env=env,stdout=f,stderr=subprocess.STDOUT)
 text=log.read_text();passed=r.returncode==0 and '1 passed; 0 failed' in text and ('test '+name+' ... ok') in text
 rows.append(dict(label=label,test=name,command=cmd,exit_code=r.returncode,pass_verified=passed,log=str(log),seconds=time.time()-start))
 (base/'work/6252-077a2a535-focused-checks.json').write_text(json.dumps(dict(source=source,checks=rows),indent=2)+'\n')
 print(label,r.returncode,passed,flush=True)
 if not passed:raise SystemExit(r.returncode or 2)
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True)
