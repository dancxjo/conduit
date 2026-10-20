"""One read-only normal-product diagnostic; chosen extent, not reported identity."""
from pathlib import Path
import importlib.util,json
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038')
def load(name,file):
 s=importlib.util.spec_from_file_location(name,T/'work'/file);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
b=load('panels','prove-5990-panels-qemu.py');p=load('pointer','prove-5990-pointer-qemu.py')
H='e29922753a2fca04145551bbb6ae00504fed76a9';out=T/'outputs/6158-e299-wide-diagnostic-2560x1600'
g=b.Guest(T/'outputs/5990-product-e29922753',H,out,2560,1600)
try:
 g.receipt.update(diagnostic_runner_sha256=b.digest(Path(__file__)),imported_panel_runner_sha256=b.digest(T/'work/prove-5990-panels-qemu.py'),imported_order_runner_sha256=b.digest(T/'work/prove-5990-pointer-qemu.py'),boundary='One chosen2560x1600extent; reported6158Source/image/guestextent unknown. Initial Source only, navigation and capture, no commits.')
 g.capture('arrival');initial=g.face();(out/'initial-face-show.json').write_text(json.dumps(initial,indent=2)+'\n');expected=p.expected_controls(initial['face']);start=(g.focus()['action_id'],g.focus()['argument_name']);observed=[]
 for _ in range(sum(1+len(a['arguments']) for a in initial['face']['actions'])):
  f=g.focus();assert f['show_id']==initial['show']['show_id'];observed.append(f);g.local('tab')
  if (g.focus()['action_id'],g.focus()['argument_name'])==start:break
 else:raise AssertionError('Source-bounded control cycle incomplete')
 actual=[(f['action_id'],f['argument_name']) for f in observed];i=expected.index(start);assert actual==expected[i:]+expected[:i] and len(actual)==12
 second=any(f['hit_bounds'] and f['hit_bounds']['x']>1280 for f in observed);assert second,'chosen wide extent expected configured columns'
 g.receipt.update(expected_source_control_order=expected,initial_focus_cycle=observed,observed_second_column_control=second,naming_controls=[f for f in observed if f['action_id']=='creche.naming'])
 g.capture_pages('initial-primary');assert g.face()==initial;assert not g.records('CONDUIT_CRECHE_CHANGE ')
 g.receipt.update(status='passed',source_changes=0);g.save()
except BaseException as e:
 g.receipt.update(status='failed',error=repr(e));g.save();raise
finally:g.close()
