import test from 'node:test';
import assert from 'node:assert/strict';
import {runSandbox} from '../../tools/ci/compiler-cache/cargo-sandbox.mjs';
test('custody observes actual successful bwrap process and its minimal effective host environment',async()=>{
 const observations=[];await runSandbox(['--version'],{observeCommand:value=>{observations.push(value);}});
 assert.equal(observations.length,1);const record=observations[0];
 assert.equal(record.program,'/usr/bin/bwrap');assert.deepEqual(record.args,['--version']);
 assert.deepEqual(record.environment,{PATH:'/usr/bin:/bin',LANG:'C'});
 assert.equal(record.cwd,process.cwd());assert.equal(record.outcome,'success');
 assert.equal(record.code,0);assert.ok(record.durationMs>=0);
});
test('failed command is observed once before refusal and cannot masquerade as host Cargo',async()=>{
 const observations=[];await assert.rejects(runSandbox(['--not-a-bwrap-option'],{observeCommand:value=>{observations.push(value);}}),{code:'cargo-sandbox-failed'});
 assert.equal(observations.length,1);assert.equal(observations[0].program,'/usr/bin/bwrap');
 assert.equal(observations[0].outcome,'failed');assert.notEqual(observations[0].code,0);
});
test('custody write errors fail closed even when spawned command succeeds',async()=>{
 await assert.rejects(runSandbox(['--version'],{observeCommand:()=>{throw Error('custody write refused');}}),/custody write refused/);
 await assert.rejects(runSandbox(['--version'],{observeCommand:()=>Promise.resolve()}),{code:'async-command-observer'});
});
