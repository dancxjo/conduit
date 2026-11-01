import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const root = fileURLToPath(new URL('../../../../',import.meta.url));
test('Patchbay registry contains exact reachable membership source module graph',async()=>{
 const registry=JSON.parse(await readFile(new URL('../../patchbay-workbench/assets/patchbay.application.template.json',import.meta.url),'utf8'));
 const roles=new Map(registry.resources.map(row=>[row.role,row])),pending=['browser-membership'],visited=new Set();
 while(pending.length){
  const role=pending.pop();if(visited.has(role))continue;visited.add(role);
  const row=roles.get(role);assert(row,`unregistered ${role}`);
  const source=await readFile(path.join(root,row.source),'utf8');
  assert(Buffer.byteLength(source)<=row.maximum_bytes,`${role} exceeds existing module bound`);
  const imports=[...source.matchAll(/(?:import|export)\s+(?:[^'";]*?\s+from\s*)?['"]([^'"]+)['"]/g)].map(match=>match[1]);
  assert.deepEqual(new Set(row.dependencies.map(dep=>dep.specifier)),new Set(imports),`${role} exact imports`);
  for(const specifier of imports){
   const dep=row.dependencies.find(dep=>dep.specifier===specifier),target=roles.get(dep.role);
   assert(target,`${role} unregistered dependency ${dep.role}`);
   assert.equal(path.posix.normalize(path.posix.join(path.posix.dirname(row.path),specifier)),target.path,`${role} staged import must resolve inside admitted registry`);
   assert.equal(path.resolve(path.dirname(path.join(root,row.source)),specifier),path.join(root,target.source),`${role} source import identity`);
   pending.push(dep.role);
  }
 }
 for(const role of ['browser-membership-session','browser-bounded-document','browser-spore-membership','browser-part-membership','browser-spore-provision','browser-rendezvous-candidates'])assert(visited.has(role),`${role} unreachable`);
});

test('workspace candidate API keeps one canonical parser and admits its staged dependency',async()=>{
 const workspace=await import('../../workspace/creche-rendezvous-candidates.mjs');
 const shared=await import('./browser-rendezvous-candidates.mjs');
 assert.equal(workspace.boundedRendezvousCandidates,shared.boundedRendezvousCandidates);
 const template=JSON.parse(await readFile(new URL('../../workspace/workspace.application.template.json',import.meta.url),'utf8'));
 const wrapper=template.resources.find(row=>row.role==='creche-rendezvous-candidates');
 assert.deepEqual(wrapper.dependencies,[{role:'browser-rendezvous-candidates',specifier:'../host/assets/browser-rendezvous-candidates.mjs'}]);
 assert(template.resources.some(row=>row.role==='browser-rendezvous-candidates'));
 const stage=await readFile(new URL('../../tools/stage-browser-workspace.sh',import.meta.url),'utf8');
 assert.match(stage,/for asset in browser-rendezvous-candidates\.mjs /);
});
