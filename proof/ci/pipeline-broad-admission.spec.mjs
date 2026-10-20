import assert from 'node:assert/strict';
import { chmodSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { UNIT_SHARDS } from '../../tools/ci/pipeline/plan.mjs';

const cli = path.resolve('tools/ci/pipeline/cli.mjs');
const workflow = readFileSync('.github/workflows/ci.yml', 'utf8');
const job = name => workflow.split(`\n  ${name}:\n`)[1]?.split(/\n  [A-Za-z_][A-Za-z0-9_-]*:\n/)[0];

for (const [changed, broad, docsOnly] of [
  ['Cargo.toml', true, false],
  ['unknown/input.rs', true, false],
  ['site/index.html', true, false],
  ['proof/browser/browser-preparation.mjs', true, false],
  ['products/conduit/src/main.rs', false, false],
  ['README.md', false, true],
]) {
  test(`candidate scanner selects the complete broad corpus for ${changed}`, t => {
    const root=mkdtempSync(path.join(tmpdir(),'conduit-admission-'));
    t.after(()=>rmSync(root,{recursive:true,force:true}));
    const git=(...args)=>{
      const result=spawnSync('git',args,{cwd:root,encoding:'utf8'});
      assert.equal(result.status,0,result.stderr);
      return result.stdout.trim();
    };
    git('init'); git('config','user.name','CI proof'); git('config','user.email','ci@example.invalid');
    git('config','commit.gpgsign','false');
    mkdirSync(path.dirname(path.join(root,changed)),{recursive:true});
    writeFileSync(path.join(root,changed),'before\n'); git('add','.'); git('commit','-m','Base');
    const base=git('rev-parse','HEAD');
    writeFileSync(path.join(root,changed),'after\n'); git('commit','-am','Candidate');
    const head=git('rev-parse','HEAD');
    const bin=path.join(root,'bin');mkdirSync(bin);
    const metadata={workspace_members:['product'],packages:[{id:'product',name:'conduit',manifest_path:path.join(root,'products/conduit/Cargo.toml')}]};
    const calls=path.join(root,'cargo-calls');
    writeFileSync(path.join(bin,'cargo'),`#!/usr/bin/env node\nimport {appendFileSync} from 'node:fs';\nif(process.argv[2]!=='metadata'||!process.argv.includes('--locked'))process.exit(91);\nappendFileSync(process.env.CARGO_CALL_LOG,'metadata\\n');\nprocess.stdout.write(${JSON.stringify(JSON.stringify(metadata))});\n`);
    chmodSync(path.join(bin,'cargo'),0o755);
    const result=spawnSync(process.execPath,[cli,'scan',base,head,'candidate'],{
      cwd:root,encoding:'utf8',env:{...process.env,PATH:bin+path.delimiter+process.env.PATH,GITHUB_OUTPUT:'',CARGO_CALL_LOG:calls},
    });
    assert.equal(result.status,0,result.stderr);
    const output=Object.fromEntries(result.stdout.trim().split('\n').map(line=>{
      const at=line.indexOf('=');return [line.slice(0,at),line.slice(at+1)];
    }));
    assert.equal(output['broad-admission'],String(broad));
    assert.equal(output['docs-only'],String(docsOnly));
    const shards = JSON.parse(output['unit-matrix']).shard;
    if(broad) assert.deepEqual(shards,[...UNIT_SHARDS]);
    assert.equal(output['conduitos-proof'], String(shards.includes('hosts-conduitos')),
      'Every selected ConduitOS shard must retain its required portable protocol proof');
    assert.equal(existsSync(calls),!docsOnly);
  });
}

test('broad admission runs independently while completion requires every delegated shard',()=>{
  const admission=job('admission');
  assert.match(admission,/needs: preflight\n/);
  assert.match(admission,/always\(\) && !cancelled\(\)/);
  assert.match(job('unit'),/inputs\.exhaustive \|\| needs\.preflight\.outputs\.broad-admission == 'true'/);
  const gate=admission.split('      - name: Require the selected admission route\n')[1].split('      - uses:')[0];
  const script=gate.split('        run: |\n')[1].replace(/^          /gm,'');
  const passes=route=>spawnSync('bash',['-e','-c',script],{
    env:{...process.env,BROAD_ADMISSION:route},stdio:'pipe',
  }).status===0;
  for(const route of ['true','false']) assert.equal(passes(route),true);
  for(const route of ['', 'unknown']) assert.equal(passes(route),false,route);
  assert.match(admission,/cargo xtask ci pipeline admission "\$BASE" --workspace-checked/);
  assert.match(admission,/else\n            cargo xtask ci pipeline admission "\$BASE"/);
});
