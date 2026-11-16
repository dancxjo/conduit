import json,os,subprocess,re,hashlib
from pathlib import Path
root=Path(__file__).resolve().parent.parent
old=root/'work/arm-unresolved-stock-geometry-20261115'
out=root/'work/arm-cord-terminal-geometry-20261115';out.mkdir(exist_ok=True)
cache=root/'work/cache-backing/rp2040-source-fit-20261113'
outputs=list(cache.glob('thumbv6m-none-eabi/release/build/conduit-rp2040-host-*/out/workset_0_layout.rs'))
selected=[p.parent for p in outputs if 'pub const ABNORMAL_CORD_COUNT:usize=0;' in p.read_text() and 'pub const EXTERNAL_INGRESS_CORD_COUNT:usize=3;' in p.read_text()]
assert len(selected)==1,selected
source=(old/'probe.rs').read_text().replace('conduit-rp2040-host-c79668900a21695b/out',selected[0].parent.name+'/out')
source+="\ntype FullCordTerminalKernel=conduit_mcu_host_runtime::execution::ExecutionKernel<conduit_mcu_host_runtime::execution_factory::Driver<'static,resident::generated::BorrowedBack<'static>>,N,C,P,R,T,H,S,conduit_kernel::HostedSignLog,K,neutral_factory::Values,Q,0,0,0,C>;\n#[no_mangle]pub static FULL_CORD_TERMINAL_KERNEL_BYTES:[u8;core::mem::size_of::<FullCordTerminalKernel>()]=[0;core::mem::size_of::<FullCordTerminalKernel>()];\n#[no_mangle]pub static SELECTED_CORD_TERMINALS:[u8;neutral_workset::CORD_TERMINALS]=[0;neutral_workset::CORD_TERMINALS];\n"
(out/'probe.rs').write_text(source)
cmd=json.loads((old/'command.json').read_text())
for i,a in enumerate(cmd):
 if a.endswith('/probe.rs'):cmd[i]=str(out/'probe.rs')
 elif a.endswith('/probe.ll'):cmd[i]=str(out/'probe.ll')
 elif '=/' in a and '.rlib' in a:
  name=a.split('=')[0];files=list((cache/'thumbv6m-none-eabi/release/deps').glob('lib'+name+'-*.rlib'));assert files,name
  cmd[i]=name+'='+str(max(files,key=lambda p:p.stat().st_mtime_ns))
(out/'command.json').write_text(json.dumps(cmd,indent=2)+'\n')
env=dict(os.environ,OUT_DIR=str(selected[0]))
with (out/'compiler.log').open('w') as log:result=subprocess.run(cmd,cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
assert result.returncode==0,(out/'compiler.log').read_text()[-3000:]
llvm=(out/'probe.ll').read_text();sizes={}
for line in llvm.splitlines():
 m=re.match(r'@([A-Z_0-9]+) = .*constant (.*)',line)
 if not m:continue
 n=re.search(r'\[(\d+) x i8\]',m[2]);sizes[m[1]]=int(n[1]) if n else 0
receipt={'assurance':'Actual ARM ABI compile only; not target runtime fit/admission/Start','sizes':sizes,'selected_generated_out':str(selected[0]),'selected_generated_layout_sha256':hashlib.sha256((selected[0]/'workset_0_layout.rs').read_bytes()).hexdigest(),'probe_sha256':hashlib.sha256(source.encode()).hexdigest(),'execution':False}
receipt['cord_stock_saving_same_code_bytes']=sizes['FULL_CORD_TERMINAL_KERNEL_BYTES']-sizes['NEUTRAL_KERNEL_BYTES']
receipt['previous_selected_kernel_bytes']=31536
receipt['saving_from_previous_selected_kernel_bytes']=31536-sizes['NEUTRAL_KERNEL_BYTES']
(root/'outputs/arm-cord-terminal-geometry-20261115.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt))
