import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rename, rm, symlink, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { sealTarget, verifyBundle } from '../../tools/ci/pipeline/receipts.mjs';

const sha = 'a'.repeat(40);
const targets = [{ id: 'hosted-linux', proofClass: 'executable' }];

async function fixture(t) {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'pipeline-receipts-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const product = path.join(directory, 'hosted-linux');
  await mkdir(path.join(product, 'bin'), { recursive: true });
  await writeFile(path.join(product, 'bin/conduit'), 'tested executable');
  const seal = () => sealTarget({ directory: product, target: targets[0].id, sha, proofClass: targets[0].proofClass });
  const verify = (overrides = {}) => verifyBundle({ directory, sha, targets, ...overrides });
  const mutateReceipt = async mutate => {
    const filename = path.join(product, 'receipt.json');
    const receipt = JSON.parse(await readFile(filename, 'utf8'));
    mutate(receipt);
    await writeFile(filename, JSON.stringify(receipt));
  };
  return { directory, product, seal, verify, mutateReceipt };
}

test('seals exact products and verifies a multi-target bundle with distinct proof classes', async t => {
  const f = await fixture(t);
  const receipt = await f.seal();
  assert.equal(receipt.files[0].path, 'bin/conduit');
  assert.equal(receipt.files[0].size, Buffer.byteLength('tested executable'));
  assert.deepEqual(await f.verify(), [receipt]);
  // Re-sealing excludes the old receipt and leaves no temporary product.
  assert.deepEqual(await f.seal(), receipt);
  const firmware = path.join(f.directory, 'esp32-c3');
  await mkdir(firmware);
  await writeFile(path.join(firmware, 'firmware.bin'), Buffer.from([0, 1, 255]));
  await sealTarget({ directory: firmware, target: 'esp32-c3', sha, proofClass: 'build' });
  const receipts = await f.verify({ targets: [...targets, { id: 'esp32-c3', proofClass: 'build' }] });
  assert.deepEqual(receipts.map(r => r.proofClass), ['build', 'executable']);
});

for (const [name, change] of [
  ['changed bytes', f => writeFile(path.join(f.product, 'bin/conduit'), 'tampered executable')],
  ['extra file', f => writeFile(path.join(f.product, 'extra'), 'unproved')],
  ['missing file', f => rm(path.join(f.product, 'bin/conduit'))],
  ['renamed file', f => rename(path.join(f.product, 'bin/conduit'), path.join(f.product, 'bin/other'))],
]) {
  test(`rejects ${name} after successful proof`, async t => {
    const f = await fixture(t);
    await f.seal();
    await change(f);
    await assert.rejects(f.verify(), /digest or file set mismatch/);
  });
}

for (const [name, change] of [
  ['wrong commit', r => { r.sha = 'b'.repeat(40); }],
  ['wrong target', r => { r.target = 'browser'; }],
  ['wrong proof class', r => { r.proofClass = 'build'; }],
  ['unknown proof class', r => { r.proofClass = 'physical'; }],
  ['wrong schema', r => { r.schema = 'other'; }],
  ['empty products', r => { r.files = []; }],
  ['duplicate products', r => { r.files.push(r.files[0]); }],
  ['parent traversal', r => { r.files[0].path = '../outside'; }],
  ['absolute path', r => { r.files[0].path = '/outside'; }],
  ['Windows absolute path', r => { r.files[0].path = 'C:/outside'; }],
  ['Windows traversal', r => { r.files[0].path = '..\\outside'; }],
  ['receipt as product', r => { r.files[0].path = 'receipt.json'; }],
  ['invalid digest', r => { r.files[0].sha256 = 'unknown'; }],
]) {
  test(`rejects receipt with ${name}`, async t => {
    const f = await fixture(t);
    await f.seal();
    await f.mutateReceipt(change);
    await assert.rejects(f.verify());
  });
}

test('requires exactly the authoritative target set', async t => {
  const f = await fixture(t);
  await f.seal();
  await assert.rejects(f.verify({ targets: [] }), /empty/);
  await assert.rejects(f.verify({ targets: [...targets, ...targets] }), /Duplicate/);
  await assert.rejects(f.verify({ targets: [...targets, { id: 'browser', proofClass: 'browser' }] }), /target set/);
  await mkdir(path.join(f.directory, 'unexpected'));
  await assert.rejects(f.verify(), /target set/);
});

test('failed or absent target proof cannot verify without its receipt', async t => {
  const f = await fixture(t);
  await assert.rejects(f.verify(), /ENOENT/);
});

test('empty products and invalid seal identities never write a receipt', async t => {
  const f = await fixture(t);
  await rm(path.join(f.product, 'bin'), { recursive: true });
  await assert.rejects(f.seal(), /No products/);
  await assert.rejects(readFile(path.join(f.product, 'receipt.json')), /ENOENT/);
  for (const overrides of [{ target: '../escape' }, { sha: 'short' }, { proofClass: 'physical' }]) {
    await assert.rejects(sealTarget({ directory: f.product, target: targets[0].id, sha, proofClass: 'executable', ...overrides }));
  }
});

for (const kind of ['file', 'directory', 'receipt', 'target', 'bundle']) {
  test(`rejects a symlink ${kind}`, async t => {
    const f = await fixture(t);
    await f.seal();
    if (kind === 'file') {
      await symlink('conduit', path.join(f.product, 'bin/link'));
    } else if (kind === 'directory') {
      await symlink('bin', path.join(f.product, 'linked-bin'), 'dir');
    } else if (kind === 'receipt') {
      await rename(path.join(f.product, 'receipt.json'), path.join(f.product, 'original.json'));
      await symlink('original.json', path.join(f.product, 'receipt.json'));
    } else if (kind === 'target') {
      await rename(f.product, path.join(f.directory, 'original'));
      await symlink('original', f.product, 'dir');
      await assert.rejects(f.verify({ targets: [...targets, { id: 'original', proofClass: 'executable' }] }));
      return;
    } else {
      const link = `${f.directory}-link`;
      t.after(() => rm(link, { force: true }));
      await symlink(f.directory, link, 'dir');
      await assert.rejects(f.verify({ directory: link }), /real directory/);
      return;
    }
    await assert.rejects(f.verify(), /Symlink|Invalid receipt/);
    await assert.rejects(f.seal(), /Symlink/);
  });
}
