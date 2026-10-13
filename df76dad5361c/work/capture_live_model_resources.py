import pathlib,os,time,json,hashlib
project=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907')
out=project/'outputs/native-admission-family/live-model-process-resources';out.mkdir(exist_ok=True)
processes={2898759:'original-authored',3407194:'dev-identity-retry-r3',3460414:'immutable-producer-profile-r1'}
hz=os.sysconf('SC_CLK_TCK');uptime=float(pathlib.Path('/proc/uptime').read_text().split()[0])
for pid,label in processes.items():
 p=pathlib.Path('/proc')/str(pid)
 if not p.exists():continue
 raw=p.joinpath('stat').read_text();a=raw.rsplit(')',1)[1].split();status=p.joinpath('status').read_text();fields=dict(line.split(':',1) for line in status.splitlines() if ':' in line)
 if a[0] == 'Z': continue
 exe=p.joinpath('exe').resolve();cmd=p.joinpath('cmdline').read_bytes().replace(b'\0',b' ').decode();start=int(a[19]);user=int(a[11]);system=int(a[12]);hwm=fields.get('VmHWM','').strip()
 prior=out/f'{label}-{pid}-latest.json'
 if prior.exists() and json.loads(prior.read_text())['start_ticks_since_boot'] != start: continue
 r={'label':label,'pid':pid,'captured_unix_ns':time.time_ns(),'executable':str(exe),'executable_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'command':cmd,'state':a[0],'start_ticks_since_boot':start,'clock_ticks_per_second':hz,'elapsed_seconds_through_snapshot':uptime-start/hz,'cpu_user_seconds_through_snapshot':user/hz,'cpu_system_seconds_through_snapshot':system/hz,'cpu_total_seconds_through_snapshot':(user+system)/hz,'vm_hwm':hwm,'vm_hwm_bytes':int(hwm.split()[0])*1024 if hwm else None,'vm_rss':fields.get('VmRSS','').strip(),'threads':fields.get('Threads','').strip(),'scope':'Whole exact model test process lifetime through this live snapshot: includes cold Source/model/Native preparation and runtime, code/shared mappings; excludes Cargo/compiler/parent orchestration and other processes. VmHWM is actual Linux peak resident memory through capture, not final future peak or allocation ceiling. CPU from whole process /proc stat; elapsed since process start. Original selected pre-Core-repair SDK; no retroactive generic bound claim.','proc_stat':raw,'proc_status':status}
 (out/f'{label}-{pid}-latest.json').write_text(json.dumps(r,indent=2)+'\n')
 with (out/f'{label}-{pid}-samples.jsonl').open('a') as log: log.write(json.dumps({k:r[k] for k in ['captured_unix_ns','start_ticks_since_boot','elapsed_seconds_through_snapshot','cpu_total_seconds_through_snapshot','vm_hwm_bytes','state']})+'\n')
