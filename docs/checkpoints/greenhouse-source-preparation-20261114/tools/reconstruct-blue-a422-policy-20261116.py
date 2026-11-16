from pathlib import Path
import subprocess,json
r=Path(__file__).resolve().parents[2];q=r/'work/source-composition-fore-qualification-20261116';deps=Path('/tmp/conduit-rp2040-three-lane-20261116/release/deps');out=r/'work/factory-tools/blue-a422-policy-reconstruction-20261116'
def lib(name):
 xs=list(deps.glob('lib'+name+'-*.rlib'));assert len(xs)==1,(name,len(xs));return xs[0]
def run(args,log):
 with (out/log).open('x') as f:p=subprocess.run(['rustup','run','1.98.1','rustc',*args],stdout=f,stderr=subprocess.STDOUT)
 print(json.dumps({'step':log,'exit':p.returncode}),flush=True);assert p.returncode==0
args=['--edition=2021','--crate-name','conduit_host_esp32_make','--crate-type','rlib',str(q/'targets/esp32/make/src/lib.rs'),'-L','dependency='+str(deps),'-o',str(out/'libconduit_host_esp32_make.rlib')]
for name in ['conduit_host_make','conduit_mcu_source_build','serde','serde_json','sha2']:args+=['--extern',name+'='+str(lib(name))]
run(args,'compile-esp32-make-export.log')
src=q/'targets/esp32/firmware/host/build.rs';dest=out/'build-script.rs';code=src.read_text()
needle='    let calibration = conduit_mcu_source_build::native_calibration::compile('
assert code.count(needle)==1
extra='''    let exported_bounds = conduit_mcu_source_build::native_calibration::Bounds::for_workset(
        &checked, &workset, conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: conduit_mcu_source_build::dht11_tm1637_resident_bounds().maximum_program_bytes,
            maximum_total_program_bytes: conduit_mcu_source_build::dht11_tm1637_resident_bounds().maximum_program_bytes,
        }).expect("same canonical parser bounds");
    fs::write(out.join("workset-policy.json"), exported_bounds.policy_json(1024 * 1024).to_string())
        .expect("export canonical immutable parser bounds");
'''
code=code.replace(needle,extra+needle);dest.write_text(code)
for name in ['heap_layout.rs','recording_build.rs']:(out/name).write_bytes((src.parent/name).read_bytes())
args=['--edition=2021',str(dest),'-L','dependency='+str(deps),'-L','dependency='+str(out),'-o',str(out/'reconstruct-policy')]
for name in ['conduit_resource_snapshot','conduit_mcu_host_runtime','conduit_core','conduit_pulse_capture','conduit_tm1637_wire','conduit_monotonic_wait','conduit_mcu_source_build','conduit_embedded_build','conduit_plot','conduit_host_make','sha2']:args+=['--extern',name+'='+str(lib(name))]
args+=['--extern','conduit_host_esp32_make='+str(out/'libconduit_host_esp32_make.rlib')]
run(args,'compile-build-script-export.log')
