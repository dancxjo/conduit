import { promises as fs } from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { CacheRefusal } from './refusal.mjs';
import { snapshotInventory, hashFile } from './cargo-snapshot.mjs';
import { HOSTED_SNAPSHOT_LIMITS } from './hosted-snapshot.mjs';
import { runSandbox } from './cargo-sandbox.mjs';
import { requireHostedHelloOutput } from '../pipeline/targets/hosted-hello.mjs';

const fail = code => { throw new CacheRefusal(code); };
const sha = value => createHash('sha256').update(value).digest('hex');

// Execute the existing canonical runtime proof independently of compilation.
// No engine, cache objects, network or mutable shared target enters this root.
export async function proveHostedHello({ snapshot, taskRoot }) {
  const started=performance.now();
  if(snapshot?.schema!=='hosted-linux-cargo-snapshot-v1'||!path.isAbsolute(taskRoot??''))fail('invalid-hosted-hello-policy');
  const root=await fs.realpath(snapshot.root),task=await fs.realpath(taskRoot);
  if(root===task||task.startsWith(root+'/')||root.startsWith(task+'/'))fail('overlapping-hosted-hello-roots');
  const build=JSON.parse(await fs.readFile(path.join(task,'cargo-cache-receipt.json'),'utf8'));
  if(build.snapshotSha256!==snapshot.sha256||sha(JSON.stringify(build.policy))!==build.namespace)fail('hosted-hello-build-custody');
  if((await snapshotInventory(root,HOSTED_SNAPSHOT_LIMITS)).sha256!==snapshot.sha256)fail('cargo-snapshot-mismatch');
  const binary=path.join(task,'target/release/conduit'),before=await hashFile(binary);
  if(build.outputs.filter(output=>output.name==='conduit').length!==1||build.outputs.find(output=>output.name==='conduit').sha256!==before)fail('hosted-hello-binary-mismatch');
  for(const name of ['hello-report.json','hello-proof.json','hello-artifacts','hello-home']){
    try{await fs.lstat(path.join(task,name));fail('hosted-hello-evidence-exists');}catch(error){if(error.code!=='ENOENT')throw error;}
  }
  await fs.mkdir(path.join(task,'hello-home'));
  const args=['--unshare-all','--die-with-parent','--new-session','--cap-drop','ALL','--clearenv',
    '--ro-bind',path.join(root,'source'),'/source','--ro-bind',path.join(root,'runtime/usr'),'/usr',
    '--ro-bind',path.join(root,'runtime/bin'),'/bin','--symlink','usr/lib','/lib','--symlink','usr/lib64','/lib64',
    '--proc','/proc','--dev','/dev','--bind',task,'/task','--chdir','/source',
    '--setenv','PATH','/usr/bin:/bin','--setenv','HOME','/task/hello-home','--setenv','TMPDIR','/task/tmp','--setenv','LANG','C',
    '/task/target/release/conduit','run','/source/plots/hello/main.conduit','--await-terminal',
    '--report','/task/hello-report.json','--artifacts','/task/hello-artifacts'];
  const result=await runSandbox(args,{timeoutMs:60000,resourcesFile:path.join(task,'hello-resource-usage.txt')});
  requireHostedHelloOutput(result.stdout);
  const reportFile=path.join(task,'hello-report.json'),stat=await fs.lstat(reportFile);
  if(!stat.isFile()||stat.size>16*1024**2)fail('invalid-hosted-hello-report');
  const report=JSON.parse(await fs.readFile(reportFile,'utf8'));
  if(report.schema!=='conduit.observatory.snapshot/v2'||report.hosts?.length!==1||report.plans?.length!==1)fail('invalid-hosted-hello-report');
  const after=await hashFile(binary);
  if(after!==before||(await snapshotInventory(root,HOSTED_SNAPSHOT_LIMITS)).sha256!==snapshot.sha256)fail('hosted-hello-input-mutated');
  const receipt={schema:1,scope:'Unchanged canonical hosted Hello executed from this exact retained release binary; independent of compiler-cache statistics',snapshotSha256:snapshot.sha256,namespace:build.namespace,binarySha256:before,binaryUnchanged:true,args,reportSha256:await hashFile(reportFile),validatedWallMs:performance.now()-started,...result};
  await fs.writeFile(path.join(task,'hello-stdout.txt'),result.stdout,{flag:'wx',mode:0o600});
  await fs.writeFile(path.join(task,'hello-proof.json'),JSON.stringify(receipt,null,2),{flag:'wx',mode:0o600});
  return receipt;
}
