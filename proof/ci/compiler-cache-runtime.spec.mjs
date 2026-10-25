import test from 'node:test';
import assert from 'node:assert/strict';
import { promises as fs } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { sealReviewedRustClosure, withCompilerCache } from '../../tools/ci/compiler-cache/runtime.mjs';

const rejected = (action, code) => assert.rejects(action, error => error.code === code);
test('runtime requires reviewed immutable closure and refuses procedural macro ambiguity', async () => {
  await rejected(() => sealReviewedRustClosure({ review: 'reviewed', immutableSnapshot: false }), 'unsupported-input-custody');
  await rejected(() => sealReviewedRustClosure({ review: 'reviewed', immutableSnapshot: true, externalProceduralMacros: true }), 'unsupported-input-custody');
});
test('runtime refuses an unpinned engine before touching cache or task output', async t => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'conduit-runtime-negative-'));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  const engine = path.join(root, 'bad-engine'); await fs.writeFile(engine, 'wrong');
  await rejected(() => withCompilerCache({ engine, cacheRoot: path.join(root, 'cache'), taskRoot: path.join(root, 'task'), maxCacheBytes: 1024, maxTaskBytes: 1024, reserveBytes: 1024 }, () => {}), 'engine-binary-mismatch');
  assert.deepEqual(await fs.readdir(root), ['bad-engine']);
});

test('real pinned server lifecycle, exclusive session, reserve refusal and changed-input refusal', {
  skip: !process.env.CONDUIT_CACHE_PROOF_ENGINE || !process.env.CONDUIT_CACHE_PROOF_RUSTC,
}, async t => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'conduit-runtime-engine-'));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  const source = path.join(root, 'input.rs');
  await fs.writeFile(source, 'pub fn increment(n: u64) -> u64 { n + 1 }');
  const closure = await sealReviewedRustClosure({source, inputs:[source], compiler:process.env.CONDUIT_CACHE_PROOF_RUSTC, review:'Single file without macros or external dependencies', immutableSnapshot:true});
  const options = { engine:process.env.CONDUIT_CACHE_PROOF_ENGINE, cacheRoot:path.join(root,'cache'), maxCacheBytes:1024*1024, reserveBytes:1024*1024, maxTaskBytes:1024*1024 };
  let saved;
  await withCompilerCache({...options,taskRoot:path.join(root,'task')}, async session => {
    saved=session;
    await rejected(() => withCompilerCache({...options,taskRoot:path.join(root,'contender')}, () => {}), 'runtime-cache-busy');
    const result = await session.compileRlib({closure,crateName:'actual_engine_test'});
    assert.ok(result.files.some(file=>file.name.endsWith('.rlib')));
    await fs.writeFile(source, 'pub fn increment(n: u64) -> u64 { n + 2 }');
    await rejected(() => session.compileRlib({closure,crateName:'actual_engine_test'}), 'source-custody-mismatch');
  });
  await rejected(() => saved.stats(), 'runtime-session-closed');
  await rejected(() => saved.compileRlib({closure,crateName:'after_session'}), 'runtime-session-closed');
  assert.equal((await fs.readdir(options.cacheRoot)).includes('.runtime-lease'),false);
  await rejected(() => withCompilerCache({...options,taskRoot:path.join(root,'refused'),reserveBytes:Number.MAX_SAFE_INTEGER},()=>{}), 'runtime-reserve-admission');
  assert.equal((await fs.readdir(root)).includes('refused'),false);
});

test('canonical root comparison refuses symlink aliases and dot-segment containment', async t => {
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'conduit-runtime-alias-'));
  t.after(()=>fs.rm(root,{recursive:true,force:true}));
  await fs.symlink(root,path.join(root,'alias'));
  const options={engine:'/unused-pinned-engine',cacheRoot:path.join(root,'cache'),maxCacheBytes:1024,maxTaskBytes:1024,reserveBytes:1024};
  await rejected(()=>withCompilerCache({...options,taskRoot:path.join(root,'alias','cache','proof')},()=>{}),'invalid-runtime-policy');
  await rejected(()=>withCompilerCache({...options,taskRoot:path.join(root,'other','..','cache','proof')},()=>{}),'invalid-runtime-policy');
  assert.deepEqual(await fs.readdir(root),['alias']);
});

test('pinned engine spawn failure becomes terminal and releases only its own lease', {
  skip: !process.env.CONDUIT_CACHE_PROOF_ENGINE,
}, async t => {
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'conduit-runtime-spawn-'));
  t.after(()=>fs.rm(root,{recursive:true,force:true}));
  const engine=path.join(root,'nonexecutable-engine');
  await fs.copyFile(process.env.CONDUIT_CACHE_PROOF_ENGINE,engine); await fs.chmod(engine,0o600);
  const cacheRoot=path.join(root,'cache');
  await rejected(()=>withCompilerCache({engine,cacheRoot,taskRoot:path.join(root,'task'),maxCacheBytes:1024*1024,maxTaskBytes:1024*1024,reserveBytes:1024*1024},()=>{}),'runtime-server-start-failed');
  assert.equal((await fs.readdir(cacheRoot)).includes('.runtime-lease'),false);
});

test('compiler spawn error settles after cleanup and session can still inspect its engine', {
  skip: !process.env.CONDUIT_CACHE_PROOF_ENGINE || !process.env.CONDUIT_CACHE_PROOF_RUSTC,
}, async t => {
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'conduit-runtime-rustc-spawn-'));
  t.after(()=>fs.rm(root,{recursive:true,force:true}));
  const compiler=path.join(root,'nonexecutable-rustc');
  await fs.copyFile(process.env.CONDUIT_CACHE_PROOF_RUSTC,compiler); await fs.chmod(compiler,0o600);
  const source=path.join(root,'input.rs'); await fs.writeFile(source,'pub fn a() {}');
  const closure=await sealReviewedRustClosure({source,inputs:[source],compiler,review:'Negative spawn test',immutableSnapshot:true});
  await withCompilerCache({engine:process.env.CONDUIT_CACHE_PROOF_ENGINE,cacheRoot:path.join(root,'cache'),taskRoot:path.join(root,'task'),maxCacheBytes:1024*1024,maxTaskBytes:1024*1024,reserveBytes:1024*1024},async session=>{
    await assert.rejects(()=>session.compileRlib({closure,crateName:'spawn_failure'}),error=>error.code==='EACCES');
    assert.equal((await session.stats()).stats.compile_requests,0);
  });
});
