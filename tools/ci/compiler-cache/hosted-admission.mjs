import {promises as fs} from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {CacheRefusal} from './refusal.mjs';
import {snapshotInventory} from './cargo-snapshot.mjs';
const PROFILE_BYTES=await fs.readFile(new URL('./audited-dependencies.json',import.meta.url));
const freeze=value=>{if(value&&typeof value==='object'){Object.values(value).forEach(freeze);Object.freeze(value);}return value;};
export const AUDITED_DEPENDENCIES=freeze(JSON.parse(PROFILE_BYTES));
const fail=code=>{throw new CacheRefusal(code);};
const sha=value=>createHash('sha256').update(value).digest('hex');
// Admission is exact immutable source identity, never a source-token scanner or
// a producer assertion of complete inputs. Changing audited source (including an
// assembly include) requires a fresh review/profile; it cannot inherit admission.
export async function buildHostedAdmission({root,metadata,bypassNames}){
 const crates=[];
 for(const reviewed of AUDITED_DEPENDENCIES.crates){
  const matches=metadata.packages.filter(pkg=>pkg.name===reviewed.name&&pkg.version===reviewed.version);
  if(!matches.length)continue;if(matches.length!==1)fail('ambiguous-audited-dependency');
  const pkg=matches[0];
  if(pkg.source!=='registry+https://github.com/rust-lang/crates.io-index'||pkg.targets.some(target=>target.kind.includes('custom-build')||target.kind.includes('proc-macro')))fail('unsupported-audited-dependency');
  const directory=path.join(root,'vendor',`${reviewed.name}-${reviewed.version}`);
  const inventory=await snapshotInventory(directory,{maxBytes:8*1024**2,maxFiles:1000});
  const checksum=JSON.parse(await fs.readFile(path.join(directory,'.cargo-checksum.json'),'utf8'));
  if(checksum.package!==reviewed.archiveSha256||inventory.sha256!==reviewed.sourceInventorySha256)fail('audited-dependency-source-mismatch');
  if(bypassNames.includes(reviewed.name))continue;
  const packages=new Map(metadata.packages.map(item=>[item.id,item]));
  const nodes=new Map((metadata.resolve?.nodes??[]).map(item=>[item.id,item]));
  const pending=[pkg.id],visited=new Set(),dependencies=[];
  while(pending.length){
    const id=pending.pop();if(visited.has(id))continue;visited.add(id);
    const dependency=packages.get(id),node=nodes.get(id);
    if(!dependency||!node||dependency.source!=='registry+https://github.com/rust-lang/crates.io-index'||!Array.isArray(node.deps))fail('unsupported-audited-dependency-closure');
    if(id!==pkg.id)dependencies.push({name:dependency.name,version:dependency.version,source:dependency.source});
    for(const next of node.deps)pending.push(next.pkg);
  }
  dependencies.sort((a,b)=>(a.name+'@'+a.version).localeCompare(b.name+'@'+b.version));
  crates.push({...reviewed,dependencies});
 }
 const admission={schema:1,policy:AUDITED_DEPENDENCIES.policy,reviewedProfileSha256:sha(PROFILE_BYTES),crates};
 const cases=crates.map(pkg=>`  '/vendor/${pkg.name}-${pkg.version}:${pkg.name}:${pkg.version}') admitted='${pkg.name}-${pkg.version}' ;;`).join('\n');
 const script=`# Finite reviewed identities only; all workspace/other crates compile normally.
admitted=''
case "\${CARGO_MANIFEST_DIR:-}:\${CARGO_PKG_NAME:-}:\${CARGO_PKG_VERSION:-}" in
${cases}
esac
[ -n "$admitted" ] || exec "$compiler" "$@"
[ "$compiler" = /toolchain/bin/rustc ] || { echo 'unsupported admitted compiler' >&2; exit 64; }
source_ok=0
cfg_next=0
for arg in "$@"; do
  if [ "$cfg_next" = 1 ]; then
    case "$arg" in test|doctest|docsrs|'feature="test"') echo 'unsupported admitted test configuration' >&2; exit 64 ;; esac
    cfg_next=0
  fi
  [ "$arg" != --cfg ] || cfg_next=1
  case "$arg" in
    /vendor/"$admitted"/src/lib.rs) source_ok=1 ;;
    *.rs) echo 'unreviewed admitted source path' >&2; exit 64 ;;
    --test|--cfg=test|--cfg=doctest|--cfg=docsrs|-Z*|*llvm-args*|*target-cpu*|*target-feature*|*codegen-backend*|*incremental*|*link-arg*|--target|--target=*|--sysroot|--sysroot=*) echo 'unsupported admitted compiler extension' >&2; exit 64 ;;
  esac
done
[ "$source_ok" = 1 ] || { echo 'missing admitted library source' >&2; exit 64; }
`;
 return {admission,script};
}
export function sharedCompatibilityInventory(files){
 const prefixes=['toolchain/','engine/','runtime/','vendor/','source/.cargo/'];
 const names=['native-policy.json','cargo-config.toml','shared-admission.json','shared-admission.sh'];
 const selected=files.filter(file=>names.includes(file.path)||prefixes.some(prefix=>file.path.startsWith(prefix)));
 if(!names.every(name=>selected.some(file=>file.path===name))||!prefixes.slice(0,4).every(prefix=>selected.some(file=>file.path.startsWith(prefix))))fail('incomplete-shared-cache-closure');
 return selected;
}
export async function verifyHostedAdmission(root){
 const metadata=JSON.parse(await fs.readFile(path.join(root,'metadata.json'),'utf8'));
 const bypassNames=(await fs.readFile(path.join(root,'proc-macro-bypass.txt'),'utf8')).trimEnd().split('\n');
 const expected=await buildHostedAdmission({root,metadata,bypassNames});
 if(await fs.readFile(path.join(root,'shared-admission.json'),'utf8')!==JSON.stringify(expected.admission)||await fs.readFile(path.join(root,'shared-admission.sh'),'utf8')!==expected.script)fail('audited-dependency-admission-mismatch');
 return expected;
}
